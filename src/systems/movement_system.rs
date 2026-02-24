use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveMode {
    Accel,
    Decel,
    Brake, // reversing direction
}

#[derive(Resource)]
pub struct MovementState {
    pub pressed: String,
    pub dir: Vec2,
    pub velocity: Vec2,

    pub speed: f32,
    pub max_speed: f32,

    mode: MoveMode,
    t: f32,
    start_speed: f32,

    pub accel_k: f32,
    pub decel_a: f32,
    pub brake_a: f32,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            pressed: String::new(),
            dir: Vec2::Y, // default forward
            velocity: Vec2::ZERO,

            speed: 0.0,
            max_speed: 6.0,
            mode: MoveMode::Decel,
            t: 0.0,
            start_speed: 0.0,

            accel_k: 6.0,
            decel_a: 6.0,
            brake_a: 10.0,
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

fn choose_mode(desired_dir: Vec2, current_dir: Vec2, speed: f32) -> MoveMode {
    if desired_dir == Vec2::ZERO {
        MoveMode::Decel
    } else if speed > 0.001 && desired_dir.dot(current_dir) < 0.0 {
        MoveMode::Brake
    } else {
        MoveMode::Accel
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

    // 1) desired direction
    let desired_dir = read_input_dir(&keys);

    // (optional) UI label
    st.pressed = direction_string(desired_dir);

    // 2) decide mode
    let new_mode = choose_mode(desired_dir, st.dir, st.speed);

    // 3) on mode change: reset timer & capture start_speed; update dir when applicable
    if new_mode != st.mode {
        st.mode = new_mode;
        st.t = 0.0;
        st.start_speed = st.speed;

        // If we have input, adopt that as our direction
        if desired_dir != Vec2::ZERO {
            st.dir = desired_dir;
        }
    } else {
        // also update dir if we're accelerating and input direction changes slightly
        if st.mode == MoveMode::Accel && desired_dir != Vec2::ZERO {
            st.dir = desired_dir;
        }
    }

    st.t += dt;

    // 4) compute scalar speed from curves
    st.speed = match st.mode {
        MoveMode::Accel => st.max_speed * accel_exp(st.t, st.accel_k).clamp(0.0, 1.0),
        MoveMode::Decel => st.start_speed * inv_square(st.t, st.decel_a),
        MoveMode::Brake => st.start_speed * inv_square(st.t, st.brake_a),
    };

    if st.speed < 0.001 && desired_dir == Vec2::ZERO {
        st.speed = 0.0;
    }

    // 5) velocity = dir * speed
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
