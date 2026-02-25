use bevy::prelude::*;

use crate::systems::MovementState;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player);
        app.add_systems(Update, update_player);
    }
}

#[derive(Component)]
pub struct Player;

pub fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Player cube in the middle of the scene
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
            material: materials.add(Color::srgb(0.8, 0.8, 0.9)),
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..default()
        },
        Player,
    ));

    // A light so we can see the cube (optional but usually needed)
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // A camera looking at the origin (optional if you already have one)
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 6.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

pub fn update_player(
    time: Res<Time>,
    st: Res<MovementState>,
    mut q: Query<&mut Transform, With<Player>>,
) {
    // Apply velocity to player position
    let dt = time.delta_seconds();

    // Map your 2D movement velocity (x=right, y=forward) into 3D (x, z)
    let v3 = Vec3::new(st.velocity.x, 0.0, st.velocity.y);

    for mut t in &mut q {
        t.translation += v3 * dt;
    }
}