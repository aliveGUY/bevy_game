use bevy::prelude::*;

use crate::systems::ThirdPersonCamera;

/// Maximum running speed.
const RUN_SPEED: f32 = 6.0;

/// Maximum walking speed.
const WALK_SPEED: f32 = 3.0;

/// Time to reach the target speed during acceleration.
const ACCELERATION_DURATION: f32 = 0.5;

/// Shape strength of the acceleration curve.
/// Higher values make the start more aggressive.
const ACCELERATION_CURVE_STRENGTH: f32 = 6.0;

/// Time to smoothly decelerate from run speed down to walk speed.
const RUN_TO_WALK_DURATION: f32 = 0.5;

/// Shape of the run-to-walk deceleration curve.
/// 1.0 = linear, 2.0 = quadratic-like.
const RUN_TO_WALK_CURVATURE: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Accelerating,
    Walking,
    Running,
    Decelerating,
}

#[derive(Resource)]
pub struct MovementState {
    pub pressed: String,

    pub state: PlayerState,

    pub dir: Vec2,
    pub velocity: Vec2,
    pub speed: f32,

    pub is_run_to_walk_decelerating: bool,
    pub run_to_walk_t: f32,
    pub run_to_walk_start_speed: f32,

    pub hard_turn_dot: f32,
    pub soft_turn_dot: f32,

    pub soft_turn_speed_factor: f32,
    pub stop_epsilon: f32,

    pub hard_turn_hold_time: f32,
    hard_turn_active: bool,
    hard_turn_timer: f32,
    pending_dir: Vec2,

    accelerating: bool,
    t: f32,
    start_speed: f32,

    pub is_falling: bool,
    pub fall_decel: f32,
    pub fall_vel_y: f32,
    pub gravity: f32,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            pressed: String::from("Idle"),

            state: PlayerState::Idle,

            dir: Vec2::Y,
            velocity: Vec2::ZERO,
            speed: 0.0,

            is_run_to_walk_decelerating: false,
            run_to_walk_t: 0.0,
            run_to_walk_start_speed: 0.0,

            hard_turn_dot: -0.707,
            soft_turn_dot: 0.707,

            soft_turn_speed_factor: 0.5,
            stop_epsilon: 0.02,

            hard_turn_hold_time: 0.1,
            hard_turn_active: false,
            hard_turn_timer: 0.0,
            pending_dir: Vec2::ZERO,

            accelerating: false,
            t: 0.0,
            start_speed: 0.0,

            is_falling: false,
            fall_decel: 20.0,

            fall_vel_y: 0.0,
            gravity: -30.0,
        }
    }
}

#[inline]
fn acceleration_function(t: f32, duration: f32, curve_strength: f32) -> f32 {
    if duration <= 0.0 {
        return 1.0;
    }

    let u = (t / duration).clamp(0.0, 1.0);
    let raw = 1.0 - (-curve_strength * u).exp();
    let max_raw = 1.0 - (-curve_strength).exp();

    if max_raw <= f32::EPSILON {
        1.0
    } else {
        raw / max_raw
    }
}

#[inline]
fn deceleration_function(
    t: f32,
    start_speed: f32,
    walk_speed: f32,
    duration: f32,
    curvature: f32
) -> f32 {
    if duration <= 0.0 {
        return walk_speed;
    }

    let u = (t / duration).clamp(0.0, 1.0);
    walk_speed + (start_speed - walk_speed) * (1.0 - u).powf(curvature.max(1.0))
}

#[inline]
fn restart_curve(st: &mut MovementState, accelerating: bool) {
    if st.accelerating != accelerating {
        st.accelerating = accelerating;
        st.t = 0.0;
        st.start_speed = st.speed;
    }
}

#[inline]
fn shift_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

fn read_input_dir(keys: &ButtonInput<KeyCode>, camera_transform: &Transform) -> Vec2 {
    let mut raw = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        raw.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        raw.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        raw.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        raw.x -= 1.0;
    }

    if raw == Vec2::ZERO {
        return Vec2::ZERO;
    }

    let raw = raw.normalize();

    let forward3 = camera_transform.forward();
    let right3 = camera_transform.right();

    let forward = Vec2::new(forward3.x, forward3.z).normalize_or_zero();
    let right = Vec2::new(right3.x, right3.z).normalize_or_zero();

    let world = forward * raw.y + right * raw.x;
    world.normalize_or_zero()
}

fn player_state_string(state: PlayerState) -> String {
    match state {
        PlayerState::Idle => "Idle".to_string(),
        PlayerState::Accelerating => "Accelerating".to_string(),
        PlayerState::Walking => "Walking".to_string(),
        PlayerState::Running => "Running".to_string(),
        PlayerState::Decelerating => "Decelerating".to_string(),
    }
}

#[inline]
fn set_player_state(st: &mut MovementState, state: PlayerState) {
    st.state = state;
    st.pressed = player_state_string(state);
}

pub fn movement_system(
    time: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    cam_q: Query<&Transform, With<ThirdPersonCamera>>,
    mut st: ResMut<MovementState>
) {
    let dt = time.delta_seconds();

    let max_fall_speed = -3.0 * RUN_SPEED;

    if st.is_falling {
        st.speed = (st.speed - st.fall_decel * dt).max(0.0);

        if st.speed <= st.stop_epsilon {
            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
            set_player_state(&mut st, PlayerState::Idle);
        } else {
            let d = st.dir.normalize_or_zero();
            st.velocity = d * st.speed;
            set_player_state(&mut st, PlayerState::Decelerating);
        }

        st.fall_vel_y += st.gravity * dt;
        if st.fall_vel_y < max_fall_speed {
            st.fall_vel_y = max_fall_speed;
        }

        st.accelerating = false;
        st.t = 0.0;
        st.start_speed = st.speed;

        st.is_run_to_walk_decelerating = false;
        st.run_to_walk_t = 0.0;
        st.run_to_walk_start_speed = 0.0;

        st.hard_turn_active = false;
        st.hard_turn_timer = 0.0;
        st.pending_dir = Vec2::ZERO;

        return;
    }

    st.fall_vel_y = 0.0;

    let desired_dir = if let Ok(cam_t) = cam_q.get_single() {
        read_input_dir(&keys, cam_t)
    } else {
        Vec2::ZERO
    };

    let has_input = desired_dir != Vec2::ZERO;
    let wants_run = has_input && shift_pressed(&keys);
    let target_speed = if wants_run { RUN_SPEED } else { WALK_SPEED };

    let moving = st.speed > st.stop_epsilon;
    let current_dir = if moving { st.dir.normalize_or_zero() } else { Vec2::ZERO };

    if st.hard_turn_active {
        if !has_input {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;
            st.pending_dir = Vec2::ZERO;

            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
            set_player_state(&mut st, PlayerState::Idle);

            st.accelerating = false;
            st.t = 0.0;
            st.start_speed = 0.0;

            st.is_run_to_walk_decelerating = false;
            st.run_to_walk_t = 0.0;
            st.run_to_walk_start_speed = 0.0;
            return;
        }

        st.pending_dir = desired_dir;
        st.hard_turn_timer += dt;

        st.speed = 0.0;
        st.velocity = Vec2::ZERO;
        set_player_state(&mut st, PlayerState::Idle);

        if st.hard_turn_timer >= st.hard_turn_hold_time {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;

            st.dir = st.pending_dir;
            restart_curve(&mut st, true);
        }
        return;
    }

    let mut soft_turn = false;
    if moving && has_input {
        let dot = current_dir.dot(desired_dir);

        if dot <= st.hard_turn_dot {
            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
            set_player_state(&mut st, PlayerState::Idle);

            st.accelerating = false;
            st.t = 0.0;
            st.start_speed = 0.0;

            st.is_run_to_walk_decelerating = false;
            st.run_to_walk_t = 0.0;
            st.run_to_walk_start_speed = 0.0;

            st.hard_turn_active = true;
            st.hard_turn_timer = 0.0;
            st.pending_dir = desired_dir;
            return;
        } else if dot <= st.soft_turn_dot {
            soft_turn = true;
        }
    }

    if has_input {
        st.dir = desired_dir;
    }

    let should_start_run_to_walk =
        st.speed > WALK_SPEED + st.stop_epsilon &&
        !st.is_run_to_walk_decelerating &&
        ((has_input && !wants_run) || !has_input);

    if should_start_run_to_walk {
        st.is_run_to_walk_decelerating = true;
        st.run_to_walk_t = 0.0;
        st.run_to_walk_start_speed = st.speed.min(RUN_SPEED);

        st.accelerating = false;
        st.t = 0.0;
        st.start_speed = st.speed;
    }

    let mut speed = if st.is_run_to_walk_decelerating {
        st.run_to_walk_t += dt;

        let speed = deceleration_function(
            st.run_to_walk_t,
            st.run_to_walk_start_speed,
            WALK_SPEED,
            RUN_TO_WALK_DURATION,
            RUN_TO_WALK_CURVATURE
        );

        let reached_walk =
            st.run_to_walk_t >= RUN_TO_WALK_DURATION || speed <= WALK_SPEED + st.stop_epsilon;

        if reached_walk {
            st.is_run_to_walk_decelerating = false;
            st.run_to_walk_t = 0.0;
            st.run_to_walk_start_speed = 0.0;

            if has_input {
                WALK_SPEED
            } else {
                0.0
            }
        } else {
            speed
        }
    } else if has_input {
        restart_curve(&mut st, true);
        st.t += dt;

        let alpha = acceleration_function(
            st.t,
            ACCELERATION_DURATION,
            ACCELERATION_CURVE_STRENGTH
        ).clamp(0.0, 1.0);

        st.start_speed + (target_speed - st.start_speed) * alpha
    } else {
        0.0
    };

    if soft_turn {
        speed *= st.soft_turn_speed_factor;
    }

    speed = speed.clamp(0.0, RUN_SPEED);

    if speed < st.stop_epsilon {
        speed = 0.0;
    }

    st.speed = speed;
    st.velocity = if speed > 0.0 { st.dir * speed } else { Vec2::ZERO };

    let next_state = if st.is_run_to_walk_decelerating {
        PlayerState::Decelerating
    } else if !has_input || speed <= st.stop_epsilon {
        PlayerState::Idle
    } else if wants_run && speed < RUN_SPEED - st.stop_epsilon {
        PlayerState::Accelerating
    } else if wants_run {
        PlayerState::Running
    } else {
        PlayerState::Walking
    };

    set_player_state(&mut st, next_state);
}
