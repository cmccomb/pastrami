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
    libwebkit2gtk-4.0-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libsoup2.4-dev \
    patchelf
  ```
- macOS hosts require Xcode command-line tools (for SDK headers and signing utilities) in addition to the Rust and Node.js
  toolchains. Install the `aarch64-apple-darwin` Rust target via `rustup target add aarch64-apple-darwin` so universal bundles
  can be produced. The Tauri bundler will also use the system `codesign` binary during `npm run tauri build`.

> [!NOTE]
> Ubuntu 24.04 currently publishes WebKitGTK 4.1. When developing on that release you may need to provide compatibility
> symlinks (`libwebkit2gtk-4.0.so`, `libjavascriptcoregtk-4.0.so`, and their corresponding `.pc` files) that forward to the
> installed 4.1 libraries so that older Tauri versions link successfully.

## Development workflow

The Rust backend now exposes a headless default build that keeps the heavy
system dependencies used by Tauri optional. Run the following commands from the
repository root to validate changes locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings -W clippy::pedantic
cargo test
```

Enable the `desktop` feature whenever you want to lint or test code paths that
depend on the Tauri runtime:

```bash
cargo clippy --all-targets --features desktop -- -D warnings -W clippy::pedantic
cargo test --features desktop
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
npm run tauri dev -- --features desktop
```

This will build the frontend, compile the Rust backend with the `desktop` feature enabled, and open the desktop shell with
hot-reload enabled. Build artifacts for distribution can be produced with:

```bash
npm run tauri build -- --features desktop
```

## Loading Rhai community packages

Open the **Settings** button in the upper-right corner of the interface to load additional Rhai packages at runtime. The
modal lists the curated [`rhaiscript`](https://github.com/orgs/rhaiscript/repositories?type=all) packages bundled with the
application. Enable the checkboxes for [`rhai-sci`](https://github.com/rhaiscript/rhai-sci) or
[`rhai-ml`](https://github.com/rhaiscript/rhai-ml) to register their APIs in the interactive REPL and one-off script runner
without restarting the app. Clearing all checkboxes reverts to the base Rhai engine.
