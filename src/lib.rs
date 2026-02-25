mod systems;
mod ui;

use bevy::prelude::*;
use bevy_rapier3d::{ plugin::{ NoUserData, RapierPhysicsPlugin }, render::RapierDebugRenderPlugin };
use systems::{ ScenePlugin, PlayerPlugin, MovementState };
use ui::UiPlugin;

pub fn run_app() {
    let mut app = App::new();
    app.init_resource::<MovementState>();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
    app.add_plugins(RapierDebugRenderPlugin::default());
    app.add_plugins(ScenePlugin);
    app.add_plugins(UiPlugin);
    app.add_plugins(PlayerPlugin);
    app.run();
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start() {
    console_error_panic_hook::set_once();
    run_app();
}
