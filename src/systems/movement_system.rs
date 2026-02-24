use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct MovementState {
    pub pressed: String,
}

pub fn movement_system(keys: Res<ButtonInput<KeyCode>>, mut st: ResMut<MovementState>) {
    const MAP: &[(KeyCode, &str)] = &[
        (KeyCode::KeyW, "W"),
        (KeyCode::KeyA, "A"),
        (KeyCode::KeyS, "S"),
        (KeyCode::KeyD, "D"),
        (KeyCode::Space, "Space"),
        (KeyCode::ShiftLeft, "LShift"),
        (KeyCode::ShiftRight, "RShift"),
        (KeyCode::ControlLeft, "LCtrl"),
        (KeyCode::ControlRight, "RCtrl"),
    ];

    let mut s = String::new();
    for (k, name) in MAP {
        if keys.pressed(*k) {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(name);
        }
    }

    if st.pressed != s {
        st.pressed = s;
    }
}
