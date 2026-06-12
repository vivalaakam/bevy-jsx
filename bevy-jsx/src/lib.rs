//! # bevy-jsx
//!
//! React-like declarative UI composition for Bevy ECS.
//!
//! Components are defined with [`jsx_component!`] and composed with
//! [`element!`]. See the crate-level docs below for the full syntax.
//!
//! ## Quick example
//!
//! ```ignore
//! use bevy_jsx::{jsx_component, element};
//!
//! // Define a component (in a separate crate that depends on bevy-jsx-macro)
//! jsx_component! {
//!     /// A text label.
//!     Label(LabelProps) {
//!         content: String,           // required — must always be provided
//!         font_size: f32 = 16.0,     // optional — defaults to 16.0
//!         color: Color = TEXT_COLOR,  // optional — defaults to TEXT_COLOR
//!     }
//!     |{ content, font_size, color }| {
//!         (
//!             Text(content),
//!             TextFont { font_size, ..default() },
//!             TextColor(color),
//!         )
//!     }
//! }
//!
//! // Compose in system code
//! fn setup(mut commands: Commands) {
//!     use bevy_jsx::Spawnable;
//!     // Required fields must be provided
//!     let el = element! { Label(content: "Hello".into()) };
//!     el.spawn_top(&mut commands);
//! }
//! ```
//!
//! ## Conditional rendering
//!
//! ```ignore
//! element! {
//!     FlexContainer {
//!         if show_title => { Label(content: "Title".into()) }
//!         if let Some(img) = texture => { Img(image: img) }
//!         else => { Label(content: "No image".into()) }
//!         Fragment {
//!             Label(content: "A".into())
//!             Label(content: "B".into())
//!         }
//!     }
//! }
//! ```
//!
//! `if` / `if let` / `else` / `else if` use `=>` before the child block
//! because Rust macros cannot parse `$expr:expr { ... }`.

extern crate self as bevy_jsx;

// ── Core types ────────────────────────────────────────────────────────────

/// Defines a JSX component: a props struct + a build function.
///
/// Fields **without** `= default_expr` are **required** — they must be
/// provided when calling `element!`.
///
/// Fields **with** `= default_expr` are **optional** — they get the specified
/// default and can be omitted.
///
/// Required fields are stored as `Option<T>` in a hidden partial-props
/// struct; omitting one panics with a descriptive message when the
/// component is built.
pub use bevy_jsx_macro::jsx_component;

pub trait __JsxComponent {
    type Props;
    fn build(props: Self::Props) -> impl Spawnable;
}

/// Something that can be spawned into the Bevy world.
/// Implemented for both `impl Bundle` (self-closing elements)
/// and `WithChildren` (elements with children).
///
/// Both methods return [`EntityCommands`] so callers (including the
/// `element!` / `__spawn_children!` macros) can chain `.insert(...)` /
/// `.with_children(...)` on the spawned entity.
pub trait Spawnable {
    /// Spawn as a child of `parent`.
    fn spawn_into<'a>(
        self,
        parent: &'a mut bevy::prelude::ChildSpawnerCommands,
    ) -> bevy::prelude::EntityCommands<'a>;
    /// Spawn as a top-level entity via `Commands`.
    fn spawn_top<'a>(
        self,
        commands: &'a mut bevy::prelude::Commands,
    ) -> bevy::prelude::EntityCommands<'a>;
}

impl<B: bevy::prelude::Bundle + Send + Sync + 'static> Spawnable for B {
    fn spawn_into<'a>(
        self,
        parent: &'a mut bevy::prelude::ChildSpawnerCommands,
    ) -> bevy::prelude::EntityCommands<'a> {
        parent.spawn(self)
    }
    fn spawn_top<'a>(
        self,
        commands: &'a mut bevy::prelude::Commands,
    ) -> bevy::prelude::EntityCommands<'a> {
        commands.spawn(self)
    }
}

impl<S: Spawnable, F: FnOnce(&mut bevy::prelude::ChildSpawnerCommands) + Send + Sync + 'static>
    Spawnable for WithChildren<S, F>
{
    fn spawn_into<'a>(
        self,
        parent: &'a mut bevy::prelude::ChildSpawnerCommands,
    ) -> bevy::prelude::EntityCommands<'a> {
        let mut entity = self.bundle.spawn_into(parent);
        entity.with_children(self.spawn_children);
        entity
    }
    fn spawn_top<'a>(
        self,
        commands: &'a mut bevy::prelude::Commands,
    ) -> bevy::prelude::EntityCommands<'a> {
        let mut entity = self.bundle.spawn_top(commands);
        entity.with_children(self.spawn_children);
        entity
    }
}

/// Wrapper for elements that have children.
///
/// `bundle` is itself a [`Spawnable`], so component trees nest: a
/// `jsx_component!` body may return a whole `element!` tree and still be
/// used as the parent of further children.
pub struct WithChildren<S, F> {
    pub bundle: S,
    pub spawn_children: F,
}

impl<S: Spawnable, F: FnOnce(&mut bevy::prelude::ChildSpawnerCommands) + Send + Sync + 'static>
    WithChildren<S, F>
{
    /// Spawn a `WithChildren` as a top-level entity via `Commands`.
    pub fn spawn(self, commands: &mut bevy::prelude::Commands) -> bevy::prelude::Entity {
        Spawnable::spawn_top(self, commands).id()
    }
}

/// Wrapper that inserts a marker component after spawning the inner element.
///
/// Produced by `element!` when the root element has an `id: Marker` prop.
pub struct WithMarker<S, M> {
    pub inner: S,
    pub marker: M,
}

impl<S: Spawnable, M: bevy::prelude::Bundle> Spawnable for WithMarker<S, M> {
    fn spawn_into<'a>(
        self,
        parent: &'a mut bevy::prelude::ChildSpawnerCommands,
    ) -> bevy::prelude::EntityCommands<'a> {
        let mut entity = self.inner.spawn_into(parent);
        entity.insert(self.marker);
        entity
    }
    fn spawn_top<'a>(
        self,
        commands: &'a mut bevy::prelude::Commands,
    ) -> bevy::prelude::EntityCommands<'a> {
        let mut entity = self.inner.spawn_top(commands);
        entity.insert(self.marker);
        entity
    }
}

// ── Macros ────────────────────────────────────────────────────────────────

/// Declarative JSX-like element composition.
///
/// See the [crate-level documentation](crate) for the full syntax reference,
/// including conditional rendering and `Fragment`.
#[macro_export]
macro_rules! element {
    // ── With children + id + props ───────────────────────────────────────
    ($component:ident ( id : $marker:path, $($prop_name:ident : $prop_val:expr),* $(,)? ) { $($children:tt)* }) => {{
        let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        $(_jsx_props.$prop_name($prop_val);)*
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithChildren {
            bundle: $crate::WithMarker { inner: _jsx_bundle, marker: $marker },
            spawn_children: move |__jsx_parent: &mut ::bevy::prelude::ChildSpawnerCommands| {
                $crate::__spawn_children!(__jsx_parent $($children)*);
            },
        }
    }};
    // ── With children + id only ──────────────────────────────────────────
    ($component:ident ( id : $marker:path ) { $($children:tt)* }) => {{
        let _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithChildren {
            bundle: $crate::WithMarker { inner: _jsx_bundle, marker: $marker },
            spawn_children: move |__jsx_parent: &mut ::bevy::prelude::ChildSpawnerCommands| {
                $crate::__spawn_children!(__jsx_parent $($children)*);
            },
        }
    }};
    // ── Self-closing with id + props ─────────────────────────────────────
    ($component:ident ( id : $marker:path, $($prop_name:ident : $prop_val:expr),* $(,)? )) => {{
        let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        $(_jsx_props.$prop_name($prop_val);)*
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithMarker { inner: _jsx_bundle, marker: $marker }
    }};
    // ── Self-closing with id only ────────────────────────────────────────
    ($component:ident ( id : $marker:path )) => {{
        let _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithMarker { inner: _jsx_bundle, marker: $marker }
    }};

    // ── With children + props ────────────────────────────────────────────
    ($component:ident ( $($prop_name:ident : $prop_val:expr),* $(,)? ) { $($children:tt)* }) => {{
        let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        $(_jsx_props.$prop_name($prop_val);)*
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithChildren {
            bundle: _jsx_bundle,
            spawn_children: move |__jsx_parent: &mut ::bevy::prelude::ChildSpawnerCommands| {
                $crate::__spawn_children!(__jsx_parent $($children)*);
            },
        }
    }};
    // ── With children, no props ─────────────────────────────────────────
    ($component:ident { $($children:tt)* }) => {{
        let _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        let _jsx_bundle = <$component as $crate::__JsxComponent>::build(_jsx_props);
        $crate::WithChildren {
            bundle: _jsx_bundle,
            spawn_children: move |__jsx_parent: &mut ::bevy::prelude::ChildSpawnerCommands| {
                $crate::__spawn_children!(__jsx_parent $($children)*);
            },
        }
    }};

    // ── Self-closing with props ──────────────────────────────────────────
    ($component:ident ( $($prop_name:ident : $prop_val:expr),* $(,)? )) => {{
        let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        $(_jsx_props.$prop_name($prop_val);)*
        <$component as $crate::__JsxComponent>::build(_jsx_props)
    }};
    // ── Self-closing, no props ───────────────────────────────────────────
    ($component:ident) => {{
        let _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
        <$component as $crate::__JsxComponent>::build(_jsx_props)
    }};
}

/// Internal: spawns children inside a `with_children` closure.
///
/// Uses TT munching. Rules are ordered most-specific first to avoid ambiguity.
/// `if` / `if let` / `else` use `=>` as the separator before the child block
/// because Rust macros cannot parse `$expr:expr { ... }`.
#[macro_export]
macro_rules! __spawn_children {
    // ── Empty ─────────────────────────────────────────────────────
    ($parent:ident) => {};

    // ── Fragment { ... } ──────────────────────────────────────────
    // Spawns children directly, without a wrapper DOM node.
    ($parent:ident Fragment { $($children:tt)* } $($rest:tt)*) => {
        $crate::__spawn_children!($parent $($children)*);
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── if / if let ... else if ... else chains ───────────────────
    // Delegated to `__jsx_if_chain!`, which folds the whole chain into a
    // single Rust `if`/`else if`/`else` statement. A standalone macro arm
    // cannot expand to an orphan `else { ... }`, so the chain has to be
    // assembled in one expansion.
    ($parent:ident if $($rest:tt)*) => {
        $crate::__jsx_if_chain!($parent [] if $($rest)*);
    };

    // ── Splice expression: #(expr) ────────────────────────────────
    ($parent:ident #($expr:expr) $($rest:tt)*) => {
        {
            let _jsx_el = $expr;
            $crate::Spawnable::spawn_into(_jsx_el, $parent);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── Child with braces + id + props: Name(id: Marker, props...) { ... } ──
    ($parent:ident $component:ident ( id : $marker:path, $($prop_name:ident : $prop_val:expr),* $(,)? ) { $($inner:tt)* } $($rest:tt)*) => {
        {
            let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
            $(_jsx_props.$prop_name($prop_val);)*
            let _jsx_output = <$component as $crate::__JsxComponent>::build(_jsx_props);
            $crate::Spawnable::spawn_into(_jsx_output, $parent)
                .with_children(|__p: &mut ::bevy::prelude::ChildSpawnerCommands| {
                    $crate::__spawn_children!(__p $($inner)*);
                })
                .insert($marker);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── Child with braces + id only: Name(id: Marker) { ... } ─────
    ($parent:ident $component:ident ( id : $marker:path ) { $($inner:tt)* } $($rest:tt)*) => {
        {
            let _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
            let _jsx_output = <$component as $crate::__JsxComponent>::build(_jsx_props);
            $crate::Spawnable::spawn_into(_jsx_output, $parent)
                .with_children(|__p: &mut ::bevy::prelude::ChildSpawnerCommands| {
                    $crate::__spawn_children!(__p $($inner)*);
                })
                .insert($marker);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── Child with braces + props: Name(props) { ... } ────────────
    ($parent:ident $component:ident ( $($prop_name:ident : $prop_val:expr),* $(,)? ) { $($inner:tt)* } $($rest:tt)*) => {
        {
            let _jsx_el = $crate::element! {
                $component ( $($prop_name : $prop_val),* ) { $($inner)* }
            };
            $crate::Spawnable::spawn_into(_jsx_el, $parent);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── Child with braces, no props: Name { ... } ──────────────────
    ($parent:ident $component:ident { $($inner:tt)* } $($rest:tt)*) => {
        {
            let _jsx_el = $crate::element! {
                $component { $($inner)* }
            };
            $crate::Spawnable::spawn_into(_jsx_el, $parent);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── Self-closing with id + props, followed by something ───────
    ($parent:ident $component:ident ( id : $marker:path, $($prop_name:ident : $prop_val:expr),* $(,)? ) $next:tt $($rest:tt)*) => {
        {
            let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
            $(_jsx_props.$prop_name($prop_val);)*
            let _jsx_output = <$component as $crate::__JsxComponent>::build(_jsx_props);
            $crate::Spawnable::spawn_into(_jsx_output, $parent).insert($marker);
        }
        $crate::__spawn_children!($parent $next $($rest)*);
    };

    // ── Self-closing with id + props: Name(id: Marker, props...) ─────
    ($parent:ident $component:ident ( id : $marker:path, $($prop_name:ident : $prop_val:expr),* $(,)? )) => {
        {
            let mut _jsx_props = <$component as $crate::__JsxComponent>::Props::default();
            $(_jsx_props.$prop_name($prop_val);)*
            let _jsx_output = <$component as $crate::__JsxComponent>::build(_jsx_props);
            $crate::Spawnable::spawn_into(_jsx_output, $parent).insert($marker);
        }
    };

    // ── Self-closing with id only, followed by something ──────────
    ($parent:ident $component:ident ( id : $marker:path ) $next:tt $($rest:tt)*) => {
        {
            let _jsx_output = $crate::element! { $component };
            $crate::Spawnable::spawn_into(_jsx_output, $parent).insert($marker);
        }
        $crate::__spawn_children!($parent $next $($rest)*);
    };

    // ── Self-closing with id only: Name(id: Marker) ───────────────
    ($parent:ident $component:ident ( id : $marker:path )) => {
        {
            let _jsx_output = $crate::element! { $component };
            $crate::Spawnable::spawn_into(_jsx_output, $parent).insert($marker);
        }
    };

    // ── Self-closing with props: Name(props) ──────────────────────
    // We require the next token is NOT `{` to disambiguate from the
    // "child with braces" rule above.
    ($parent:ident $component:ident ( $($prop_name:ident : $prop_val:expr),* $(,)? ) $next:tt $($rest:tt)*) => {
        {
            let _jsx_output = $crate::element! {
                $component ( $($prop_name : $prop_val),* )
            };
            $crate::Spawnable::spawn_into(_jsx_output, $parent);
        }
        $crate::__spawn_children!($parent $next $($rest)*);
    };

    // ── Self-closing with props, at end of input ──────────────────
    ($parent:ident $component:ident ( $($prop_name:ident : $prop_val:expr),* $(,)? )) => {
        {
            let _jsx_output = $crate::element! {
                $component ( $($prop_name : $prop_val),* )
            };
            $crate::Spawnable::spawn_into(_jsx_output, $parent);
        }
    };

    // ── Self-closing, no props, followed by something ─────────────────
    ($parent:ident $component:ident $next:tt $($rest:tt)*) => {
        {
            let _jsx_output = $crate::element! { $component };
            $crate::Spawnable::spawn_into(_jsx_output, $parent);
        }
        $crate::__spawn_children!($parent $next $($rest)*);
    };

    // ── Self-closing, no props: Name ───────────────────────────────
    ($parent:ident $component:ident) => {
        {
            let _jsx_output = $crate::element! { $component };
            $crate::Spawnable::spawn_into(_jsx_output, $parent);
        }
    };
}

/// Internal: folds an `if` / `else if` / `else` chain from `element!` child
/// syntax into one Rust if-else statement.
///
/// The accumulator (`[$($acc:tt)*]`) collects already-translated
/// `if cond { ... } else` prefixes while the muncher walks the chain.
/// Siblings after the chain spawn unconditionally via `__spawn_children!`.
#[doc(hidden)]
#[macro_export]
macro_rules! __jsx_if_chain {
    // ── if let ... else if → accumulate, keep munching the chain ──
    ($parent:ident [$($acc:tt)*] if let $pat:pat = $expr:expr => { $($then:tt)* } else if $($rest:tt)*) => {
        $crate::__jsx_if_chain!($parent [
            $($acc)*
            if let $pat = $expr {
                $crate::__spawn_children!($parent $($then)*);
            } else
        ] if $($rest)*);
    };

    // ── if ... else if → accumulate, keep munching the chain ──────
    ($parent:ident [$($acc:tt)*] if $cond:expr => { $($then:tt)* } else if $($rest:tt)*) => {
        $crate::__jsx_if_chain!($parent [
            $($acc)*
            if $cond {
                $crate::__spawn_children!($parent $($then)*);
            } else
        ] if $($rest)*);
    };

    // ── if let ... else => { ... } — chain ends with an else ──────
    ($parent:ident [$($acc:tt)*] if let $pat:pat = $expr:expr => { $($then:tt)* } else => { $($els:tt)* } $($rest:tt)*) => {
        $($acc)*
        if let $pat = $expr {
            $crate::__spawn_children!($parent $($then)*);
        } else {
            $crate::__spawn_children!($parent $($els)*);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── if ... else => { ... } — chain ends with an else ──────────
    ($parent:ident [$($acc:tt)*] if $cond:expr => { $($then:tt)* } else => { $($els:tt)* } $($rest:tt)*) => {
        $($acc)*
        if $cond {
            $crate::__spawn_children!($parent $($then)*);
        } else {
            $crate::__spawn_children!($parent $($els)*);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── if let without else — chain ends ──────────────────────────
    ($parent:ident [$($acc:tt)*] if let $pat:pat = $expr:expr => { $($then:tt)* } $($rest:tt)*) => {
        $($acc)*
        if let $pat = $expr {
            $crate::__spawn_children!($parent $($then)*);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };

    // ── if without else — chain ends ──────────────────────────────
    ($parent:ident [$($acc:tt)*] if $cond:expr => { $($then:tt)* } $($rest:tt)*) => {
        $($acc)*
        if $cond {
            $crate::__spawn_children!($parent $($then)*);
        }
        $crate::__spawn_children!($parent $($rest)*);
    };
}
