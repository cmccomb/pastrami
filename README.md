# `pastrami` on `rhai`

`pastrami` is a Tauri-based desktop companion for exploring the [`rhai`](https://rhai.rs/) scripting language together with
scientific extensions from [`rhai-sci`](https://github.com/alexheretic/rhai-sci). The application packages a REPL, file
navigation, and workspace details in a single interface so that exploratory data work remains fast and reproducible.

## Prerequisites

The project targets recent Linux distributions. Ensure the following tooling is available before running the application or
its test suite:

- [Rust](https://www.rust-lang.org/tools/install) toolchain (`rustup` with the `stable` toolchain is recommended)
- `node` and `npm` for the frontend assets when building the full desktop bundle
- System libraries required by WebKitGTK and GTK3:
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

> [!NOTE]
> Ubuntu 24.04 currently publishes WebKitGTK 4.1. When developing on that release you may need to provide compatibility
> symlinks (`libwebkit2gtk-4.0.so`, `libjavascriptcoregtk-4.0.so`, and their corresponding `.pc` files) that forward to the
> installed 4.1 libraries so that older Tauri versions link successfully.

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
Rust test suite. CI runs on `ubuntu-22.04` to match Tauri's current Linux support matrix.

## Running the desktop application

The `src-tauri` directory contains the Rust backend while the web assets live in the repository root. After installing the
prerequisites you can launch the development build with:

```bash
npm install
npm run tauri
```

This will build the frontend, compile the Rust backend, and open the desktop shell with hot-reload enabled.
