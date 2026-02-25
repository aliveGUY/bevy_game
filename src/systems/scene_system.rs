use bevy::{
    asset::LoadState,
    core_pipeline::Skybox,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};
use bevy_rapier3d::prelude::*;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_light, setup_ground));
        app.add_systems(Update, attach_skybox);
    }
}

#[derive(Component)]
pub struct SkyboxHandle(pub Handle<Image>);

#[derive(Component)]
pub struct Ground;

fn setup_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ground_size = 10.0;
    let ground_height = 1.0;

    // Visual + physics ground
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(
                ground_size,
                ground_height,
                ground_size,
            ))),
            material: materials.add(Color::srgb(0.3, 0.5, 0.3)),
            // top surface at y=0
            transform: Transform::from_xyz(0.0, -ground_height / 2.0, 0.0),
            ..default()
        },
        Ground,
        RigidBody::Fixed,
        Collider::cuboid(
            ground_size / 2.0,
            ground_height / 2.0,
            ground_size / 2.0,
        ),
    ));
}

fn attach_skybox(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    cams: Query<(Entity, &SkyboxHandle), Without<Skybox>>,
) {
    for (e, h) in &cams {
        if asset_server.load_state(&h.0) != LoadState::Loaded {
            continue;
        }

        let Some(img) = images.get_mut(&h.0) else { continue; };

        img.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });

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