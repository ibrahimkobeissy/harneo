//! Harneo's desktop shell: window setup and thin IPC commands. Logic lives in `harneo-core`.

/// Starts the app. A failure to start is reported and exits non-zero rather than panicking,
/// since `unwrap`/`expect` are denied outside tests.
pub fn run() {
    let result = tauri::Builder::default().run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("harneo: failed to start the app: {err}");
        std::process::exit(1);
    }
}
