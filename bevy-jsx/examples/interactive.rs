//! Interactive counter: marker components (`id:`) make JSX-spawned entities
//! addressable from ordinary Bevy systems — buttons mutate a counter label.
//!
//! Run with: `cargo run -p bevy-jsx --example interactive`

use bevy::prelude::*;
use bevy_jsx::{element, jsx_component};

// ── Components ────────────────────────────────────────────────────────────

jsx_component! {
    Column(ColumnProps) {
        row_gap: Val = Val::Px(12.0),
        background_color: Color = Color::NONE,
    }
    |{background_color, ..props}| {
        (
            Node {
                ..props,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(background_color),
        )
    }
}

jsx_component! {
    Btn(BtnProps) {
        label: String,
        background_color: Color = Color::srgb(0.25, 0.3, 0.4),
    }
    |props| {
        (
            Button,
            Node {
                width: Val::Px(160.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(props.background_color),
            children![(
                Text::new(props.label),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
            )],
        )
    }
}

// ── Markers & state ───────────────────────────────────────────────────────

#[derive(Component)]
struct CounterText;

#[derive(Component)]
struct IncButton;

#[derive(Component)]
struct DecButton;

#[derive(Resource, Default)]
struct Counter(i32);

// ── App ───────────────────────────────────────────────────────────────────

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Counter>()
        .add_systems(Startup, setup)
        .add_systems(Update, (on_click, update_counter_text))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let ui = element! {
        Column(background_color: Color::srgb(0.1, 0.1, 0.12)) {
            Label(id: CounterText, content: "0".into())
            Btn(id: IncButton, label: "+1".into())
            Btn(id: DecButton, label: "-1".into(), background_color: Color::srgb(0.4, 0.25, 0.25))
        }
    };
    ui.spawn(&mut commands);
}

jsx_component! {
    Label(LabelProps) {
        content: String,
    }
    |props| {
        (
            Text::new(props.content),
            TextFont { font_size: FontSize::Px(32.0), ..default() },
        )
    }
}

fn on_click(
    mut counter: ResMut<Counter>,
    inc_q: Query<&Interaction, (Changed<Interaction>, With<IncButton>)>,
    dec_q: Query<&Interaction, (Changed<Interaction>, With<DecButton>)>,
) {
    for interaction in &inc_q {
        if *interaction == Interaction::Pressed {
            counter.0 += 1;
        }
    }
    for interaction in &dec_q {
        if *interaction == Interaction::Pressed {
            counter.0 -= 1;
        }
    }
}

fn update_counter_text(counter: Res<Counter>, mut query: Query<&mut Text, With<CounterText>>) {
    if !counter.is_changed() {
        return;
    }
    for mut text in &mut query {
        **text = counter.0.to_string();
    }
}
