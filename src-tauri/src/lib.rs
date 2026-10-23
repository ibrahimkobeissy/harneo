//! Harneo's desktop shell: window setup and thin IPC commands. Logic lives in `harneo-core`.

mod commands;
mod watch;

/// Starts the app. A failure to start is reported and exits non-zero rather than panicking,
/// since `unwrap`/`expect` are denied outside tests.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::LibraryState::open())
        .manage(commands::UsageState::open())
        .manage(watch::WatchState::default())
        .setup(|app| {
            watch::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::library_list,
            commands::library_detail,
            commands::library_remove,
            commands::library_find,
            commands::library_reaudit,
            commands::library_audits,
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
            commands::artifact_collections,
            commands::usage_index,
            commands::usage_of,
            commands::usage_summary,
            commands::links_of,
            commands::link_add,
            commands::link_remove,
            commands::link_reject,
            commands::bundle_of,
            commands::updates_check,
            commands::update_status,
            commands::update_diff,
            commands::update_apply,
            commands::update_all,
            commands::settings_get,
            commands::settings_set,
            commands::claude_cli_status,
            commands::bundles_detect_preview,
            commands::bundles_detect_library,
            commands::profiles_list,
            commands::profile_get,
            commands::profile_create,
            commands::profile_save,
            commands::profile_rename,
            commands::profile_describe,
            commands::profile_delete,
            commands::profile_clone,
            commands::profile_from_collection,
            commands::profile_resolve,
            commands::profile_layout,
            commands::profile_export,
            commands::profile_import_preview,
            commands::profile_import_commit,
            commands::artifact_set_manual_only,
            commands::scaffold_plan,
            commands::install_apply,
            commands::refit_compare,
            commands::refit_plan,
            commands::projects_list,
            commands::project_get,
            commands::project_files,
            commands::project_file_content,
            commands::artifact_profiles,
            commands::switch_plan,
            commands::removal_plan,
            commands::project_backups,
            commands::backup_restore,
            commands::backup_delete,
            watch::watch_status,
            watch::watch_pause,
            commands::drift_check,
            commands::drift_check_all,
            commands::drift_diff,
            commands::drift_reapply_plan,
            commands::drift_promote,
            commands::drift_promote_added,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("harneo: failed to start the app: {err}");
        std::process::exit(1);
    }
}
