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

    pub hard_turn_dot: f32, // e.g. -0.70  (135..225 starts around dot <= cos(135) = -0.707)
    pub soft_turn_dot: f32, // e.g.  0.70  (45..135 starts around dot <= cos(45)  =  0.707)

    pub soft_turn_speed_factor: f32, // 0.5
    pub stop_epsilon: f32,

    // hard turn behavior
    pub hard_turn_hold_time: f32, // 0.10
    hard_turn_active: bool,
    hard_turn_timer: f32,
    pending_dir: Vec2,

    // curve state
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

            // These correspond to actual angle bands:
            // soft: dot <= cos(45°)  ~ 0.707  => 45..180 (we'll also gate with hard below)
            // hard: dot <= cos(135°) ~ -0.707 => 135..225
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
    let desired_dir = read_input_dir(&keys);
    let has_input = desired_dir != Vec2::ZERO;

    st.pressed = direction_string(desired_dir);

    // Use current movement direction only if actually moving
    let moving = st.speed > st.stop_epsilon;
    let current_dir = if moving { st.dir.normalize_or_zero() } else { Vec2::ZERO };

    // ---------------------------
    // HARD TURN ACTIVE (lock)
    // ---------------------------
    if st.hard_turn_active {
        // If player released input: cancel immediately and remain stopped
        if !has_input {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;
            st.pending_dir = Vec2::ZERO;

            st.speed = 0.0;
            st.velocity = Vec2::ZERO;

            // Also kill curve so nothing "revives" speed later
            st.accelerating = false;
            st.t = 0.0;
            st.start_speed = 0.0;

            return;
        }

        // Still holding some direction while locked
        st.pending_dir = desired_dir;
        st.hard_turn_timer += dt;

        // Forced stop
        st.speed = 0.0;
        st.velocity = Vec2::ZERO;

        // After hold time, allow movement in pending dir
        if st.hard_turn_timer >= st.hard_turn_hold_time {
            st.hard_turn_active = false;
            st.hard_turn_timer = 0.0;

            st.dir = st.pending_dir;
            restart_curve(&mut st, true);
        }

        return;
    }

    // ---------------------------
    // Turn detection (when moving)
    // ---------------------------
    let mut soft_turn = false;

    if moving && has_input {
        let dot = current_dir.dot(desired_dir); // both unit

        if dot <= st.hard_turn_dot {
            // HARD: instant stop + (maybe) lock if holding
            st.speed = 0.0;
            st.velocity = Vec2::ZERO;

            // immediately reset curve so you don't "coast" via decel curve
            st.accelerating = false;
            st.t = 0.0;
            st.start_speed = 0.0;

            // enter lock only if still holding
            st.hard_turn_active = true;
            st.hard_turn_timer = 0.0;
            st.pending_dir = desired_dir;
            return;
        } else if dot <= st.soft_turn_dot {
            soft_turn = true;
        }
    }

    // ---------------------------
    // Direction update
    // ---------------------------
    if has_input {
        st.dir = desired_dir;
    }

    // ---------------------------
    // Curves
    // ---------------------------
    restart_curve(&mut st, has_input);
    st.t += dt;

    let mut speed = if st.accelerating {
        st.max_speed * accel_exp(st.t, st.accel_k).clamp(0.0, 1.0)
    } else {
        st.start_speed * inv_square(st.t, st.decel_a)
    };

    // SOFT TURN: drop speed by 50% immediately (your spec)
    if soft_turn {
        speed *= st.soft_turn_speed_factor;
    }

    // Cleanup
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
