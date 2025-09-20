#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};
use tauri::Manager;

struct MyState(Mutex<rhai::Engine>, Mutex<rhai::Scope<'static>>);

trait ScriptOutput: Send + Sync + 'static {
    fn emit(&self, message: String);
}

#[derive(Clone)]
struct WindowScriptOutput {
    window: tauri::Window,
}

impl WindowScriptOutput {
    fn new(window: tauri::Window) -> Self {
        Self { window }
    }
}

impl ScriptOutput for WindowScriptOutput {
    fn emit(&self, message: String) {
        if let Ok(serialized) = serde_json::to_string(&message) {
            let script = format!("append_output({serialized})");
            let _ = self.window.eval(script.as_str());
        }
    }
}

fn main() {
    let mut engine = rhai::Engine::new();
    let mut scope = rhai::Scope::new();
    engine.register_global_module(SciPackage::new().as_shared_module());

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![rhai_repl, rhai_script])
        .manage(MyState(Mutex::new(engine), Mutex::new(scope)))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use rhai::{packages::Package, Engine};
use rhai_sci::SciPackage;

#[tauri::command]
fn rhai_script(script: &str, window: tauri::Window) {
    let output = Arc::new(WindowScriptOutput::new(window.clone()));
    run_rhai_script(script, output);
}

#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.0.lock().unwrap();
    let mut scope = state.1.lock().unwrap();
    let output = Arc::new(WindowScriptOutput::new(window.clone()));
    let print_output = Arc::clone(&output);
    engine.on_print(move |x| {
        print_output.emit(x.to_string());
    });

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &script) {
        Ok(result) => output.emit(result.to_string()),
        Err(e) => output.emit(format!("{e:?}")),
    };
}

fn run_rhai_script(script: &str, output: Arc<dyn ScriptOutput>) {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());
    let print_output = Arc::clone(&output);
    engine.on_print(move |x| {
        print_output.emit(x.to_string());
    });

    match engine.compile(script) {
        Ok(script_ast) => match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
            Ok(result) => output.emit(result.to_string()),
            Err(error) => output.emit(format!("{error:?}")),
        },
        Err(error) => output.emit(error.to_string()),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingOutput {
        messages: Mutex<Vec<String>>,
    }

    impl RecordingOutput {
        fn messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    impl ScriptOutput for RecordingOutput {
        fn emit(&self, message: String) {
            self.messages.lock().unwrap().push(message);
        }
    }

    #[test]
    fn invalid_script_emits_error_message() {
        let output = Arc::new(RecordingOutput::default());
        run_rhai_script("let x =", Arc::clone(&output));

        let messages = output.messages();
        assert!(!messages.is_empty(), "expected an error message");
        let message = &messages[0];
        assert!(
            message.to_lowercase().contains("error"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn valid_script_emits_result_message() {
        let output = Arc::new(RecordingOutput::default());
        run_rhai_script("40 + 2", Arc::clone(&output));

        let messages = output.messages();
        assert!(messages.contains(&"42".to_string()));
    }
}
