#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use std::sync::{Arc, Mutex};

use serde::Serialize;

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct PackageToggles {
    sci: bool,
    ml: bool,
}

impl Default for PackageToggles {
    fn default() -> Self {
        Self {
            sci: true,
            ml: false,
        }
    }
}

impl PackageToggles {
    fn apply_to_engine(&self, engine: &mut rhai::Engine) {
        if self.sci {
            engine.register_global_module(SciPackage::new().as_shared_module());
        }

        if self.ml {
            engine.register_global_module(MLPackage::new().as_shared_module());
        }
    }

    fn update_from_selection(&mut self, selected: &[String]) {
        let normalized: Vec<String> = selected
            .iter()
            .map(|name| name.trim().to_lowercase())
            .collect();

        self.sci = normalized.iter().any(|name| name == "rhai-sci");
        self.ml = normalized.iter().any(|name| name == "rhai-ml");
    }

    fn selected_packages(&self) -> Vec<String> {
        let mut packages = Vec::new();
        if self.sci {
            packages.push("rhai-sci".to_string());
        }

        if self.ml {
            packages.push("rhai-ml".to_string());
        }

        packages
    }
}

#[derive(Serialize)]
struct PackageDescriptor {
    name: String,
    description: String,
    repository: String,
    selected: bool,
}

struct MyState {
    engine: Mutex<rhai::Engine>,
    scope: Mutex<rhai::Scope<'static>>,
    packages: Mutex<PackageToggles>,
}

fn main() {
    let packages = PackageToggles::default();
    let mut engine = rhai::Engine::new();
    packages.apply_to_engine(&mut engine);
    let scope = rhai::Scope::new();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            rhai_repl,
            rhai_script,
            list_available_packages,
            update_packages
        ])
        .manage(MyState {
            engine: Mutex::new(engine),
            scope: Mutex::new(scope),
            packages: Mutex::new(packages),
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use rhai::packages::Package;
use rhai_ml::MLPackage;
use rhai_sci::SciPackage;

#[tauri::command]
fn rhai_script(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let output_sink: OutputSink = Arc::new(move |message: String| {
        send_output(&window, &message);
    });

    let selected_packages = state
        .packages
        .lock()
        .expect("package mutex poisoned")
        .clone();

    run_rhai_script_with_sink(script, &output_sink, &selected_packages);
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn rhai_repl(script: &str, window: tauri::Window, state: tauri::State<MyState>) {
    let mut engine = state.engine.lock().unwrap();
    let mut scope = state.scope.lock().unwrap();
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
fn run_rhai_script_with_sink(script: &str, sink: &OutputSink, packages: &PackageToggles) {
    let mut engine = rhai::Engine::new();
    packages.apply_to_engine(&mut engine);

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
    use super::{append_output_script, run_rhai_script_with_sink, OutputSink, PackageToggles};
    use std::sync::{Arc, Mutex};

    fn run_script_with_collector(script: &str, packages: PackageToggles) -> Vec<String> {
        let captured_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_target = Arc::clone(&captured_output);
        let sink: OutputSink = Arc::new(move |message: String| {
            sink_target
                .lock()
                .expect("collector mutex poisoned")
                .push(message);
        });

        run_rhai_script_with_sink(script, &sink, &packages);

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
        let output = run_script_with_collector("let x = ;", PackageToggles::default());
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
        let output = run_script_with_collector("40 + 2", PackageToggles::default());
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
            PackageToggles::default(),
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
        let output =
            run_script_with_collector(r#"print("hi"); 41 + 1;"#, PackageToggles::default());

        assert_eq!(
            output,
            vec!["hi".to_string(), "42".to_string()],
            "expected print output to precede the result"
        );
    }

    #[test]
    fn package_selection_can_toggle_sci_and_ml_modules() {
        let mut packages = PackageToggles::default();
        assert!(packages.sci, "sci should be enabled by default");
        assert!(!packages.ml, "ml should be disabled by default");

        packages.update_from_selection(&["rhai-ml".to_string()]);

        assert!(
            !packages.sci,
            "updating selection without rhai-sci should disable the sci package"
        );
        assert!(
            packages.ml,
            "updating selection with rhai-ml should enable the ml package"
        );

        assert_eq!(packages.selected_packages(), vec!["rhai-ml".to_string()]);
    }
}

#[tauri::command]
fn list_available_packages(state: tauri::State<MyState>) -> Vec<PackageDescriptor> {
    let selected = state
        .packages
        .lock()
        .expect("package mutex poisoned")
        .clone();

    vec![
        PackageDescriptor {
            name: "rhai-sci".to_string(),
            description: "Scientific and numerical utilities built on smartcore and nalgebra"
                .to_string(),
            repository: "https://github.com/rhaiscript/rhai-sci".to_string(),
            selected: selected.sci,
        },
        PackageDescriptor {
            name: "rhai-ml".to_string(),
            description: "Machine learning helpers for Rhai scripts".to_string(),
            repository: "https://github.com/rhaiscript/rhai-ml".to_string(),
            selected: selected.ml,
        },
    ]
}

#[tauri::command]
fn update_packages(selected: Vec<String>, state: tauri::State<MyState>) {
    let mut package_state = state.packages.lock().expect("package mutex poisoned");

    let mut new_selection = package_state.clone();
    new_selection.update_from_selection(&selected);

    if *package_state == new_selection {
        return;
    }

    *package_state = new_selection.clone();

    let mut engine = state.engine.lock().unwrap();
    *engine = {
        let mut refreshed = rhai::Engine::new();
        new_selection.apply_to_engine(&mut refreshed);
        refreshed
    };

    let mut scope = state.scope.lock().unwrap();
    *scope = rhai::Scope::new();
}
