# bevy-jsx

React-like declarative UI composition for Bevy ECS.

A component is a plain function from a **props struct** to `impl Bundle`.
Trees are composed with the `element!` macro, which expands to ordinary
`spawn` + `with_children` calls — no hidden spawn machinery, no trait
objects, no type erasure. Marker components, queries and change detection
work exactly as in hand-written Bevy code.

```rust
use bevy::prelude::*;
use bevy_jsx::{element, jsx_component};

jsx_component! {
    Container(ContainerProps) {
        flex_direction: FlexDirection = FlexDirection::Row,
        column_gap: Val = Val::Auto,
        background_color: Color = Color::NONE,
    }
    |{background_color, ..props}| {
        (
            Node { ..props, ..default() },
            BackgroundColor(background_color),
        )
    }
}

fn setup(mut commands: Commands) {
    let ui = element! {
        Container(column_gap: Val::Px(16.0), background_color: Color::BLACK) {
            Label(content: "Hello".into())
            Btn(id: SaveButton, label: "Save".into())
        }
    };
    ui.spawn(&mut commands);
}
```

## Crate layout

| Crate | Contents |
|---|---|
| `bevy-jsx` | `element!`, `WithChildren`, the `__JsxComponent` trait; re-exports `jsx_component!` |
| `bevy-jsx-macro` | proc-macro implementation of `jsx_component!` (do not depend on it directly) |

```toml
[dependencies]
bevy-jsx = { path = "../bevy-jsx" }
```

---

## Defining components — `jsx_component!`

```rust
jsx_component! {
    /// Doc comment goes onto both generated structs.
    Name(NameProps) {
        // props
    }
    |props| { /* build: props -> impl Bundle */ }
}
```

The macro generates:

1. `pub struct NameProps { ... }` — `Debug + Clone`, with a hand-rolled
   `Default` impl honouring per-field defaults;
2. `pub struct Name;` — the unit struct used as the tag in `element!`;
3. `impl __JsxComponent for Name` with `fn build(props) -> impl Bundle`.

### Props and defaults

Each prop is `name: Type` with an optional `= expr` default. Fields without
an explicit default fall back to `Default::default()`:

```rust
jsx_component! {
    Badge(BadgeProps) {
        text: String,                 // String::default()
        color: Color = Color::WHITE,  // explicit default
        size: Option<f32>,            // optional prop, None by default
    }
    |props| { /* ... */ }
}
```

All prop types must implement `Debug + Clone`. Default expressions may be
arbitrary expressions (constants, function calls, `UiRect::all(...)`, …).

### Build closure: whole-props form

`|props| body` binds the entire props struct under one name:

```rust
|props| {
    Node { width: props.width, ..default() }
}
```

### Build closure: destructuring form with `..props` spread

`|{a, b, ..rest}| body` extracts the named props into local bindings and
makes the *remaining* props available through a pseudo-spread `..rest`
inside any struct literal in the body:

```rust
jsx_component! {
    Container(ContainerProps) {
        width: Val = Val::Auto,
        height: Val = Val::Auto,
        flex_direction: FlexDirection = FlexDirection::Row,
        background_color: Color = Color::NONE,
    }
    |{background_color, ..props}| {
        (
            Node {
                ..props,        // ⇐ expands to: width: …, height: …, flex_direction: …,
                ..default()     // fills the rest of Node's fields
            },
            BackgroundColor(background_color),
        )
    }
}
```

`..props` is not real Rust (struct-update syntax allows a single base, last)
— the macro rewrites it at expansion time into an explicit
`field: value` list for every prop **not** extracted, so the remaining
`..default()` stays the single real base.

Rules:

- every extracted name must be a declared prop — compile error otherwise;
- every *remaining* prop must be a field of the struct you spread into
  (here `Node`); a stray prop produces the usual "struct has no field" error;
- keep `..default()` (or another base) after `..props` when the target
  struct has fields the props don't cover;
- the spread *moves* the remaining props — spread once, unless all remaining
  props are `Copy`;
- explicit fields can be mixed with the spread:
  `Node { ..props, width: Val::Percent(100.0), ..default() }`;
- the rest binding may be omitted (`|{color}|`) when no spread is needed;
- inside the body the token sequence `..name` (with the rest binding's name)
  is *always* treated as the spread — don't reuse that name as a range bound.

---

## Composing trees — `element!`

```rust
element! {
    Container(flex_direction: FlexDirection::Column) {
        Label(content: "Title".into())
        Btn(id: SaveButton, label: "Save".into())
        Spacer
        #(prebuilt_element)
    }
}
```

Child syntax inside the braces (whitespace-separated, no commas between
children):

| Syntax | Meaning |
|---|---|
| `Name` | self-closing, default props |
| `Name(prop: value, ...)` | self-closing with props |
| `Name(id: Marker, prop: value, ...)` | + inserts `Marker` on the entity (`id` must be first) |
| `Name { children… }` | with children, default props |
| `Name(props…) { children… }` | with children and props |
| `#(expr)` | splice a prebuilt `WithChildren` value |

### Return type and spawning

- **Self-closing root** (`element! { Spacer(flex_grow: 1.0) }`) expands to a
  plain `impl Bundle` — pass it to `commands.spawn(...)`, `children![...]`,
  anywhere a bundle goes.
- **Root with children** expands to a `WithChildren { bundle, spawn_children }`
  value. Spawn it with:
  - `el.spawn(&mut commands)` — top level, returns `Entity`;
  - `el.spawn_into(parent)` — inside a `with_children` closure.

### Markers (`id:`)

`id: Marker` inserts any component (typically a unit marker) on the spawned
entity, so systems can target it with ordinary queries:

```rust
#[derive(Component)]
struct SaveButton;

element! { Btn(id: SaveButton, label: "Save".into()) }

fn on_save(q: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>) { /* … */ }
```

`id` accepts a path, so namespaced markers (`id: markers::Save`) work too.

### Conditional rendering

```rust
element! {
    Container {
        if show_title => { Label(content: "Title".into()) }
        if let Some(img) = texture => { Img(image: img) }
        else => { Label(content: "No image".into()) }
        Fragment {
            Label(content: "A".into())
            Label(content: "B".into())
        }
    }
}
```

`if` / `if let` / `else` / `else if` use `=>` before the child block
because Rust macros cannot parse `$expr:expr { ... }`.

### Splice (`#(expr)`)

Any expression evaluating to a `WithChildren` can be interpolated as a child.
Useful for building parts of a tree conditionally or in a loop... at the
moment a splice spawns exactly one element; to splice a collection, wrap the
loop in a plain `with_children` call or splice each item individually.

```rust
let card = element! { Container { Label(content: "Card".into()) } };
element! {
    Container {
        #(card)
    }
}
```

---

## How it expands

`element!` with children becomes, roughly:

```rust
WithChildren {
    bundle: <Container as __JsxComponent>::build({
        let mut props = ContainerProps::default();
        props.column_gap = Val::Px(16.0);
        props
    }),
    spawn_children: |parent: &mut ChildSpawnerCommands| {
        parent.spawn(<Label as __JsxComponent>::build(/* … */));
        parent.spawn(<Btn as __JsxComponent>::build(/* … */)).insert(SaveButton);
    },
}
```

Everything is statically typed; an unknown prop name fails to compile with a
normal "no field" error pointing at the call site.

## Examples & tests

- `examples/basic.rs` — static layout exercising every syntax form
  (`cargo run -p bevy-jsx --example basic`);
- `examples/interactive.rs` — counter with buttons driven through `id:`
  markers (`cargo run -p bevy-jsx --example interactive`);
- `tests/render.rs` — spawns trees into a real `World` and asserts entity
  hierarchy, ordering, component values, markers and splice behaviour.

## Limitations

- Children inside `element!` are static — conditional/repeated children go
  through `#(expr)` splices or a hand-written `with_children`.
- `id:` must be the first entry in the prop list.
- Props are set by field assignment, so prop *names* must be plain
  identifiers (no shorthand or nesting).
- The `..props` spread relies on token rewriting: it only understands the
  literal `..name` sequence, and every remaining prop must exist on the
  target struct.