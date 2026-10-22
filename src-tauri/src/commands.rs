//! Thin IPC commands: convert arguments and results, never hold logic (that is `harneo-core`).

use std::collections::HashMap;
use std::sync::Mutex;

use harneo_core::artifact::ArtifactKind;

use harneo_core::library::{ArtifactDetail, ImportReport, Library, Page, Preview, Query, Source};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
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

    /// Like `with`, for the calls that change the library.
    fn with_mut<T>(
        &self,
        f: impl FnOnce(&mut Library) -> harneo_core::Result<T>,
    ) -> Result<T, String> {
        let mut guard = self.0.lock().map_err(|e| e.to_string())?;
        match &mut *guard {
            Ok(library) => f(library).map_err(|e| e.to_string()),
            Err(message) => Err(message.clone()),
        }
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

// --- Artifact detail panel ---

/// One library artifact in full: scan result, provenance and stored files.
#[tauri::command]
pub fn library_detail(
    state: State<'_, LibraryState>,
    id: String,
) -> Result<ArtifactDetail, String> {
    state.with(|library| library.detail(&id))
}

/// Deletes an artifact from the library (its copy, tags and collection entries).
#[tauri::command]
pub fn library_remove(state: State<'_, LibraryState>, id: String) -> Result<(), String> {
    state.with_mut(|library| library.remove(&id))
}

/// The id of the library artifact of `kind` named `name`, if any (for opening a reference).
#[tauri::command]
pub fn library_find(
    state: State<'_, LibraryState>,
    kind: ArtifactKind,
    name: String,
) -> Result<Option<String>, String> {
    state.with(|library| library.find(kind, &name))
}

// --- Import flow -----------------------------------------------------------------------

/// `~/.claude`, the "your own harness" import source, as a string for the UI.
#[tauri::command]
pub fn home_claude_dir() -> Result<String, String> {
    let dir = harneo_core::paths::claude_home().map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// A typed git URL read into URL, branch and subfolder (so a pasted `…/tree/main/skills` link
/// works); a local source is left as it is.
fn normalized(source: Source) -> harneo_core::Result<Source> {
    match source {
        Source::Git {
            url,
            branch,
            subfolder,
        } => Source::parse_git(&url, branch.as_deref(), subfolder.as_deref()),
        other => Ok(other),
    }
}

/// Scans a source and lists what could be imported. A git source is cloned first, which takes
/// time, so the work runs on the blocking pool and the window never blocks (the library mutex is
/// taken inside that closure, never across an await). The clone waits in the library for `library_import`.
#[tauri::command]
pub async fn library_preview(app: AppHandle, source: Source) -> Result<Preview, String> {
    let source = normalized(source).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.preview(&source))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copies the ticked candidates of a preview into the library with the chosen tags.
#[tauri::command]
pub async fn library_import(
    app: AppHandle,
    preview: Preview,
    selection: Vec<String>,
    tags: HashMap<String, Vec<String>>,
) -> Result<ImportReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with_mut(|library| library.import(&preview, &selection, &tags))
    })
    .await
    .map_err(|e| e.to_string())?
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

    #[test]
    fn library_remove_and_detail_report_unknown_ids() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let state = LibraryState::from_result(Ok(Library::open(dir.path())?));
        let missing = state.with(|library| library.detail("nope")).err();
        assert!(missing.is_some_and(|m| m.contains("not in the library")));
        let removed = state.with_mut(|library| library.remove("nope")).err();
        assert!(removed.is_some_and(|m| m.contains("not in the library")));
        assert_eq!(state.with(|l| l.find(ArtifactKind::Skill, "x"))?, None);
        Ok(())
    }

    #[test]
    fn import_runs_through_the_state_and_lists_the_result() -> Result<(), Box<dyn std::error::Error>>
    {
        let data = tempfile::tempdir()?;
        let source = tempfile::tempdir()?;
        std::fs::create_dir_all(source.path().join("skills/alpha"))?;
        std::fs::write(
            source.path().join("skills/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: A skill\n---\nBody\n",
        )?;
        let state = LibraryState::from_result(Ok(Library::open(data.path())?));
        let src = Source::Local {
            path: source.path().to_path_buf(),
        };
        let preview = state.with(|library| library.preview(&src))?;
        assert_eq!(preview.candidates.len(), 1);
        let ids = vec![preview.candidates[0].id.clone()];
        let report = state.with_mut(|library| library.import(&preview, &ids, &HashMap::new()))?;
        assert_eq!(report.imported, ids);
        let page = state.with(|library| library.list(&Query::default()))?;
        assert_eq!(page.total, 1);
        Ok(())
    }

    #[test]
    fn home_claude_dir_ends_in_dot_claude() -> Result<(), String> {
        assert!(home_claude_dir()?.ends_with(".claude"));
        Ok(())
    }

    #[test]
    fn a_pasted_tree_url_is_split_into_branch_and_subfolder() -> harneo_core::Result<()> {
        let source = normalized(Source::Git {
            url: "github.com/o/r/tree/main/skills".into(),
            branch: None,
            subfolder: None,
        })?;
        assert!(matches!(
            source,
            Source::Git { branch: Some(ref b), subfolder: Some(ref s), .. } if b == "main" && s == "skills"
        ));
        Ok(())
    }
}
