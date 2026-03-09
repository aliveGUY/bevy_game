use std::time::Duration;

use bevy::{ animation::graph::{ AnimationGraph, AnimationNodeIndex }, prelude::*, gltf::Gltf };
use bevy_rapier3d::prelude::*;

use crate::systems::{ movement_system, Ground, MovementState, PlayerState };

const PLAYER_HALF_HEIGHT: f32 = 0.5;

// Footprint sensor
const FOOT_HALF_X: f32 = 0.3;
const FOOT_HALF_Z: f32 = 0.3;
const FOOT_HALF_Y: f32 = 0.03;
const FOOT_BELOW_FEET: f32 = 0.01;

// Animation blending
const ANIMATION_BLEND_DURATION_SECS: f32 = 0.2;

// GLTF animation names from Blender.
const IDLE_ANIMATION_NAME: &str = "Idle";
const WALK_ANIMATION_NAME: &str = "Walk";
const RUN_ANIMATION_NAME: &str = "Run";
const ACCELERATION_ANIMATION_NAME: &str = "Acceleration";
const DECELERATION_ANIMATION_NAME: &str = "Decelaration";

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAnimations>();
        app.init_resource::<CurrentPlayerAnimation>();

        app.add_systems(Startup, setup_player);

        app.add_systems(Update, (
            initialize_player_animations_once,
            bind_animation_graph_to_players.after(initialize_player_animations_once),
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

#[derive(Resource, Default)]
pub struct PlayerAnimations {
    pub gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    pub idle: Option<AnimationNodeIndex>,
    pub walk: Option<AnimationNodeIndex>,
    pub run: Option<AnimationNodeIndex>,
    pub acceleration: Option<AnimationNodeIndex>,
    pub deceleration: Option<AnimationNodeIndex>,
    pub initialized: bool,
    pub warned_missing_required: bool,
}

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum CurrentPlayerAnimation {
    #[default]
    Idle,
    Walk,
    Run,
    Acceleration,
    Deceleration,
}

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut animations: ResMut<PlayerAnimations>
) {
    animations.gltf = asset_server.load("map/dummy.glb");

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

pub fn initialize_player_animations_once(
    mut animations: ResMut<PlayerAnimations>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>
) {
    if animations.initialized {
        return;
    }

    let Some(gltf) = gltfs.get(&animations.gltf) else {
        return;
    };

    let mut graph = AnimationGraph::new();

    let mut idle = None;
    let mut walk = None;
    let mut run = None;
    let mut acceleration = None;
    let mut deceleration = None;

    for (name, clip_handle) in &gltf.named_animations {
        info!("GLTF animation found: '{}'", name);

        match name.as_ref() {
            IDLE_ANIMATION_NAME => {
                idle = Some(graph.add_clip(clip_handle.clone(), 1.0, graph.root));
            }
            WALK_ANIMATION_NAME => {
                walk = Some(graph.add_clip(clip_handle.clone(), 1.0, graph.root));
            }
            RUN_ANIMATION_NAME => {
                run = Some(graph.add_clip(clip_handle.clone(), 1.0, graph.root));
            }
            ACCELERATION_ANIMATION_NAME => {
                acceleration = Some(graph.add_clip(clip_handle.clone(), 1.0, graph.root));
            }
            DECELERATION_ANIMATION_NAME => {
                deceleration = Some(graph.add_clip(clip_handle.clone(), 1.0, graph.root));
            }
            other => {
                warn!("Unexpected animation found in GLTF: '{}'", other);
            }
        }
    }

    if !animations.warned_missing_required {
        if idle.is_none() {
            warn!("Missing required animation '{}'.", IDLE_ANIMATION_NAME);
        }
        if walk.is_none() {
            warn!("Missing required animation '{}'.", WALK_ANIMATION_NAME);
        }
        if run.is_none() {
            warn!("Missing required animation '{}'.", RUN_ANIMATION_NAME);
        }
        if acceleration.is_none() {
            warn!("Missing required animation '{}'.", ACCELERATION_ANIMATION_NAME);
        }
        if deceleration.is_none() {
            warn!("Missing required animation '{}'.", DECELERATION_ANIMATION_NAME);
        }
    }

    animations.warned_missing_required = true;

    let graph_handle = graphs.add(graph);

    animations.graph = Some(graph_handle);
    animations.idle = idle;
    animations.walk = walk;
    animations.run = run;
    animations.acceleration = acceleration;
    animations.deceleration = deceleration;
    animations.initialized = true;
}

pub fn bind_animation_graph_to_players(
    animations: Res<PlayerAnimations>,
    mut commands: Commands,
    players: Query<Entity, Added<AnimationPlayer>>
) {
    let Some(graph) = animations.graph.clone() else {
        return;
    };

    for entity in &players {
        commands.entity(entity).insert((graph.clone(), AnimationTransitions::new()));
    }
}

pub fn update_player_animation(
    st: Res<MovementState>,
    animations: Res<PlayerAnimations>,
    mut current: ResMut<CurrentPlayerAnimation>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>
) {
    let desired = match st.state {
        PlayerState::Idle => CurrentPlayerAnimation::Idle,
        PlayerState::Accelerating => CurrentPlayerAnimation::Acceleration,
        PlayerState::Walking => CurrentPlayerAnimation::Walk,
        PlayerState::Running => CurrentPlayerAnimation::Run,
        PlayerState::Decelerating => CurrentPlayerAnimation::Deceleration,
    };

    if *current != desired {
        info!("Animation transition: {:?} -> {:?}", *current, desired);
    }

    if *current == desired {
        return;
    }

    let node = match desired {
        CurrentPlayerAnimation::Idle => animations.idle,
        CurrentPlayerAnimation::Walk => animations.walk,
        CurrentPlayerAnimation::Run => animations.run,
        CurrentPlayerAnimation::Acceleration => animations.acceleration,
        CurrentPlayerAnimation::Deceleration => animations.deceleration,
    };

    let Some(node) = node else {
        warn!("Requested animation {:?}, but no matching clip was loaded.", desired);
        return;
    };

    let blend_duration = Duration::from_secs_f32(ANIMATION_BLEND_DURATION_SECS);

    for (mut player, mut transitions) in &mut players {
        transitions.play(&mut player, node, blend_duration).repeat();
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
