#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};

use rhai::packages::Package;
use rhai_fs::FilesystemPackage;
use rhai_ml::MLPackage;
use rhai_rand::RandomPackage;
use rhai_sci::SciPackage;
use rhai_url::UrlPackage;
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

type SharedModule = rhai::Shared<rhai::Module>;

fn flatten_package_module<P>(package: P) -> SharedModule
where
    P: Package,
{
    let shared = package.as_shared_module();
    let mut module = rhai::Module::new();
    module.combine_flatten((*shared).clone());
    module.into()
}

fn build_rand_module() -> SharedModule {
    flatten_package_module(RandomPackage::new())
}

fn build_fs_module() -> SharedModule {
    flatten_package_module(FilesystemPackage::new())
}

fn build_url_module() -> SharedModule {
    flatten_package_module(UrlPackage::new())
}

fn build_ml_module() -> SharedModule {
    flatten_package_module(MLPackage::new())
}

fn build_sci_module() -> SharedModule {
    flatten_package_module(SciPackage::new())
}

fn configure_engine(mut engine: rhai::Engine) -> rhai::Engine {
    engine.register_static_module("rand", build_rand_module());
    engine.register_static_module("fs", build_fs_module());
    engine.register_static_module("url", build_url_module());
    engine.register_static_module("ml", build_ml_module());
    engine.register_static_module("sci", build_sci_module());
    engine
}

struct MyState {
    engine: Mutex<rhai::Engine>,
    scope: Mutex<rhai::Scope<'static>>,
}

fn main() {
    let engine = configure_engine(rhai::Engine::new());
    let scope = rhai::Scope::new();

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
    let app_state = {
        let state = window.state::<Arc<MyState>>();
        Arc::clone(&state)
    };

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

    let mut engine = app_state.engine.lock().unwrap();
    let mut scope = app_state.scope.lock().unwrap();

    let evaluation_window = window.clone();
    let print_window = window;
    engine.on_print(move |message| {
        send_output(&print_window, message);
    });

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
        Ok(result) => send_output(&evaluation_window, &result.to_string()),
        Err(e) => send_output(&evaluation_window, &format!("{e:?}")),
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
    let mut engine = configure_engine(rhai::Engine::new());

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

    #[test]
    fn bundled_modules_are_available_under_namespaces() {
        let output = run_script_with_collector(
            r#"
            let value = rand::rand(0, 10);
            value >= 0 && value <= 10
            "#,
        );

        let last_message = output
            .last()
            .expect("missing output entry for bundled module test");

        assert_eq!(
            last_message, "true",
            "expected namespace-qualified module call to succeed"
        );
    }
}
