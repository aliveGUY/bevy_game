use bevy::prelude::*;

#[derive(Resource)]
pub struct MovementState {
    pub pressed: String,

    pub dir: Vec2,
    pub velocity: Vec2,
    pub speed: f32,

    pub max_speed: f32,
    pub accel_k: f32,
    pub decel_a: f32,

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

    // set by ground detection (player_system)
    pub is_falling: bool,

    // horizontal decay while falling
    pub fall_decel: f32,

    // ✅ NEW: vertical falling state (units/sec, negative down)
    pub fall_vel_y: f32,

    // ✅ NEW: gravity accel (units/sec^2, negative down)
    pub gravity: f32,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            pressed: String::new(),
            dir: Vec2::Y,
            velocity: Vec2::ZERO,
            speed: 0.0,

            max_speed: 6.0,
            accel_k: 6.0,
            decel_a: 6.0,

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
            gravity: -30.0, // tune
        }
    }
}

#[inline]
fn accel_exp(t: f32, k: f32) -> f32 {
    1.0 - (-k * t.max(0.0)).exp()
}

#[inline]
fn inv_square(t: f32, a: f32) -> f32 {
    1.0 / (1.0 + a * t.max(0.0)).powi(2)
}

#[inline]
fn restart_curve(st: &mut MovementState, accelerating: bool) {
    if st.accelerating != accelerating {
        st.accelerating = accelerating;
        st.t = 0.0;
        st.start_speed = st.speed;
    }
}

fn read_input_dir(keys: &ButtonInput<KeyCode>) -> Vec2 {
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
    if raw.length_squared() > 0.0 {
        raw.normalize()
    } else {
        Vec2::ZERO
    }
}

pub fn movement_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut st: ResMut<MovementState>
) {
    let dt = time.delta_seconds();

    // terminal fall speed = 3x top move speed
    let max_fall_speed = -3.0 * st.max_speed;

    // ✅ FALLING MODE:
    // - no new horizontal accel forces
    // - smoothly decay existing horizontal speed to 0
    // - integrate vertical fall velocity with gravity
    if st.is_falling {
        st.pressed = "Falling".to_string();

        // horizontal decay
        st.speed = (st.speed - st.fall_decel * dt).max(0.0);

        if st.speed <= st.stop_epsilon {
            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
        } else {
            let d = st.dir.normalize_or_zero();
            st.velocity = d * st.speed;
        }

        // vertical accelerate down
        st.fall_vel_y += st.gravity * dt;
        if st.fall_vel_y < max_fall_speed {
            st.fall_vel_y = max_fall_speed;
        }

        // prevent other logic while falling
        st.accelerating = false;
        st.t = 0.0;
        st.start_speed = st.speed;
        st.hard_turn_active = false;
        st.hard_turn_timer = 0.0;
        st.pending_dir = Vec2::ZERO;

        return;
    }

    // ✅ GROUNDED MODE:
    // reset vertical fall speed
    st.fall_vel_y = 0.0;

    // ---------------------------
    // NORMAL MODE (your original logic)
    // ---------------------------
    let desired_dir = read_input_dir(&keys);
    let has_input = desired_dir != Vec2::ZERO;

    st.pressed = direction_string(desired_dir);

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

    restart_curve(&mut st, has_input);
    st.t += dt;

    let mut speed = if st.accelerating {
        st.max_speed * accel_exp(st.t, st.accel_k).clamp(0.0, 1.0)
    } else {
        st.start_speed * inv_square(st.t, st.decel_a)
    };

    if soft_turn {
        speed *= st.soft_turn_speed_factor;
    }

    if !has_input && speed < st.stop_epsilon {
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
