//! Thin IPC commands: convert arguments and results, never hold logic (that is `harneo-core`).

use std::sync::Mutex;

use harneo_core::library::{Library, Page, Query};
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

/// What the About panel shows. `data_dir` crosses IPC as a string, since a path is not JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
}

/// Same version and data dir the `harneo` CLI reports, so both front ends agree.
#[tauri::command]
pub fn app_info() -> Result<AppInfo, String> {
    let data_dir = harneo_core::paths::data_dir().map_err(|e| e.to_string())?;
    Ok(AppInfo {
        version: harneo_core::version().into(),
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

/// The library, opened once at startup. An open failure is kept as its message and handed to
/// the UI by every library command, so the screen shows an error state instead of the app
/// crashing.
pub struct LibraryState(Mutex<Result<Library, String>>);

impl LibraryState {
    /// Opens the library at the app's data dir.
    pub fn open() -> Self {
        let opened = harneo_core::paths::data_dir()
            .and_then(|dir| Library::open(&dir))
            .map_err(|e| e.to_string());
        Self(Mutex::new(opened))
    }

    #[cfg(test)]
    fn from_result(opened: Result<Library, String>) -> Self {
        Self(Mutex::new(opened))
    }

    fn with<T>(&self, f: impl FnOnce(&Library) -> harneo_core::Result<T>) -> Result<T, String> {
        let guard = self.0.lock().map_err(|e| e.to_string())?;
        match &*guard {
            Ok(library) => f(library).map_err(|e| e.to_string()),
            Err(message) => Err(message.clone()),
        }
    }
}

/// One page of the library (filter, sort, offset, limit) with its total and facet counts.
#[tauri::command]
pub fn library_list(state: State<'_, LibraryState>, query: Query) -> Result<Page, String> {
    state.with(|library| library.list(&query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_matches_core() -> Result<(), String> {
        let info = app_info()?;
        assert_eq!(info.version, harneo_core::version());
        let expected = harneo_core::paths::data_dir().map_err(|e| e.to_string())?;
        assert_eq!(info.data_dir, expected.to_string_lossy());
        Ok(())
    }

    #[test]
    fn app_info_serializes_camel_case() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(AppInfo {
            version: "1.0.0".into(),
            data_dir: "/x".into(),
        })?;
        assert_eq!(json["dataDir"], "/x");
        assert_eq!(json["version"], "1.0.0");
        Ok(())
    }

    #[test]
    fn library_list_maps_query_and_page() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let state = LibraryState::from_result(Ok(Library::open(dir.path())?));
        // The UI sends camelCase JSON with only the fields it sets.
        let query: Query = serde_json::from_value(serde_json::json!({
            "filter": { "types": ["skill"], "tags": [], "sources": [], "text": "x" },
            "sort": "importedAt",
            "descending": true,
            "offset": 24,
            "limit": 24
        }))?;
        assert_eq!(query.offset, 24);
        let page = state.with(|library| library.list(&query))?;
        let json = serde_json::to_value(&page)?;
        assert_eq!(json["total"], 0);
        assert!(json["items"].as_array().is_some_and(Vec::is_empty));
        assert!(json["facets"]["kinds"].is_array());
        Ok(())
    }

    #[test]
    fn library_open_error_reaches_the_ui_as_a_message() {
        let state = LibraryState::from_result(Err("cannot open the library".into()));
        let err = state.with(|library| library.list(&Query::default())).err();
        assert_eq!(err.as_deref(), Some("cannot open the library"));
    }
}
