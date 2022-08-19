use rhai::{packages::Package, Engine, INT};
use rhai_rand::RandomPackage;
use rhai_sci::SciPackage;

fn main() {
    // Create a new Rhai engine
    let mut engine = Engine::new();

    // Add the rhai-sci package to the new engine
    engine.register_global_module(SciPackage::new().as_shared_module());

    // Add the rhai-rand package to the new engine
    engine.register_global_module(RandomPackage::new().as_shared_module());

    // Now run your code
    let value = engine.eval::<INT>("argmin([43, 42, -500])").unwrap();
    println!("{value}");
}
