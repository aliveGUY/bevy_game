use bevy::input::mouse::{ MouseMotion, MouseWheel };
use bevy::prelude::*;
use bevy::window::{ CursorGrabMode, PrimaryWindow };
use bevy_rapier3d::prelude::*;

use crate::systems::{ Player, SkyboxHandle };

const MIN_DISTANCE: f32 = 2.0;
const MAX_DISTANCE: f32 = 12.0;
const DEFAULT_DISTANCE: f32 = 6.0;

const MIN_PITCH: f32 = -1.2;
const MAX_PITCH: f32 = 1.0;

const LOOK_HEIGHT: f32 = 1.0;
const POSITION_LERP: f32 = 12.0;
const ROTATE_SENS: f32 = 0.001;
const ZOOM_SENS: f32 = 0.5;

pub struct ThirdPersonCameraPlugin;

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraInputState>();

        app.add_systems(Startup, setup_third_person_camera);

        app.add_systems(Update, (
            camera_lock_toggle_system,
            camera_input_system.after(camera_lock_toggle_system),
        ));

        app.add_systems(
            FixedUpdate,
            third_person_camera_system.after(crate::systems::apply_player_motion)
        );
    }
}

#[derive(Resource, Default)]
pub struct CameraInputState {
    pub locked: bool,
}

#[derive(Component)]
pub struct ThirdPersonCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    pub target_offset: Vec3,
    pub smoothness: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub rotate_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub collision_radius: f32,
}

impl Default for ThirdPersonCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.35,
            distance: DEFAULT_DISTANCE,
            target_distance: DEFAULT_DISTANCE,
            target_offset: Vec3::new(0.0, LOOK_HEIGHT, 0.0),
            smoothness: POSITION_LERP,
            pitch_min: MIN_PITCH,
            pitch_max: MAX_PITCH,
            zoom_min: MIN_DISTANCE,
            zoom_max: MAX_DISTANCE,
            rotate_sensitivity: ROTATE_SENS,
            zoom_sensitivity: ZOOM_SENS,
            collision_radius: 0.2,
        }
    }
}

fn setup_third_person_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 4.0, DEFAULT_DISTANCE).looking_at(
                Vec3::new(0.0, LOOK_HEIGHT, 0.0),
                Vec3::Y
            ),
            ..default()
        },
        ThirdPersonCamera::default(),
        SkyboxHandle(asset_server.load("skybox/skybox.ktx2")),
    ));
}

fn camera_lock_toggle_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut input_state: ResMut<CameraInputState>
) {
    let Ok(mut window) = window_q.get_single_mut() else {
        return;
    };

    // Left click locks the camera
    if mouse_buttons.just_pressed(MouseButton::Left) && !input_state.locked {
        input_state.locked = true;
        window.cursor.visible = false;

        // Locked is ideal, but some platforms behave better with Confined.
        window.cursor.grab_mode = CursorGrabMode::Locked;
    }

    // Escape unlocks it
    if keys.just_pressed(KeyCode::Escape) && input_state.locked {
        input_state.locked = false;
        window.cursor.visible = true;
        window.cursor.grab_mode = CursorGrabMode::None;
    }
}

fn camera_input_system(
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    input_state: Res<CameraInputState>,
    mut cam_q: Query<&mut ThirdPersonCamera>
) {
    let Ok(mut camera) = cam_q.get_single_mut() else {
        return;
    };

    if input_state.locked {
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion_events.read() {
            delta += ev.delta;
        }

        camera.yaw -= delta.x * camera.rotate_sensitivity;
        camera.pitch -= delta.y * camera.rotate_sensitivity;
        camera.pitch = camera.pitch.clamp(camera.pitch_min, camera.pitch_max);
    } else {
        // Drain events while unlocked so stale motion is not applied later
        for _ in mouse_motion_events.read() {
        }
    }

    for ev in mouse_wheel_events.read() {
        camera.target_distance -= ev.y * camera.zoom_sensitivity;
    }

    camera.target_distance = camera.target_distance.clamp(camera.zoom_min, camera.zoom_max);
}

fn third_person_camera_system(
    time: Res<Time<Fixed>>,
    rapier: Res<RapierContext>,
    player_q: Query<&Transform, (With<Player>, Without<ThirdPersonCamera>)>,
    mut cam_q: Query<
        (&mut Transform, &mut ThirdPersonCamera),
        (With<ThirdPersonCamera>, Without<Player>),
    >,
) {
    let Ok(player_t) = player_q.get_single() else {
        return;
    };
    let Ok((mut cam_transform, mut cam)) = cam_q.get_single_mut() else {
        return;
    };

    let dt = time.delta_seconds();
    let target = player_t.translation + cam.target_offset;

    cam.distance += (cam.target_distance - cam.distance) * (1.0 - (-10.0 * dt).exp());

    let yaw_rot = Quat::from_rotation_y(cam.yaw);
    let pitch_rot = Quat::from_rotation_x(cam.pitch);
    let rotation = yaw_rot * pitch_rot;

    let desired_offset = rotation * Vec3::new(0.0, 0.0, cam.distance);
    let desired_camera_pos = target + desired_offset;

    let mut final_camera_pos = desired_camera_pos;

    let to_camera = desired_camera_pos - target;
    let distance = to_camera.length();

    if distance > 0.001 {
        let dir = to_camera / distance;

        if let Some((_entity, hit)) = rapier.cast_ray_and_get_normal(
            target,
            dir,
            distance,
            true,
            QueryFilter::default(),
        ) {
            let safe_dist = (hit.time_of_impact - cam.collision_radius).max(cam.zoom_min);
            final_camera_pos = target + dir * safe_dist;
        }
    }

    let alpha = 1.0 - (-cam.smoothness * dt).exp();
    cam_transform.translation = cam_transform.translation.lerp(final_camera_pos, alpha);
    cam_transform.look_at(target, Vec3::Y);
}