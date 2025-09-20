#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Builds a JavaScript snippet that safely forwards a message to the frontend.
///
/// The message is serialized using `serde_json` so that newline characters,
/// quotes, and other special characters remain valid once evaluated in the
/// webview.
///
/// # Examples
/// ```rust,ignore
/// let script = append_output_script("Hello\nworld").unwrap();
/// assert_eq!(script, "append_output(\"Hello\\nworld\")");
/// ```
fn append_output_script(message: &str) -> Result<String, serde_json::Error> {
    serde_json::to_string(message).map(|escaped| format!("append_output({escaped})"))
}

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
        send_output(&window, &message);
    });

    run_rhai_script_with_sink(script, output_sink);
}

#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.0.lock().unwrap();
    let mut scope = state.1.lock().unwrap();
    let w = window.clone();
    engine.on_print(move |x| {
        send_output(&w, &x.to_string());
    });

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &script) {
        Ok(result) => send_output(&window, &result.to_string()),
        Err(e) => send_output(&window, &format!("{e:?}")),
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
    fn append_output_script_serializes_control_characters() {
        let result = append_output_script("Line 1\nLine 2\tTabbed").unwrap();
        assert_eq!(result, "append_output(\"Line 1\\nLine 2\\tTabbed\")");
    }

    #[test]
    fn append_output_script_preserves_quotes_and_unicode() {
        let result = append_output_script("He said, \"hi\" ☃").unwrap();
        assert_eq!(result, "append_output(\"He said, \\\"hi\\\" ☃\")");
    }

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
