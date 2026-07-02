//! MingoROS Studio — Tauri 2 entry point.
//!
//! Thin bootstrap: set up logging, then hand off to `mingoros_studio::run()`
//! (the app body lives in `lib.rs`, mirroring MingoCAN's can-studio).

// On Windows the framework expects a Windows GUI subsystem binary; the cfg-attr
// below suppresses the console window in release builds. macOS / Linux ignore it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `RUST_LOG=mingoros_studio=debug` works as expected; defaults to INFO.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    mingoros_studio::run();
}
