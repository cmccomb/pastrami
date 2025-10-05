# `pastrami` on `rhai`

`pastrami` is a Tauri-based desktop companion for exploring the [`rhai`](https://rhai.rs/) scripting language together with
scientific extensions from [`rhai-sci`](https://github.com/alexheretic/rhai-sci). The application packages a REPL, file
navigation, and workspace details in a single interface so that exploratory data work remains fast and reproducible.

## Prerequisites

The project targets recent Linux distributions and macOS releases. Ensure the following tooling is available before running
the application or its test suite:

- [Rust](https://www.rust-lang.org/tools/install) toolchain (`rustup` with the `stable` toolchain is recommended)
- `node` and `npm` for the frontend assets when building the full desktop bundle
- Linux hosts require the system libraries needed by WebKitGTK and GTK3:
  ```bash
  sudo apt-get update
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libsoup2.4-dev \
    libglib2.0-dev \
    patchelf
  ```
  The `glib-2.0` development headers are necessary for crates that depend on `glib-sys`, including the GTK bindings pulled in
  by Tauri's windowing layer.
- macOS hosts require Xcode command-line tools (for SDK headers and signing utilities) in addition to the Rust and Node.js
  toolchains. Install the `aarch64-apple-darwin` Rust target via `rustup target add aarch64-apple-darwin` so universal bundles
  can be produced. The Tauri bundler will also use the system `codesign` binary during `npm run tauri build`.

> [!NOTE]
> Ubuntu 24.04 publishes WebKitGTK 4.1. The bundled Tauri backend links against the same version, so the development
> dependencies above install `libwebkit2gtk-4.1-dev` and `libjavascriptcoregtk-4.1-dev`. If you are building on an older
> distribution that only ships WebKitGTK 4.0 you can still compile the application, but cross-version builds are not
> supported. Should your linker complain about missing `libwebkit2gtk-4.0.so` or `libjavascriptcoregtk-4.0.so`, create
> compatibility symlinks that point to the 4.1 versions, for example:
> ```bash
> sudo ln -sf /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.0.so
> sudo ln -sf /usr/lib/x86_64-linux-gnu/libjavascriptcoregtk-4.1.so /usr/lib/x86_64-linux-gnu/libjavascriptcoregtk-4.0.so
> ```
> This mirrors the workaround we apply in CI so `cargo clippy` and `cargo test` can link successfully against WebKitGTK.

## Development workflow

Run the following commands from the repository root to validate changes locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
cargo test
```

Documentation for the Rust components can be generated with:

```bash
cargo doc --no-deps
```

## Continuous integration

GitHub Actions validates every commit with formatting (`cargo fmt`), linting (`cargo clippy` with pedantic warnings), and the
Rust test suite on `ubuntu-22.04`. A dedicated `macos-13` job installs the Rust and Node.js toolchains, builds the universal
macOS bundle via `npm run tauri build -- --target universal-apple-darwin`, and uploads the resulting `.app` artifact.

## Running the desktop application

The `src-tauri` directory contains the Rust backend while the web assets live in the repository root. After installing the
prerequisites you can launch the development build with:

```bash
npm install
npm run tauri
```

This will build the frontend, compile the Rust backend, and open the desktop shell with hot-reload enabled.

### Editor assistance

The embedded REPL and script workspace now use CodeMirror with Rhai-aware autocomplete:

- Press <kbd>Ctrl</kbd> + <kbd>Space</kbd> to open the completion palette at any cursor location.
- Start typing Rhai keywords or namespace prefixes (`rand::`, `fs::`, `url::`, `ml::`, `sci::`) to receive suggestions automatically.
- Inside the REPL, <kbd>Enter</kbd> submits the current buffer while <kbd>Shift</kbd> + <kbd>Enter</kbd> inserts a newline without executing.

The static completion source covers Rhai syntax primitives alongside the namespaces that `configure_engine` makes available so that common APIs remain one shortcut away.

## Bundled Rhai packages

Pastrami preloads the curated [`rhaiscript`](https://github.com/orgs/rhaiscript/repositories?type=all) packages directly
into the Rhai engine so their APIs are always available without visiting a settings modal. Each package is exposed through
its own namespace to avoid polluting the global scope:

- [`sci`](https://github.com/rhaiscript/rhai-sci) wraps the scientific helpers from `rhai-sci` (matrix algebra, statistics,
  and modelling powered by the `nalgebra`, `smartcore`, and `rand` feature set)
- [`ml`](https://github.com/rhaiscript/rhai-ml) provides machine learning utilities from `rhai-ml`
- [`fs`](https://github.com/rhaiscript/rhai-fs) offers filesystem helpers from `rhai-fs`
- [`url`](https://github.com/rhaiscript/rhai-url) exposes URL parsing and manipulation helpers from `rhai-url`
- [`rand`](https://github.com/rhaiscript/rhai-rand) supplies random number generation helpers from `rhai-rand`

Call functions with the namespace prefix—for example `rand::rand(0, 10)` to generate a random integer. The REPL and
script runner share this configuration, so scripts have access to the same modules by default.

The curated scientific namespace intentionally omits the optional CSV/file helpers from `rhai-sci`'s `io` feature so the Tauri
bundle remains portable across macOS and Linux. Combine the `sci` tools with the dedicated `fs` namespace whenever a script
needs filesystem access.
