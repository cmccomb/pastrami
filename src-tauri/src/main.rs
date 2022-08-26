#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::Mutex;
use tauri::Manager;

struct MyState(Mutex<rhai::Engine>);

fn main() {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());
    engine.register_global_module(RandomPackage::new().as_shared_module());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![rhai])
        .manage(MyState(Mutex::new(engine)))
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
        w.eval(&format!("append_output('{}')", x.to_string()));
    });
    let script_ast = engine.compile(&name).map_err(|e| e.to_string()).unwrap();

    match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
        Ok(result) => window.eval(&format!("append_output('{}')", result.to_string())),
        Err(e) => window.eval(&format!("append_output('{:?}')", e)),
    };
}

#[tauri::command]
fn rhai_repl(name: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.0.lock().unwrap();
    let w = window.clone();
    engine.on_print(move |x| {
        w.eval(&format!("append_output('{}')", x.to_string()));
    });
    let script_ast = engine.compile(&name).map_err(|e| e.to_string()).unwrap();

    match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
        Ok(result) => window.eval(&format!("append_output('{}')", result.to_string())),
        Err(e) => window.eval(&format!("append_output('{:?}')", e)),
    };
}
