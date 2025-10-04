#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(feature = "custom-protocol")]
mod desktop {
    use std::sync::{Arc, Mutex};

    use app::{
        append_output_script, run_rhai_script_with_sink, OutputSink, PackageDescriptor,
        PackageToggles,
    };
    use serde::Serialize;
    use tauri::State;

    pub struct MyState {
        engine: Mutex<rhai::Engine>,
        scope: Mutex<rhai::Scope<'static>>,
        packages: Mutex<PackageToggles>,
    }

    pub fn run_application() {
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

    #[tauri::command]
    fn rhai_script(script: &str, window: tauri::Window, state: State<MyState>) {
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
    fn rhai_repl(script: &str, window: tauri::Window, state: State<MyState>) {
        let mut engine = state.engine.lock().expect("engine mutex poisoned");
        let mut scope = state.scope.lock().expect("scope mutex poisoned");
        let window_for_print = window.clone();
        engine.on_print(move |message| {
            send_output(&window_for_print, message);
        });

        match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
            Ok(result) => send_output(&window, &result.to_string()),
            Err(error) => send_output(&window, &format!("{error:?}")),
        }
    }

    #[derive(Serialize)]
    struct SerializablePackageDescriptor {
        name: String,
        description: String,
        repository: String,
        selected: bool,
    }

    #[tauri::command]
    fn list_available_packages(state: State<MyState>) -> Vec<SerializablePackageDescriptor> {
        let selected = state
            .packages
            .lock()
            .expect("package mutex poisoned")
            .clone();

        build_package_descriptors(&selected)
            .into_iter()
            .map(|descriptor| SerializablePackageDescriptor {
                name: descriptor.name,
                description: descriptor.description,
                repository: descriptor.repository,
                selected: descriptor.selected,
            })
            .collect()
    }

    fn build_package_descriptors(selected: &PackageToggles) -> Vec<PackageDescriptor> {
        vec![
            PackageDescriptor {
                name: "rhai-sci".to_string(),
                description: "Scientific and numerical utilities built on smartcore and nalgebra"
                    .to_string(),
                repository: "https://github.com/rhaiscript/rhai-sci".to_string(),
                selected: selected.sci(),
            },
            PackageDescriptor {
                name: "rhai-ml".to_string(),
                description: "Machine learning helpers for Rhai scripts".to_string(),
                repository: "https://github.com/rhaiscript/rhai-ml".to_string(),
                selected: selected.ml(),
            },
        ]
    }

    #[tauri::command]
    fn update_packages(selected: Vec<String>, state: State<MyState>) {
        let mut package_state = state.packages.lock().expect("package mutex poisoned");

        let mut new_selection = package_state.clone();
        new_selection.update_from_selection(&selected);

        if *package_state == new_selection {
            return;
        }

        *package_state = new_selection.clone();

        let mut engine = state.engine.lock().expect("engine mutex poisoned");
        *engine = {
            let mut refreshed = rhai::Engine::new();
            new_selection.apply_to_engine(&mut refreshed);
            refreshed
        };

        let mut scope = state.scope.lock().expect("scope mutex poisoned");
        *scope = rhai::Scope::new();
    }
}

#[cfg(feature = "custom-protocol")]
fn main() {
    desktop::run_application();
}

#[cfg(not(feature = "custom-protocol"))]
fn main() {
    eprintln!("Enable the `desktop` (or `custom-protocol`) feature to run the Tauri UI.");
}
