#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use rhai::{packages::Package, Engine};
use rhai_sci::SciPackage;

#[tauri::command]
fn greet(name: &str) -> String {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());
    let script_ast = engine.compile(&name).map_err(|e| e.to_string()).unwrap();

    let result: rhai::Dynamic = engine
        .eval_ast(&script_ast)
        .map_err(|e| e.to_string())
        .unwrap();

    format!("{:?}", result)
}
