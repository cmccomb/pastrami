#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};
use tauri::Manager;

struct MyState(Mutex<rhai::Engine>, Mutex<rhai::Scope<'static>>);

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

use rhai::packages::Package;
use rhai_sci::SciPackage;

#[tauri::command]
fn rhai_script(script: &str, window: tauri::Window) {
    let output_sink: OutputSink = Arc::new(move |message: String| {
        let script = format!("append_output('{}')", message);
        let _ = window.eval(&script);
    });

    run_rhai_script_with_sink(script, output_sink);
}

#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.0.lock().unwrap();
    let mut scope = state.1.lock().unwrap();
    let w = window.clone();
    engine.on_print(move |x| {
        w.eval(&format!("append_output('{}')", x.to_string()));
    });

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &script) {
        Ok(result) => window.eval(&format!("append_output('{}')", result.to_string())),
        Err(e) => window.eval(&format!("append_output('{:?}')", e)),
    };
}

type OutputSink = Arc<dyn Fn(String) + Send + Sync + 'static>;

fn run_rhai_script_with_sink(script: &str, sink: OutputSink) {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());

    let print_sink = sink.clone();
    engine.on_print(move |x| {
        print_sink(x.to_string());
    });

    match engine.compile(script) {
        Ok(script_ast) => match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
            Ok(result) => sink(result.to_string()),
            Err(e) => sink(format!("{:?}", e)),
        },
        Err(e) => sink(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_script_reports_error_without_panicking() {
        let captured_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_target = captured_output.clone();
        let sink: OutputSink = Arc::new(move |message: String| {
            sink_target.lock().unwrap().push(message);
        });

        run_rhai_script_with_sink("let x = ;", sink);

        let output = captured_output.lock().unwrap();
        assert!(!output.is_empty(), "no output captured from invalid script");
        let last = output.last().expect("missing output entry");
        assert!(
            last.to_lowercase().contains("error"),
            "expected error message, got: {last}"
        );
    }

    #[test]
    fn valid_script_reports_result() {
        let captured_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_target = captured_output.clone();
        let sink: OutputSink = Arc::new(move |message: String| {
            sink_target.lock().unwrap().push(message);
        });

        run_rhai_script_with_sink("40 + 2", sink);

        let output = captured_output.lock().unwrap();
        assert!(output.contains(&"42".to_string()));
    }
}
