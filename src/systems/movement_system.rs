use bevy::prelude::*;

use crate::systems::ThirdPersonCamera;

const RUN_SPEED: f32 = 6.0;
const WALK_SPEED: f32 = 3.0;
const RUN_TO_WALK_DURATION: f32 = 0.25;
const RUN_TO_WALK_CURVATURE: f32 = 2.0;

#[derive(Resource)]
pub struct MovementState {
    pub pressed: String,

    pub dir: Vec2,
    pub velocity: Vec2,
    pub speed: f32,

    pub accel_k: f32,

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
            pressed: String::new(),
            dir: Vec2::Y,
            velocity: Vec2::ZERO,
            speed: 0.0,

            accel_k: 6.0,

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
fn acceleration_function(t: f32, k: f32) -> f32 {
    1.0 - (-k * t.max(0.0)).exp()
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

pub fn movement_system(
    time: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    cam_q: Query<&Transform, With<ThirdPersonCamera>>,
    mut st: ResMut<MovementState>
) {
    let dt = time.delta_seconds();

    let max_fall_speed = -3.0 * RUN_SPEED;

    if st.is_falling {
        st.pressed = "Falling".to_string();

        st.speed = (st.speed - st.fall_decel * dt).max(0.0);

        if st.speed <= st.stop_epsilon {
            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
        } else {
            let d = st.dir.normalize_or_zero();
            st.velocity = d * st.speed;
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

    st.pressed = if wants_run {
        format!("Run {}", direction_string(desired_dir))
    } else {
        direction_string(desired_dir)
    };

    let moving = st.speed > st.stop_epsilon;
    let current_dir = if moving { st.dir.normalize_or_zero() } else { Vec2::ZERO };

    if st.hard_turn_active {
        if !has_input {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;
            st.pending_dir = Vec2::ZERO;

            st.speed = 0.0;
            st.velocity = Vec2::ZERO;

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

        let alpha = acceleration_function(st.t, st.accel_k).clamp(0.0, 1.0);
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
}

fn direction_string(dir: Vec2) -> String {
    if dir == Vec2::ZERO {
        return "Idle".to_string();
    }

    let mut parts = Vec::new();

    if dir.y > 0.0 {
        parts.push("Forward");
    }
    if dir.y < 0.0 {
        parts.push("Backward");
    }
    if dir.x > 0.0 {
        parts.push("Right");
    }
    if dir.x < 0.0 {
        parts.push("Left");
    }

    parts.join(" ")
}
