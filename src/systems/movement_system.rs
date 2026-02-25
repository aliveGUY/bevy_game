use bevy::prelude::*;

#[derive(Resource)]
pub struct MovementState {
    // UI/debug
    pub pressed: String,

    // output
    pub dir: Vec2, // last non-zero direction (unit)
    pub velocity: Vec2, // dir * speed
    pub speed: f32,

    // tuning
    pub max_speed: f32,
    pub accel_k: f32, // accel curve strength
    pub decel_a: f32, // decel curve strength

    // dot thresholds (replace angle math)
    // dot = dir · desired_dir
    // near -1 => 180deg turn
    pub hard_turn_dot: f32, // e.g. -0.95
    // around 0 => ~90deg turn
    pub soft_turn_dot: f32, // e.g. 0.20

    // soft turn effect
    pub soft_turn_speed_factor: f32, // e.g. 0.5

    // stop behavior
    pub stop_epsilon: f32,

    // hard turn stop simulation
    pub hard_turn_min_stop_time: f32, // 50ms
    pub hard_turn_max_stop_time: f32, // 150ms
    hard_turn_active: bool,
    hard_turn_timer: f32,
    pending_dir: Vec2,

    // curve state (minimal)
    accelerating: bool,
    t: f32,
    start_speed: f32,
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

            hard_turn_dot: -0.95,
            soft_turn_dot: 0.2,

            soft_turn_speed_factor: 0.5,

            stop_epsilon: 0.02,

            hard_turn_min_stop_time: 0.1,
            hard_turn_max_stop_time: 0.15,
            hard_turn_active: false,
            hard_turn_timer: 0.0,
            pending_dir: Vec2::ZERO,

            accelerating: false,
            t: 0.0,
            start_speed: 0.0,
        }
    }
}

#[inline]
fn accel_exp(t: f32, k: f32) -> f32 {
    // 0..1
    1.0 - (-k * t.max(0.0)).exp()
}

#[inline]
fn inv_square(t: f32, a: f32) -> f32 {
    // 1..0
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
    let desired_dir = read_input_dir(&keys);
    st.pressed = direction_string(desired_dir);

    // --- 1) HARD TURN ACTIVE: force stop for min..max time ---
    if st.hard_turn_active {
        st.hard_turn_timer += dt;
        st.pending_dir = desired_dir; // can change mind while stopped

        st.speed = 0.0;
        st.velocity = Vec2::ZERO;

        let min_t = st.hard_turn_min_stop_time.min(st.hard_turn_max_stop_time).max(0.0);
        let max_t = st.hard_turn_max_stop_time.max(min_t);

        let can_leave = st.hard_turn_timer >= min_t;
        let must_leave = st.hard_turn_timer >= max_t;

        if can_leave && (st.pending_dir != Vec2::ZERO || must_leave) {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;

            if st.pending_dir != Vec2::ZERO {
                st.dir = st.pending_dir;
                restart_curve(&mut st, true);
            } else {
                restart_curve(&mut st, false);
            }
        }

        return;
    }

    // --- 2) Turn detection via dot thresholds (no angles) ---
    let moving = st.speed > st.stop_epsilon;
    let has_input = desired_dir != Vec2::ZERO;

    let mut soft_turn_active = false;

    if moving && has_input {
        let dot = st.dir.normalize_or_zero().dot(desired_dir); // both unit
        if dot <= st.hard_turn_dot {
            // start hard stop immediately
            st.hard_turn_active = true;
            st.hard_turn_timer = 0.0;
            st.pending_dir = desired_dir;

            st.speed = 0.0;
            st.velocity = Vec2::ZERO;
            return;
        } else if dot <= st.soft_turn_dot {
            soft_turn_active = true;
        }
    }

    // --- 3) Direction snaps to input (simple sim) ---
    if has_input {
        st.dir = desired_dir;
    }

    // --- 4) Update speed from curves ---
    restart_curve(&mut st, has_input);

    st.t += dt;

    let mut speed = if st.accelerating {
        st.max_speed * accel_exp(st.t, st.accel_k).clamp(0.0, 1.0)
    } else {
        st.start_speed * inv_square(st.t, st.decel_a)
    };

    // --- 5) Soft turn speed cap ---
    if soft_turn_active {
        let cap = st.max_speed * st.soft_turn_speed_factor;
        if speed > cap {
            speed = cap;
        }
    }

    // --- 6) Cleanup & output ---
    if !has_input && speed < st.stop_epsilon {
        speed = 0.0;
    }

    st.speed = speed;
    st.velocity = st.dir * st.speed;
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
