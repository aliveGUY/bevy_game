use bevy::{
    asset::LoadState,
    core_pipeline::Skybox,
    prelude::*,
    render::render_resource::{ TextureViewDescriptor, TextureViewDimension },
};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_camera, setup_light));
        app.add_systems(Update, attach_skybox);
    }
}

// store the handle right on the camera
#[derive(Component)]
struct SkyboxHandle(Handle<Image>);

fn setup_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        SkyboxHandle(asset_server.load("skybox/skybox.ktx2")),
    ));
}

fn attach_skybox(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    cams: Query<(Entity, &SkyboxHandle), Without<Skybox>>
) {
    for (e, h) in &cams {
        if asset_server.load_state(&h.0) != LoadState::Loaded {
            continue;
        }

        if let Some(img) = images.get_mut(&h.0) {
            img.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        } else {
            continue;
        }

        commands.entity(e).insert(Skybox {
            image: h.0.clone(),
            brightness: 1000.0,
        });
    }
}

fn setup_light(mut commands: Commands) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 35_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(20.0, 40.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.4, 0.6, 1.0),
        brightness: 0.25,
    });
}
