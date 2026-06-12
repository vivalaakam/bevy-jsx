//! Static layout showcasing every `bevy-jsx` syntax form:
//! per-field defaults, the `..props` spread, children, markers and splice.
//!
//! Run with: `cargo run -p bevy-jsx --example basic`

use bevy::prelude::*;
use bevy_jsx::{element, jsx_component};

// ── Components ────────────────────────────────────────────────────────────

jsx_component! {
    /// A generic flex container. Props that mirror `Node` fields flow into it
    /// through the `..props` spread; `background_color` is extracted into its
    /// own component.
    Container(ContainerProps) {
        width: Val = Val::Auto,
        height: Val = Val::Auto,
        flex_grow: f32 = 0.0,
        flex_direction: FlexDirection = FlexDirection::Row,
        justify_content: JustifyContent = JustifyContent::Start,
        align_items: AlignItems = AlignItems::Stretch,
        row_gap: Val = Val::Auto,
        column_gap: Val = Val::Auto,
        padding: UiRect = UiRect::default(),
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
    /// A text label using the whole-props closure form.
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
    /// A flex-grow spacer that pushes siblings apart.
    Spacer(SpacerProps) {
        flex_grow: f32 = 1.0,
    }
    |props| {
        Node {
            flex_grow: props.flex_grow,
            ..default()
        }
    }
}

// ── Markers ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct TitleText;

// ── App ───────────────────────────────────────────────────────────────────

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Splice: build a card once, interpolate it into the tree with #(card).
    let card = element! {
        Container(
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(4.0),
            background_color: Color::srgb(0.18, 0.2, 0.25),
        ) {
            Label(content: "Card".into(), font_size: 18.0)
            Label(content: "built separately, spliced in".into(), font_size: 12.0, color: Color::srgb(0.6, 0.6, 0.65))
        }
    };

    let ui = element! {
        Container(
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(16.0),
            padding: UiRect::all(Val::Px(24.0)),
            background_color: Color::srgb(0.1, 0.1, 0.12),
        ) {
            // Header row: marker `id:` makes the title queryable.
            Container(align_items: AlignItems::Center, column_gap: Val::Px(8.0)) {
                Label(id: TitleText, content: "bevy-jsx".into(), font_size: 28.0)
                Spacer
                Label(content: "basic example".into(), color: Color::srgb(0.5, 0.5, 0.55))
            }
            #(card)
            // Self-closing child with default props.
            Spacer
            Label(content: "footer".into(), font_size: 12.0)
        }
    };
    ui.spawn(&mut commands);
}
