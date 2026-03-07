use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::systems::{ movement_system, Ground, MovementState };

const PLAYER_HALF_HEIGHT: f32 = 0.5;

// Footprint “sensor” (fall only when whole footprint is off the edge)
const FOOT_HALF_X: f32 = 0.3;
const FOOT_HALF_Z: f32 = 0.3;
const FOOT_HALF_Y: f32 = 0.03;
const FOOT_BELOW_FEET: f32 = 0.01;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player);

        app.add_systems(FixedUpdate, (
            update_grounded_flag_and_snap,
            movement_system.after(update_grounded_flag_and_snap),
            apply_player_motion.after(movement_system),
        ));
    }
}

#[derive(Component)]
pub struct Player;

pub fn setup_player(
    mut commands: Commands,
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
    ));
}

pub fn apply_player_motion(
    time: Res<Time<Fixed>>,
    st: Res<MovementState>,
    mut q: Query<&mut Transform, With<Player>>
) {
    let dt = time.delta_seconds();
    let Ok(mut t) = q.get_single_mut() else {
        return;
    };

    t.translation.x += st.velocity.x * dt;
    t.translation.z += st.velocity.y * dt;

    if st.is_falling {
        t.translation.y += st.fall_vel_y * dt;
    }

    if st.velocity.length_squared() > 0.0001 {
        let move_dir = Vec3::new(st.velocity.x, 0.0, st.velocity.y).normalize();
        let yaw = move_dir.x.atan2(move_dir.z);
        t.rotation = Quat::from_rotation_y(yaw);
    }
}

/// 1) Detect grounded by footprint intersection vs Ground.
/// 2) If grounded: snap player y to Ground top surface + PLAYER_HALF_HEIGHT.
///    This removes the need for any constant GROUND_Y.
pub fn update_grounded_flag_and_snap(
    rapier: Res<RapierContext>,
    mut st: ResMut<MovementState>,
    ground_q: Query<(&GlobalTransform, &Collider), With<Ground>>,
    mut player_q: Query<(Entity, &mut Transform), With<Player>>
) {
    let Ok((player_e, mut t)) = player_q.get_single_mut() else {
        return;
    };

    let pos = t.translation;

    let foot_center = Vec3::new(
        pos.x,
        pos.y - PLAYER_HALF_HEIGHT + FOOT_HALF_Y - FOOT_BELOW_FEET,
        pos.z
    );

    let foot_shape = Collider::cuboid(FOOT_HALF_X, FOOT_HALF_Y, FOOT_HALF_Z);
    let filter = QueryFilter::default().exclude_collider(player_e);

    let mut grounded = false;
    let mut best_top_y: Option<f32> = None;

    rapier.intersections_with_shape(foot_center, Quat::IDENTITY, &foot_shape, filter, |hit_entity| {
        let Ok((g_gt, g_col)) = ground_q.get(hit_entity) else {
            return true;
        };

        grounded = true;

        if let Some(cub) = g_col.as_cuboid() {
            let half_y = cub.half_extents().y;
            let top_y = g_gt.translation().y + half_y;

            best_top_y = Some(match best_top_y {
                Some(cur) => cur.max(top_y),
                None => top_y,
            });
        }

        true
    });

    st.is_falling = !grounded;

    if let Some(top_y) = best_top_y {
        let target_y = top_y + PLAYER_HALF_HEIGHT;
        if grounded && (t.translation.y - target_y).abs() > 0.0001 {
            t.translation.y = target_y;
        }
    }
}
