mod components;

use bevy::prelude::*;
use crate::{ systems::MovementState, ui::components::{ HeartbeatUiPlugin, HeartbeatValue } };
use components::HeartbeatBundle;

#[derive(Component)]
struct MovementHudText;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HeartbeatUiPlugin);
        app.add_systems(Startup, setup_ui);
        app.add_systems(Update, (interface_system, update_heartbeat));
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        TextBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                ..default()
            },
            text: Text::from_section("Pressed: (none)", TextStyle {
                font_size: 18.0,
                color: Color::BLACK,
                ..default()
            }),
            ..default()
        },
        MovementHudText,
    ));

    // heartbeat (top-right but below the text)
    commands.spawn(HeartbeatBundle {
        node: NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(40.0),
                right: Val::Px(10.0),

                width: Val::Px(200.0),
                height: Val::Px(40.0),

                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ..default()
        },
        ..default()
    });
}

fn interface_system(st: Res<MovementState>, mut q: Query<&mut Text, With<MovementHudText>>) {
    if !st.is_changed() {
        return;
    }

    let Ok(mut text) = q.get_single_mut() else {
        return;
    };

    text.sections[0].value = if st.pressed.is_empty() {
        "Pressed: (none)".to_string()
    } else {
        format!("Pressed: {}", st.pressed)
    };
}

fn update_heartbeat(st: Res<MovementState>, mut q: Query<&mut HeartbeatValue>) {
    let Ok(mut hb) = q.get_single_mut() else { return; };
    hb.0 = st.velocity.length();
}
