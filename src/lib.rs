mod systems;
mod ui;

use bevy::prelude::*;
use systems::{ ScenePlugin, PlayerPlugin, MovementState, movement_system };
use ui::UiPlugin;

pub fn run_app() {
    let mut app = App::new();
    app.init_resource::<MovementState>();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(ScenePlugin);
    app.add_plugins(UiPlugin);
    app.add_plugins(PlayerPlugin);
    app.add_systems(Update, movement_system);
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
