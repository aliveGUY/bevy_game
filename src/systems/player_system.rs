use bevy::{ animation::graph::{ AnimationGraph, AnimationNodeIndex }, prelude::* };
use bevy_rapier3d::prelude::*;

use crate::systems::{ movement_system, Ground, MovementState };

const PLAYER_HALF_HEIGHT: f32 = 0.5;

// Footprint sensor
const FOOT_HALF_X: f32 = 0.3;
const FOOT_HALF_Z: f32 = 0.3;
const FOOT_HALF_Y: f32 = 0.03;
const FOOT_BELOW_FEET: f32 = 0.01;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAnimations>();
        app.init_resource::<CurrentPlayerAnimation>();

        app.add_systems(Startup, setup_player);

        app.add_systems(Update, (
            bind_animation_graph_to_players,
            update_player_animation.after(bind_animation_graph_to_players),
        ));

        app.add_systems(FixedUpdate, (
            update_grounded_flag_and_snap,
            movement_system.after(update_grounded_flag_and_snap),
            apply_player_motion.after(movement_system),
        ));
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Resource)]
pub struct PlayerAnimations {
    pub graph: Handle<AnimationGraph>,
    pub idle: AnimationNodeIndex,
    pub walk: AnimationNodeIndex,
}

impl FromWorld for PlayerAnimations {
    fn from_world(world: &mut World) -> Self {
        let idle_clip: Handle<AnimationClip> = {
            let asset_server = world.resource::<AssetServer>();
            asset_server.load("map/dummy.glb#Animation0")
        };

        let walk_clip: Handle<AnimationClip> = {
            let asset_server = world.resource::<AssetServer>();
            asset_server.load("map/dummy.glb#Animation1")
        };

        let mut graph = AnimationGraph::new();
        let idle = graph.add_clip(idle_clip, 1.0, graph.root);
        let walk = graph.add_clip(walk_clip, 1.0, graph.root);

        let graph_handle = {
            let mut graphs = world.resource_mut::<Assets<AnimationGraph>>();
            graphs.add(graph)
        };

        Self {
            graph: graph_handle,
            idle,
            walk,
        }
    }
}

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum CurrentPlayerAnimation {
    #[default]
    None,
    Idle,
    Walk,
}

pub fn setup_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_xyz(0.0, 2.0, 0.0),
                ..default()
            },
            Player,
        ))
        .with_children(|parent| {
            parent.spawn(SceneBundle {
                scene: asset_server.load("map/dummy.glb#Scene0"),
                transform: Transform::default(),
                ..default()
            });
        });
}

pub fn bind_animation_graph_to_players(
    animations: Res<PlayerAnimations>,
    mut commands: Commands,
    players: Query<Entity, Added<AnimationPlayer>>
) {
    for entity in &players {
        commands.entity(entity).insert(animations.graph.clone());
    }
}

pub fn update_player_animation(
    st: Res<MovementState>,
    animations: Res<PlayerAnimations>,
    mut current: ResMut<CurrentPlayerAnimation>,
    mut players: Query<&mut AnimationPlayer>
) {
    let desired = if st.velocity.length_squared() > 0.0001 && !st.is_falling {
        CurrentPlayerAnimation::Walk
    } else {
        CurrentPlayerAnimation::Idle
    };

    if *current == desired {
        return;
    }

    let node = match desired {
        CurrentPlayerAnimation::Idle => animations.idle,
        CurrentPlayerAnimation::Walk => animations.walk,
        CurrentPlayerAnimation::None => {
            return;
        }
    };

    for mut player in &mut players {
        player.stop_all();
        player.play(node).repeat();
    }

    *current = desired;
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
