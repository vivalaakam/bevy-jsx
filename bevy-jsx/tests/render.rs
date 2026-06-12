//! Integration tests: spawn `element!` trees into a real `World` and assert
//! the resulting entity hierarchy and component values.

use bevy::ecs::hierarchy::Children;
use bevy::prelude::*;
use bevy_jsx::{Spawnable, element, jsx_component};

// ── Test components ───────────────────────────────────────────────────────

jsx_component! {
    /// Whole-props closure form.
    Label(LabelProps) {
        content: String,
        font_size: f32 = 16.0,
        color: Color = Color::WHITE,
    }
    |props| {
        (
            Text::new(props.content),
            TextFont {
                font_size: FontSize::Px(props.font_size),
                ..default()
            },
            TextColor(props.color),
        )
    }
}

jsx_component! {
    /// Destructuring form with `..props` spread into `Node`.
    Panel(PanelProps) {
        width: Val = Val::Px(100.0),
        height: Val = Val::Px(50.0),
        flex_direction: FlexDirection = FlexDirection::Column,
        row_gap: Val = Val::Auto,
        background_color: Color = Color::NONE,
    }
    |{background_color, ..props}| {
        (
            Node {
                ..props,
                ..default()
            },
            BackgroundColor(background_color),
        )
    }
}

jsx_component! {
    /// Destructuring form with a non-`Copy` extracted prop (partial move).
    TitledBox(TitledBoxProps) {
        title: String,
        width: Val = Val::Px(10.0),
    }
    |{title, ..props}| {
        (
            Node {
                ..props,
                ..default()
            },
            Text::new(title),
        )
    }
}

jsx_component! {
    /// Destructuring form without a rest binding.
    Tinted(TintedProps) {
        color: Color = Color::BLACK,
    }
    |{color}| {
        (Node::default(), BackgroundColor(color))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Spawn any `element!` output (self-closing or with children) into a fresh
/// world, apply commands.
fn spawn_tree(el: impl Spawnable) -> (World, Entity) {
    let mut world = World::new();
    let entity = {
        let mut commands = world.commands();
        el.spawn_top(&mut commands).id()
    };
    world.flush();
    (world, entity)
}

fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .entity(entity)
        .get::<Children>()
        .map(|c| c.iter().collect())
        .unwrap_or_default()
}

// ── Props: defaults & overrides ───────────────────────────────────────────

#[test]
fn self_closing_uses_defaults() {
    let (world, entity) = spawn_tree(element! { Panel });

    let node = world.entity(entity).get::<Node>().unwrap();
    assert_eq!(node.width, Val::Px(100.0));
    assert_eq!(node.height, Val::Px(50.0));
    assert_eq!(node.flex_direction, FlexDirection::Column);

    let bg = world.entity(entity).get::<BackgroundColor>().unwrap();
    assert_eq!(bg.0, Color::NONE);
}

#[test]
fn props_override_defaults() {
    let (world, entity) = spawn_tree(element! {
        Panel(width: Val::Percent(42.0), background_color: Color::WHITE)
    });

    let node = world.entity(entity).get::<Node>().unwrap();
    // Overridden prop flows through the `..props` spread.
    assert_eq!(node.width, Val::Percent(42.0));
    // Untouched props keep their declared defaults.
    assert_eq!(node.height, Val::Px(50.0));
    // Extracted prop lands in its own component.
    let bg = world.entity(entity).get::<BackgroundColor>().unwrap();
    assert_eq!(bg.0, Color::WHITE);
}

#[test]
fn spread_does_not_leak_extracted_fields() {
    // `background_color` is extracted, so the spread must not try to set it
    // on `Node` (it is not a Node field — this test failing to compile would
    // be the actual regression).
    let (world, entity) = spawn_tree(element! {
        Panel(background_color: Color::srgb(1.0, 0.0, 0.0))
    });
    let bg = world.entity(entity).get::<BackgroundColor>().unwrap();
    assert_eq!(bg.0, Color::srgb(1.0, 0.0, 0.0));
}

#[test]
fn destructured_non_copy_prop_partial_move() {
    let (world, entity) = spawn_tree(element! {
        TitledBox(title: "Inventory".into(), width: Val::Px(7.0))
    });

    assert_eq!(world.entity(entity).get::<Text>().unwrap().0, "Inventory");
    assert_eq!(
        world.entity(entity).get::<Node>().unwrap().width,
        Val::Px(7.0)
    );
}

#[test]
fn destructure_without_rest() {
    let (world, entity) = spawn_tree(element! { Tinted(color: Color::srgb(0.0, 1.0, 0.0)) });
    let bg = world.entity(entity).get::<BackgroundColor>().unwrap();
    assert_eq!(bg.0, Color::srgb(0.0, 1.0, 0.0));
}

#[test]
fn whole_props_form_still_works() {
    let (world, entity) = spawn_tree(element! { Label(content: "hp".into(), font_size: 22.0) });

    assert_eq!(world.entity(entity).get::<Text>().unwrap().0, "hp");
    assert_eq!(
        world.entity(entity).get::<TextFont>().unwrap().font_size,
        FontSize::Px(22.0)
    );
    assert_eq!(
        world.entity(entity).get::<TextColor>().unwrap().0,
        Color::WHITE
    );
}

// ── Children hierarchy ────────────────────────────────────────────────────

#[test]
fn children_spawn_in_order() {
    let el = element! {
        Panel(row_gap: Val::Px(4.0)) {
            Label(content: "first".into())
            Label(content: "second".into())
            Label(content: "third".into())
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert_eq!(kids.len(), 3);
    let texts: Vec<&str> = kids
        .iter()
        .map(|&e| world.entity(e).get::<Text>().unwrap().0.as_str())
        .collect();
    assert_eq!(texts, ["first", "second", "third"]);
}

#[test]
fn nested_children() {
    let el = element! {
        Panel {
            Panel(width: Val::Px(20.0)) {
                Label(content: "deep".into())
            }
            Label(content: "shallow".into())
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert_eq!(kids.len(), 2);

    let inner = kids[0];
    assert_eq!(
        world.entity(inner).get::<Node>().unwrap().width,
        Val::Px(20.0)
    );
    let inner_kids = children_of(&world, inner);
    assert_eq!(inner_kids.len(), 1);
    assert_eq!(world.entity(inner_kids[0]).get::<Text>().unwrap().0, "deep");

    assert_eq!(world.entity(kids[1]).get::<Text>().unwrap().0, "shallow");
}

#[test]
fn self_closing_child_without_props() {
    let el = element! {
        Panel {
            Tinted
            Label(content: "x".into())
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert_eq!(kids.len(), 2);
    assert!(world.entity(kids[0]).get::<BackgroundColor>().is_some());
}

// ── Marker components (`id:`) ─────────────────────────────────────────────

#[derive(Component)]
struct ConfirmButton;

#[derive(Component)]
struct Sidebar;

#[test]
fn id_inserts_marker_on_self_closing_child() {
    let el = element! {
        Panel {
            Label(id: ConfirmButton, content: "OK".into())
            Label(content: "Cancel".into())
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert!(world.entity(kids[0]).get::<ConfirmButton>().is_some());
    assert!(world.entity(kids[1]).get::<ConfirmButton>().is_none());
}

#[test]
fn id_inserts_marker_on_child_with_children() {
    let el = element! {
        Panel {
            Panel(id: Sidebar, width: Val::Px(200.0)) {
                Label(content: "menu".into())
            }
        }
    };
    let (world, root) = spawn_tree(el);

    let sidebar = children_of(&world, root)[0];
    assert!(world.entity(sidebar).get::<Sidebar>().is_some());
    assert_eq!(
        world.entity(sidebar).get::<Node>().unwrap().width,
        Val::Px(200.0)
    );
    let inner = children_of(&world, sidebar);
    assert_eq!(world.entity(inner[0]).get::<Text>().unwrap().0, "menu");
}

// ── Conditional rendering (`if` / `if let` / `else if` / `else`) ──────────

fn texts_of(world: &World, entity: Entity) -> Vec<String> {
    children_of(world, entity)
        .iter()
        .filter_map(|&e| world.entity(e).get::<Text>().map(|t| t.0.clone()))
        .collect()
}

#[test]
fn if_true_renders_children() {
    let el = element! {
        Panel {
            if true => { Label(content: "shown".into()) }
            Label(content: "after".into())
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["shown", "after"]);
}

#[test]
fn if_false_skips_children() {
    let el = element! {
        Panel {
            if false => { Label(content: "hidden".into()) }
            Label(content: "after".into())
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["after"]);
}

#[test]
fn if_else_renders_else_branch() {
    let el = element! {
        Panel {
            if false => { Label(content: "then".into()) }
            else => { Label(content: "else".into()) }
            Label(content: "after".into())
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["else", "after"]);
}

#[test]
fn else_if_chain_picks_matching_branch() {
    let value = 2;
    let el = element! {
        Panel {
            if value == 1 => { Label(content: "one".into()) }
            else if value == 2 => { Label(content: "two".into()) }
            else if value == 3 => { Label(content: "three".into()) }
            else => { Label(content: "other".into()) }
            Label(content: "after".into())
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["two", "after"]);
}

#[test]
fn if_let_some_renders_with_binding() {
    let title: Option<String> = Some("bound".into());
    let el = element! {
        Panel {
            if let Some(t) = title.clone() => { Label(content: t) }
            else => { Label(content: "fallback".into()) }
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["bound"]);
}

#[test]
fn if_let_none_renders_else() {
    let title: Option<String> = None;
    let el = element! {
        Panel {
            if let Some(t) = title.clone() => { Label(content: t) }
            else => { Label(content: "fallback".into()) }
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["fallback"]);
}

// ── Fragment ──────────────────────────────────────────────────────────────

#[test]
fn fragment_flattens_children_into_parent() {
    let el = element! {
        Panel {
            Label(content: "a".into())
            Fragment {
                Label(content: "b".into())
                Label(content: "c".into())
            }
            Label(content: "d".into())
        }
    };
    let (world, root) = spawn_tree(el);
    // Fragment must not introduce a wrapper node.
    assert_eq!(texts_of(&world, root), ["a", "b", "c", "d"]);
}

// ── Composite components (body returns an `element!` tree) ───────────────

jsx_component! {
    /// Component whose body is itself an `element!` tree with children —
    /// the `HeroCard` pattern.
    Card(CardProps) {
        title: String,
        width: Val = Val::Px(120.0),
    }
    |{title, width}| {
        element! {
            Panel(width: width) {
                Label(content: title)
                Label(content: "body".into())
            }
        }
    }
}

#[test]
fn composite_component_spawns_internal_tree() {
    let (world, root) = spawn_tree(element! { Card(title: "hero".into()) });

    assert_eq!(
        world.entity(root).get::<Node>().unwrap().width,
        Val::Px(120.0)
    );
    assert_eq!(texts_of(&world, root), ["hero", "body"]);
}

#[test]
fn composite_component_as_child() {
    let el = element! {
        Panel {
            Label(content: "before".into())
            Card(title: "nested".into(), width: Val::Px(60.0))
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert_eq!(kids.len(), 2);
    let card = kids[1];
    assert_eq!(
        world.entity(card).get::<Node>().unwrap().width,
        Val::Px(60.0)
    );
    assert_eq!(texts_of(&world, card), ["nested", "body"]);
}

#[test]
fn composite_component_accepts_extra_children() {
    // Children passed at the call site append after the internal tree.
    let el = element! {
        Card(title: "t".into()) {
            Label(content: "extra".into())
        }
    };
    let (world, root) = spawn_tree(el);
    assert_eq!(texts_of(&world, root), ["t", "body", "extra"]);
}

#[test]
fn composite_component_with_id_marker() {
    let el = element! {
        Panel {
            Card(id: Sidebar, title: "marked".into())
        }
    };
    let (world, root) = spawn_tree(el);
    let card = children_of(&world, root)[0];
    assert!(world.entity(card).get::<Sidebar>().is_some());
    assert_eq!(texts_of(&world, card), ["marked", "body"]);
}

// ── Required props ────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "missing required prop `content` for `Label`")]
fn missing_required_prop_panics_with_message() {
    let (_world, _entity) = spawn_tree(element! { Label });
}

// ── Splice (`#(expr)`) ────────────────────────────────────────────────────

#[test]
fn splice_spawns_prebuilt_element() {
    let prebuilt = element! {
        Panel(width: Val::Px(33.0)) {
            Label(content: "spliced".into())
        }
    };
    let el = element! {
        Panel {
            Label(content: "before".into())
            #(prebuilt)
            Label(content: "after".into())
        }
    };
    let (world, root) = spawn_tree(el);

    let kids = children_of(&world, root);
    assert_eq!(kids.len(), 3);
    assert_eq!(world.entity(kids[0]).get::<Text>().unwrap().0, "before");
    assert_eq!(
        world.entity(kids[1]).get::<Node>().unwrap().width,
        Val::Px(33.0)
    );
    assert_eq!(world.entity(kids[2]).get::<Text>().unwrap().0, "after");
    let spliced_kids = children_of(&world, kids[1]);
    assert_eq!(
        world.entity(spliced_kids[0]).get::<Text>().unwrap().0,
        "spliced"
    );
}
