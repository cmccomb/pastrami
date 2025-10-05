use std::collections::BTreeSet;
use std::sync::Arc;

use rhai::packages::Package;
use rhai::Shared;
use serde_json::Error as SerdeError;

/// Shared module handle used to register Rhai packages with the engine.
pub type SharedModule = Shared<rhai::Module>;

/// Converts a Rhai package into a flattened module tree for registration.
fn flatten_package_module<P>(package: &P) -> SharedModule
where
    P: Package,
{
    let shared = package.as_shared_module();
    let mut module = rhai::Module::new();
    module.combine_flatten((*shared).clone());
    module.into()
}

/// Constructs the `rand` namespace backed by `rhai-rand` helpers.
fn build_rand_module() -> SharedModule {
    let package = rhai_rand::RandomPackage::new();
    flatten_package_module(&package)
}

/// Constructs the `fs` namespace backed by `rhai-fs` helpers.
fn build_fs_module() -> SharedModule {
    let package = rhai_fs::FilesystemPackage::new();
    flatten_package_module(&package)
}

/// Constructs the `url` namespace backed by `rhai-url` helpers.
fn build_url_module() -> SharedModule {
    let package = rhai_url::UrlPackage::new();
    flatten_package_module(&package)
}

/// Constructs the `ml` namespace backed by `rhai-ml` helpers.
fn build_ml_module() -> SharedModule {
    let package = rhai_ml::MLPackage::new();
    flatten_package_module(&package)
}

/// Constructs the `sci` namespace backed by a curated `rhai-sci` configuration.
fn build_sci_module() -> SharedModule {
    let package = rhai_sci::SciPackage::new();
    flatten_package_module(&package)
}

fn collect_module_entries(prefix: &str, module: &rhai::Module, entries: &mut BTreeSet<String>) {
    for (name, _) in module.iter_fn() {
        entries.insert(format!("{prefix}::{name}"));
    }

    for (module_name, sub_module) in module.iter_sub_modules() {
        let nested_prefix = format!("{prefix}::{module_name}");
        entries.insert(nested_prefix.clone());
        entries.insert(format!("{nested_prefix}::"));
        collect_module_entries(&nested_prefix, sub_module.as_ref(), entries);
    }
}

/// Collects completion entries for each bundled Rhai package.
///
/// The returned list includes namespace identifiers (for example, `rand` and
/// `rand::`), as well as fully-qualified function and sub-module names.
///
/// # Examples
///
/// ```
/// # use app::collect_completion_entries;
/// let entries = collect_completion_entries();
/// assert!(entries.iter().any(|entry| entry == "rand"));
/// assert!(entries.iter().any(|entry| entry == "rand::"));
/// assert!(entries
///     .iter()
///     .any(|entry| entry.starts_with("rand::") && entry.len() > "rand::".len()));
/// ```
pub fn collect_completion_entries() -> Vec<String> {
    let mut entries: BTreeSet<String> = BTreeSet::new();

    let modules = vec![
        ("rand", build_rand_module()),
        ("fs", build_fs_module()),
        ("url", build_url_module()),
        ("ml", build_ml_module()),
        ("sci", build_sci_module()),
    ];

    for (namespace, module) in modules {
        entries.insert(namespace.to_string());
        entries.insert(format!("{namespace}::"));
        collect_module_entries(namespace, module.as_ref(), &mut entries);
    }

    entries.into_iter().collect()
}

/// Registers all bundled namespaces on the provided engine and returns it.
///
/// # Examples
///
/// ```
/// # use rhai::Engine;
/// # use app::configure_engine;
/// # fn demo() -> Result<(), Box<rhai::EvalAltResult>> {
/// let engine = configure_engine(Engine::new());
/// let within_bounds: bool = engine.eval("let v = rand::rand(0, 10); v >= 0 && v <= 10")?;
/// assert!(within_bounds);
/// # Ok(())
/// # }
/// # demo().unwrap();
/// ```
pub fn configure_engine(mut engine: rhai::Engine) -> rhai::Engine {
    engine.register_static_module("rand", build_rand_module());
    engine.register_static_module("fs", build_fs_module());
    engine.register_static_module("url", build_url_module());
    engine.register_static_module("ml", build_ml_module());
    engine.register_static_module("sci", build_sci_module());
    engine
}

pub type OutputSink = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Builds a JavaScript snippet that safely forwards a message to the frontend.
///
/// # Errors
/// Returns an error if the message cannot be serialized with `serde_json`.
///
/// # Examples
///
/// ```
/// # use app::append_output_script;
/// let script = append_output_script("hello")?;
/// assert_eq!(script, "append_output(\"hello\")");
/// # Ok::<_, serde_json::Error>(())
/// ```
pub fn append_output_script(message: &str) -> Result<String, SerdeError> {
    serde_json::to_string(message).map(|escaped| format!("append_output({escaped})"))
}

/// Compiles and evaluates a Rhai script, streaming output and errors into `sink`.
///
/// # Examples
///
/// ```
/// # use std::sync::{Arc, Mutex};
/// # use app::{run_rhai_script_with_sink, OutputSink};
/// let captured = Arc::new(Mutex::new(Vec::new()));
/// let sink_target = Arc::clone(&captured);
/// let sink: OutputSink = Arc::new(move |message| {
///     sink_target.lock().unwrap().push(message);
/// });
/// run_rhai_script_with_sink("41 + 1", &sink);
/// assert_eq!(captured.lock().unwrap().last().unwrap(), "42");
/// ```
pub fn run_rhai_script_with_sink(script: &str, sink: &OutputSink) {
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
    use super::{
        append_output_script, collect_completion_entries, run_rhai_script_with_sink, OutputSink,
    };
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
            "expected print output to precede the result",
        );
    }

    #[test]
    fn bundled_modules_are_available_under_namespaces() {
        let output = run_script_with_collector(
            r"
            let value = rand::rand(0, 10);
            value >= 0 && value <= 10
            ",
        );

        let last_message = output
            .last()
            .expect("missing output entry for bundled module test");

        assert_eq!(
            last_message, "true",
            "expected namespace-qualified module call to succeed",
        );
    }

    #[test]
    fn collect_completion_entries_includes_namespace_and_functions() {
        let entries = collect_completion_entries();

        assert!(entries.contains(&"rand".to_string()));
        assert!(entries.contains(&"rand::".to_string()));
        assert!(entries
            .iter()
            .any(|entry| entry.starts_with("rand::") && entry.len() > "rand::".len()));
    }
}
