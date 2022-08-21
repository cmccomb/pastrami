#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use tauri::Manager;

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![rhai])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use rhai::{packages::Package, Engine};
use rhai_rand::RandomPackage;
use rhai_sci::SciPackage;

#[tauri::command]
fn rhai(name: &str, window: tauri::Window) {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());
    engine.register_global_module(RandomPackage::new().as_shared_module());
    let w = window.clone();
    engine.on_print(move |x| {
        w.eval(&format!("append_output('{}')", x));
    });
    let script_ast = engine.compile(&name).map_err(|e| e.to_string()).unwrap();

    match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
        Ok(result) => window.eval(&format!("append_output('{}')", result.to_string())),
        Err(e) => window.eval(&format!("append_output('{:?}')", e)),
    };
}
