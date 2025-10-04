#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::{Arc, Mutex};

use app::{append_output_script, configure_engine, run_rhai_script_with_sink, OutputSink};
use rhai::{Dynamic, Engine, Scope};
use tauri::Manager;

fn send_output(window: &tauri::Window, message: &str) {
    match append_output_script(message) {
        Ok(script) => {
            if let Err(eval_error) = window.eval(&script) {
                eprintln!("failed to evaluate output script: {eval_error}");
            }
        }
        Err(serialize_error) => {
            eprintln!("failed to serialize output message {message:?}: {serialize_error}");
        }
    }
}

struct MyState {
    engine: Mutex<Engine>,
    scope: Mutex<Scope<'static>>,
}

fn main() {
    let engine = configure_engine(Engine::new());
    let scope = Scope::new();

    let app_state = Arc::new(MyState {
        engine: Mutex::new(engine),
        scope: Mutex::new(scope),
    });

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![rhai_repl, rhai_script])
        .manage(app_state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn rhai_script(script: &str, window: tauri::Window) {
    let sink_window = window;
    let output_sink: OutputSink = Arc::new(move |message: String| {
        send_output(&sink_window, &message);
    });

    run_rhai_script_with_sink(script, &output_sink);
}

#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window) {
    let app_state = {
        let app_handle = window.app_handle();
        let state = app_handle.state::<Arc<MyState>>();
        Arc::clone(&state)
    };

    let mut engine = app_state
        .engine
        .lock()
        .expect("shared engine mutex poisoned");
    let mut scope = app_state.scope.lock().expect("shared scope mutex poisoned");

    let evaluation_window = window.clone();
    let print_window = window;
    engine.on_print(move |message| {
        send_output(&print_window, message);
    });

    match engine.eval_with_scope::<Dynamic>(&mut scope, script) {
        Ok(result) => send_output(&evaluation_window, &result.to_string()),
        Err(e) => send_output(&evaluation_window, &format!("{e:?}")),
    }
}
