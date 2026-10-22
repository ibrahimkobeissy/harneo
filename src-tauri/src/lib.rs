//! Harneo's desktop shell: window setup and thin IPC commands. Logic lives in `harneo-core`.

mod commands;

/// Starts the app. A failure to start is reported and exits non-zero rather than panicking,
/// since `unwrap`/`expect` are denied outside tests.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::LibraryState::open())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::library_list,
            commands::library_detail,
            commands::library_remove,
            commands::library_find,
            commands::home_claude_dir,
            commands::library_preview,
            commands::library_import,
            commands::tags_list,
            commands::tag_create,
            commands::tag_rename,
            commands::tag_recolor,
            commands::tag_delete,
            commands::artifact_tags,
            commands::artifact_tag_add,
            commands::artifact_tag_remove,
            commands::collections_list,
            commands::collection_create_manual,
            commands::collection_create_smart,
            commands::collection_rename,
            commands::collection_update_rule,
            commands::collection_delete,
            commands::collection_add,
            commands::collection_remove,
            commands::collection_page,
            commands::artifact_collections
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("harneo: failed to start the app: {err}");
        std::process::exit(1);
    }
}
