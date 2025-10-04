use std::sync::Arc;

use rhai::packages::Package;
use rhai_ml::MLPackage;
use rhai_sci::SciPackage;

/// A sink that consumes output strings emitted by Rhai scripts.
pub type OutputSink = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Builds a JavaScript snippet that safely forwards a message to the frontend.
///
/// The message is serialized using `serde_json` so that newline characters,
/// quotes, and other special characters remain valid once evaluated in the
/// webview.
///
/// # Examples
/// ```
/// use app::append_output_script;
///
/// let script = append_output_script("Hello" ).unwrap();
/// assert_eq!(script, "append_output(\"Hello\")");
/// ```
///
/// # Errors
///
/// Returns [`serde_json::Error`] if the provided message cannot be serialized to
/// JSON.
pub fn append_output_script(message: &str) -> Result<String, serde_json::Error> {
    serde_json::to_string(message).map(|escaped| format!("append_output({escaped})"))
}

/// Keeps track of which Rhai extension packages are enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageToggles {
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
    /// Registers the selected packages with the provided `Engine` instance.
    pub fn apply_to_engine(&self, engine: &mut rhai::Engine) {
        if self.sci {
            engine.register_global_module(SciPackage::new().as_shared_module());
        }

        if self.ml {
            engine.register_global_module(MLPackage::new().as_shared_module());
        }
    }

    /// Updates the toggles using a list of selected package names.
    pub fn update_from_selection<I, S>(&mut self, selected: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let normalized: Vec<String> = selected
            .into_iter()
            .map(|name| name.as_ref().trim().to_lowercase())
            .collect();

        self.sci = normalized.iter().any(|name| name == "rhai-sci");
        self.ml = normalized.iter().any(|name| name == "rhai-ml");
    }

    /// Returns the currently selected packages.
    #[must_use]
    pub fn selected_packages(&self) -> Vec<String> {
        let mut packages = Vec::new();
        if self.sci {
            packages.push("rhai-sci".to_string());
        }

        if self.ml {
            packages.push("rhai-ml".to_string());
        }

        packages
    }

    /// Indicates whether the scientific utilities package is enabled.
    #[must_use]
    pub fn sci(&self) -> bool {
        self.sci
    }

    /// Indicates whether the machine learning package is enabled.
    #[must_use]
    pub fn ml(&self) -> bool {
        self.ml
    }
}

/// Describes an optional Rhai package that can be toggled in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageDescriptor {
    pub name: String,
    pub description: String,
    pub repository: String,
    pub selected: bool,
}

/// Executes a Rhai script and forwards every emitted message to the provided
/// sink.
///
/// Messages include anything produced by the script via `print` as well as the
/// final return value or runtime error.
///
/// # Examples
/// ```
/// use app::{run_rhai_script_with_sink, OutputSink, PackageToggles};
/// use std::sync::{Arc, Mutex};
///
/// let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
/// let sink_output = Arc::clone(&captured);
/// let sink: OutputSink = Arc::new(move |message| {
///     sink_output
///         .lock()
///         .expect("failed to capture script output")
///         .push(message);
/// });
///
/// run_rhai_script_with_sink("40 + 2", &sink, &PackageToggles::default());
/// assert_eq!(captured.lock().unwrap().as_slice(), ["42"]);
/// ```
pub fn run_rhai_script_with_sink(script: &str, sink: &OutputSink, packages: &PackageToggles) {
    let mut engine = rhai::Engine::new();
    packages.apply_to_engine(&mut engine);

    let print_sink = Arc::clone(sink);
    engine.on_print(move |x| {
        print_sink(x.to_string());
    });

    match engine.compile(script) {
        Ok(script_ast) => match engine.eval_ast::<rhai::Dynamic>(&script_ast) {
            Ok(result) => sink(result.to_string()),
            Err(e) => sink(format!("{e:?}")),
        },
        Err(e) => sink(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{append_output_script, run_rhai_script_with_sink, OutputSink, PackageToggles};
    use std::sync::{Arc, Mutex};

    fn run_script_with_collector(script: &str, packages: &PackageToggles) -> Vec<String> {
        let captured_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_target = Arc::clone(&captured_output);
        let sink: OutputSink = Arc::new(move |message: String| {
            sink_target
                .lock()
                .expect("collector mutex poisoned")
                .push(message);
        });

        run_rhai_script_with_sink(script, &sink, packages);

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
        let default_packages = PackageToggles::default();
        let output = run_script_with_collector("let x = ;", &default_packages);
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
        let default_packages = PackageToggles::default();
        let output = run_script_with_collector("40 + 2", &default_packages);
        assert!(
            output.contains(&"42".to_string()),
            "expected valid script to produce \"42\" but saw {output:?}",
        );
    }

    #[test]
    fn runtime_errors_are_forwarded_to_the_sink() {
        let default_packages = PackageToggles::default();
        let output = run_script_with_collector(
            r#"
            fn explode() { throw("boom"); }
            explode();
            "#,
            &default_packages,
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
        let default_packages = PackageToggles::default();
        let output = run_script_with_collector(r#"print("hi"); 41 + 1;"#, &default_packages);

        assert_eq!(
            output,
            vec!["hi".to_string(), "42".to_string()],
            "expected print output to precede the result",
        );
    }

    #[test]
    fn package_selection_can_toggle_sci_and_ml_modules() {
        let mut packages = PackageToggles::default();
        assert!(packages.sci(), "sci should be enabled by default");
        assert!(!packages.ml(), "ml should be disabled by default");

        packages.update_from_selection(&["rhai-ml".to_string()]);

        assert!(
            !packages.sci(),
            "updating selection without rhai-sci should disable the sci package",
        );
        assert!(
            packages.ml(),
            "updating selection with rhai-ml should enable the ml package",
        );

        assert_eq!(packages.selected_packages(), vec!["rhai-ml".to_string()]);
    }
}
