mod systems;

use bevy::prelude::*;
use systems::ScenePlugin;

pub fn run_app() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(ScenePlugin);
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
