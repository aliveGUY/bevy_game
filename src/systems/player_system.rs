use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::systems::{ movement_system, Ground, MovementState, SkyboxHandle };

pub const CAMERA_DISTANCE: f32 = 10.0;
const CAMERA_HEIGHT: f32 = 5.0;

const PLAYER_HALF_HEIGHT: f32 = 0.5;

// Footprint “sensor” (fall only when whole footprint is off the edge)
const FOOT_HALF_X: f32 = 0.49;
const FOOT_HALF_Z: f32 = 0.49;
const FOOT_HALF_Y: f32 = 0.03;
const FOOT_BELOW_FEET: f32 = 0.01;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player);

        app.add_systems(FixedUpdate, (
            movement_system,
            apply_player_motion.after(movement_system),
            update_grounded_flag_and_snap.after(apply_player_motion),
        ));

        app.add_systems(Update, follow_player_camera);
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct FollowPlayerCamera;

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    // Start above where ground likely is; ground snap will correct on first tick.
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
            material: materials.add(Color::srgb(0.8, 0.8, 0.9)),
            transform: Transform::from_xyz(0.0, 2.0, 0.0),
            ..default()
        },
        Player,
        RigidBody::KinematicPositionBased,
    ));

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, CAMERA_HEIGHT, CAMERA_DISTANCE).looking_at(
                Vec3::ZERO,
                Vec3::Y
            ),
            ..default()
        },
        FollowPlayerCamera,
        SkyboxHandle(asset_server.load("skybox/skybox.ktx2")),
    ));
}

pub fn apply_player_motion(
    time: Res<Time>,
    st: Res<MovementState>,
    mut q: Query<&mut Transform, With<Player>>
) {
    let dt = time.delta_seconds();
    let Ok(mut t) = q.get_single_mut() else {
        return;
    };

    // Horizontal ALWAYS (movement_system decays to 0 while falling)
    t.translation.x += st.velocity.x * dt;
    t.translation.z += st.velocity.y * dt;

    // Vertical ONLY depends on falling flag and fall velocity
    if st.is_falling {
        t.translation.y += st.fall_vel_y * dt;
    }
}

/// 1) Detect grounded by footprint intersection vs Ground.
/// 2) If grounded: snap player y to Ground top surface + PLAYER_HALF_HEIGHT.
///    This removes the need for any constant GROUND_Y.
pub fn update_grounded_flag_and_snap(
    rapier: Res<RapierContext>,
    mut st: ResMut<MovementState>,
    // We need actual data for ground entities:
    ground_q: Query<(&GlobalTransform, &Collider), With<Ground>>,
    mut player_q: Query<(Entity, &GlobalTransform, &mut Transform), With<Player>>,
) {
    let Ok((player_e, gt, mut t)) = player_q.get_single_mut() else { return; };
    let pos = gt.translation();

    // Footprint box center at player feet
    let foot_center = Vec3::new(
        pos.x,
        (pos.y - PLAYER_HALF_HEIGHT) + FOOT_HALF_Y - FOOT_BELOW_FEET,
        pos.z,
    );

    let foot_shape = Collider::cuboid(FOOT_HALF_X, FOOT_HALF_Y, FOOT_HALF_Z);

    let filter = QueryFilter::default().exclude_collider(player_e);

    // Find all intersections, but only count Ground entities.
    let mut grounded = false;
    let mut best_top_y: Option<f32> = None;

    rapier.intersections_with_shape(
        foot_center,
        Quat::IDENTITY,
        &foot_shape,
        filter,
        |hit_entity| {
            let Ok((g_gt, g_col)) = ground_q.get(hit_entity) else {
                // not Ground => ignore
                return true; // keep searching
            };

            grounded = true;

            // Compute top surface Y for cuboid colliders (perfect for your box maps).
            // NOTE: This assumes the ground cuboids are not rotated.
            if let Some(cub) = g_col.as_cuboid() {
                let half_y = cub.half_extents().y;
                let top_y = g_gt.translation().y + half_y;

                best_top_y = Some(match best_top_y {
                    Some(cur) => cur.max(top_y),
                    None => top_y,
                });
            }

            true // keep searching (we want highest top_y under the footprint)
        },
    );

    st.is_falling = !grounded;

    // If grounded, snap to the best ground height.
    // This removes jitter and eliminates any need for a GROUND_Y constant.
    if grounded {
        if let Some(top_y) = best_top_y {
            t.translation.y = top_y + PLAYER_HALF_HEIGHT;
        }
    }
}

pub fn follow_player_camera(
    player_q: Query<&Transform, With<Player>>,
    mut cam_q: Query<&mut Transform, (With<FollowPlayerCamera>, Without<Player>)>
) {
    let Ok(player_t) = player_q.get_single() else {
        return;
    };
    let Ok(mut cam_t) = cam_q.get_single_mut() else {
        return;
    };

    let player_pos = player_t.translation;
    let offset = Vec3::new(0.0, CAMERA_HEIGHT, CAMERA_DISTANCE);

    cam_t.translation = player_pos + offset;
    cam_t.look_at(player_pos, Vec3::Y);
}
