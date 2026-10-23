//! Live rescan: owns the file watcher from `harneo-core`, rebuilds it when the registry changes
//! and turns its changes into the `harneo://changed` event. The watcher only reads.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use harneo_core::drift::watch::{ChangeScope, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use ts_rs::TS;

use crate::commands::LibraryState;

/// The event the frontend listens to; the payload is a [`ChangeScope`].
pub const CHANGED_EVENT: &str = "harneo://changed";
/// The setting that stores whether the user paused the live rescan.
pub const PAUSED_KEY: &str = "watch.paused";

/// What Settings shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct WatchStatus {
    /// Registered projects being watched.
    pub watching: u32,
    pub paused: bool,
}

/// The watcher and its switches, in managed state.
#[derive(Default)]
pub struct WatchState {
    watcher: Mutex<Option<Watcher>>,
    paused: Arc<AtomicBool>,
    projects: AtomicU32,
}

/// Starts the watcher from the registry (at app start). A failure leaves the app working
/// without live rescan.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<WatchState>();
    let stored = app
        .state::<LibraryState>()
        .with(|library| library.settings_get(PAUSED_KEY))
        .ok()
        .flatten();
    state.paused.store(
        stored == Some(serde_json::Value::Bool(true)),
        Ordering::SeqCst,
    );
    refresh_watch(app);
}

/// Rebuilds what is watched from the registry; call it after the registry changes.
pub fn refresh_watch<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<WatchState>();
    let Ok(paths) = app
        .state::<LibraryState>()
        .with(|library| library.watch_paths())
    else {
        return;
    };
    let projects = paths.len().saturating_sub(1);
    state.projects.store(
        u32::try_from(projects).unwrap_or(u32::MAX),
        Ordering::SeqCst,
    );
    let Ok(mut guard) = state.watcher.lock() else {
        return;
    };
    if let Some(watcher) = guard.as_ref() {
        watcher.set_paths(paths);
        return;
    }
    let handle = app.clone();
    let paused = state.paused.clone();
    *guard = Watcher::start(paths, move |scope: ChangeScope| {
        if !paused.load(Ordering::SeqCst) {
            // A failed emit means the window is gone; there is no one to tell.
            let _ = handle.emit(CHANGED_EVENT, scope);
        }
    })
    .ok();
}

/// How many projects are watched, and whether the rescan is paused.
#[tauri::command]
pub fn watch_status(state: State<'_, WatchState>) -> WatchStatus {
    WatchStatus {
        watching: state.projects.load(Ordering::SeqCst),
        paused: state.paused.load(Ordering::SeqCst),
    }
}

/// Pauses or resumes the live rescan, and remembers the choice.
#[tauri::command]
pub fn watch_pause(
    watch: State<'_, WatchState>,
    library: State<'_, LibraryState>,
    paused: bool,
) -> Result<(), String> {
    library.with(|l| l.settings_set(PAUSED_KEY, &serde_json::Value::Bool(paused)))?;
    watch.paused.store(paused, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use harneo_core::library::Library;
    use std::sync::mpsc;
    use std::time::Duration;
    use tauri::Listener;

    fn app_with(
        dir: &std::path::Path,
    ) -> Result<tauri::App<tauri::test::MockRuntime>, Box<dyn std::error::Error>> {
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir)?)));
        app.manage(WatchState::default());
        Ok(app)
    }

    #[test]
    fn status_serializes_camel_case() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(WatchStatus {
            watching: 2,
            paused: true,
        })?;
        assert_eq!(json["watching"], 2);
        assert_eq!(json["paused"], true);
        Ok(())
    }

    #[test]
    fn pause_is_stored_and_read_back_at_start() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = app_with(dir.path())?;
        watch_pause(app.state(), app.state(), true)?;
        assert!(watch_status(app.state()).paused);
        let stored = app
            .state::<LibraryState>()
            .with(|l| l.settings_get(PAUSED_KEY))?;
        assert_eq!(stored, Some(serde_json::Value::Bool(true)));

        // A fresh app over the same data folder starts paused.
        let again = app_with(dir.path())?;
        start(again.handle());
        assert_eq!(
            watch_status(again.state()),
            WatchStatus {
                watching: 0,
                paused: true
            }
        );
        Ok(())
    }

    #[test]
    fn a_library_change_is_emitted_unless_paused() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = app_with(dir.path())?;
        let (tx, rx) = mpsc::channel();
        app.listen(CHANGED_EVENT, move |event| {
            let _ = tx.send(event.payload().to_string());
        });
        start(app.handle());
        // A second refresh retargets the running watcher instead of starting another.
        refresh_watch(app.handle());
        std::thread::sleep(Duration::from_millis(200));
        let store = dir.path().join("library").join("artifacts");
        std::fs::write(store.join("x"), "1")?;
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3))?,
            r#"{"scope":"library"}"#
        );

        watch_pause(app.state(), app.state(), true)?;
        std::fs::write(store.join("y"), "1")?;
        assert!(rx.recv_timeout(Duration::from_millis(1200)).is_err());
        Ok(())
    }
}
