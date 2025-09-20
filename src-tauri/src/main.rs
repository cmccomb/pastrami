#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};

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
    let scope = rhai::Scope::new();
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

    run_rhai_script_with_sink(script, &output_sink);
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.0.lock().unwrap();
    let mut scope = state.1.lock().unwrap();
    let window_for_print = window.clone();
    engine.on_print(move |message| {
        send_output(&window_for_print, message);
    });

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
        Ok(result) => send_output(&window, &result.to_string()),
        Err(e) => send_output(&window, &format!("{e:?}")),
    }
}

type OutputSink = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Executes a Rhai script and forwards every emitted message to the provided
/// sink.
///
/// Messages include anything produced by the script via `print` as well as the
/// final return value or runtime error.
///
/// # Examples
/// ```rust,ignore
/// let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
/// let sink_output = std::sync::Arc::clone(&output);
/// let sink: OutputSink = std::sync::Arc::new(move |message: String| {
///     sink_output
///         .lock()
///         .expect("failed to capture script output")
///         .push(message);
/// });
///
/// run_rhai_script_with_sink("40 + 2", &sink);
/// assert_eq!(output.lock().unwrap().as_slice(), ["42"]);
/// ```
fn run_rhai_script_with_sink(script: &str, sink: &OutputSink) {
    let mut engine = rhai::Engine::new();
    engine.register_global_module(SciPackage::new().as_shared_module());

    let print_sink = Arc::clone(sink);
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
    use super::{append_output_script, run_rhai_script_with_sink, OutputSink};
    use std::sync::{Arc, Mutex};

    fn run_script_with_collector(script: &str) -> Vec<String> {
        let captured_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_target = Arc::clone(&captured_output);
        let sink: OutputSink = Arc::new(move |message: String| {
            sink_target
                .lock()
                .expect("collector mutex poisoned")
                .push(message);
        });

        run_rhai_script_with_sink(script, &sink);

        let collected = captured_output
            .lock()
            .expect("collector mutex poisoned")
            .clone();

        collected
    }

    #[test]
    fn append_output_script_serializes_control_characters() {
        let result = append_output_script("Line 1\nLine 2\tTabbed")
            .expect("failed to serialize message with control characters");
        assert_eq!(result, "append_output(\"Line 1\\nLine 2\\tTabbed\")");
    }

    #[test]
    fn append_output_script_preserves_quotes_and_unicode() {
        let result = append_output_script("He said, \"hi\" ☃")
            .expect("failed to serialize message with quotes and unicode");
        assert_eq!(result, "append_output(\"He said, \\\"hi\\\" ☃\")");
    }

    #[test]
    fn invalid_script_reports_parse_error() {
        let output = run_script_with_collector("let x = ;");
        let last_message = output
            .last()
            .expect("missing output entry for invalid script");

        assert!(
            last_message.contains("Unexpected"),
            "expected parse error message, got: {last_message}",
        );
    }

    #[test]
    fn valid_script_reports_result() {
        let output = run_script_with_collector("40 + 2");
        assert!(
            output.contains(&"42".to_string()),
            "expected valid script to produce \"42\" but saw {output:?}",
        );
    }

    #[test]
    fn runtime_errors_are_forwarded_to_the_sink() {
        let output = run_script_with_collector(
            r#"
            fn explode() { throw("boom"); }
            explode();
            "#,
        );

        let last_message = output
            .last()
            .expect("missing output entry for runtime error");
        assert!(
            last_message.contains("boom"),
            "expected runtime error payload, got: {last_message}",
        );
    }

    #[test]
    fn print_statements_are_captured_before_results() {
        let output = run_script_with_collector(r#"print("hi"); 41 + 1;"#);

        assert_eq!(
            output,
            vec!["hi".to_string(), "42".to_string()],
            "expected print output to precede the result"
        );
    }
}
