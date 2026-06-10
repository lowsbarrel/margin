//! Standalone codegen entrypoint: regenerates `src/lib/bindings.ts` from the
//! Rust `#[tauri::command]` + `specta::Type` definitions. Run with:
//!   cargo run --bin gen_bindings
//! This does not start the Tauri/webview runtime, so it works headless in CI.

fn main() {
    app_lib::export_bindings();
    println!("Wrote src/lib/bindings.ts");
}
