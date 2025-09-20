//! Minimal stub implementation of the `rhai-sci` package used for testing.

use rhai::packages::Package;
use rhai::Module;
use rhai::Shared;
use std::sync::Arc;

/// Provides scientific utilities for Rhai scripts.
///
/// This stub simply exposes an empty module so that the application can
/// compile in the absence of the real `rhai-sci` crate.
#[derive(Clone, Default)]
pub struct SciPackage(Shared<Module>);

impl SciPackage {
    /// Create a new [`SciPackage`].
    #[must_use]
    pub fn new() -> Self {
        let mut module = Module::new();
        Self::init(&mut module);
        Self(Arc::new(module))
    }
}

impl Package for SciPackage {
    fn init(module: &mut Module) {
        let _ = module;
    }

    fn as_shared_module(&self) -> Shared<Module> {
        self.0.clone()
    }
}
