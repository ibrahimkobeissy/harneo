//! Thin IPC commands: convert arguments and results, never hold logic (that is `harneo-core`).

use std::collections::HashMap;
use std::sync::Mutex;

use harneo_core::artifact::{ArtifactKind, ScanOptions};

use harneo_core::bundles::{
    cli_status, ClaudeConfig, CliStatus, DetectProgress, Detection, LinkOrigin, Links,
};
use harneo_core::install::{
    ApplyReport, Backup, Comparison, InstalledFile, Plan, Project, ProjectFile, RefitAction,
};
use harneo_core::library::{
    ArtifactAudit, ArtifactDetail, Collection, Filter, ImportReport, Library, Page, Preview, Query,
    ReauditReport, Source, Tag, TagColor, TagWithCount,
};
use harneo_core::profiles::{
    Layout, ProfileId, ProfileImportPreview, ProfileImportReport, ProfileSpec, ProfileSummary,
    ProfileUse, ProfileView, Resolved, MAX_FILE_BYTES,
};
use harneo_core::updates::{
    CheckReport, CheckScope, FileDiff, UpdateChoice, UpdateReport, UpdateStatus,
};
use harneo_core::usage::{IndexReport, Usage, UsageIndex, UsageSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
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

/// Re-runs the security audit on every stored library copy and stores the results.
#[tauri::command]
pub async fn library_reaudit(app: AppHandle) -> Result<ReauditReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with_mut(|library| library.reaudit_all())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The stored audit of every library artifact, for the Security page.
#[tauri::command]
pub fn library_audits(state: State<'_, LibraryState>) -> Result<Vec<ArtifactAudit>, String> {
    state.with(|library| library.audits())
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
pub async fn library_preview(
    app: AppHandle,
    source: Source,
    include_managed: Option<bool>,
) -> Result<Preview, String> {
    let options = ScanOptions {
        include_managed: include_managed.unwrap_or(false),
        ..ScanOptions::default()
    };
    let source = normalized(source).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.preview_with(&source, &options))
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
    links: Option<Vec<(String, String, String)>>,
) -> Result<ImportReport, String> {
    let links = links.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with_mut(|library| library.import_with_links(&preview, &selection, &tags, &links))
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Tags ------------------------------------------------------------------------------

/// Every tag with the number of artifacts carrying it.
#[tauri::command]
pub fn tags_list(state: State<'_, LibraryState>) -> Result<Vec<TagWithCount>, String> {
    state.with(|library| library.tags())
}

/// Creates a tag with a palette color.
#[tauri::command]
pub fn tag_create(
    state: State<'_, LibraryState>,
    name: String,
    color: TagColor,
) -> Result<Tag, String> {
    state.with(|library| library.create_tag(&name, color))
}

/// Renames a tag.
#[tauri::command]
pub fn tag_rename(state: State<'_, LibraryState>, id: i64, name: String) -> Result<Tag, String> {
    state.with(|library| library.rename_tag(id, &name))
}

/// Changes a tag's color.
#[tauri::command]
pub fn tag_recolor(
    state: State<'_, LibraryState>,
    id: i64,
    color: TagColor,
) -> Result<Tag, String> {
    state.with(|library| library.recolor_tag(id, color))
}

/// Deletes a tag and takes it off every artifact.
#[tauri::command]
pub fn tag_delete(state: State<'_, LibraryState>, id: i64) -> Result<(), String> {
    state.with(|library| library.delete_tag(id))
}

/// The tags on one artifact.
#[tauri::command]
pub fn artifact_tags(state: State<'_, LibraryState>, id: String) -> Result<Vec<Tag>, String> {
    state.with(|library| library.artifact_tags(&id))
}

/// Puts a tag on an artifact.
#[tauri::command]
pub fn artifact_tag_add(
    state: State<'_, LibraryState>,
    id: String,
    tag_id: i64,
) -> Result<(), String> {
    state.with(|library| library.tag_artifact(&id, tag_id))
}

/// Takes a tag off an artifact.
#[tauri::command]
pub fn artifact_tag_remove(
    state: State<'_, LibraryState>,
    id: String,
    tag_id: i64,
) -> Result<(), String> {
    state.with(|library| library.untag_artifact(&id, tag_id))
}

// --- Updates ---------------------------------------------------------------------------

/// Looks at the sources for artifacts that changed at their source. Git is asked first and
/// cloned only when it moved, which takes time, so the work runs on the blocking pool and the
/// library lock is held only to read what is needed and to store what was found.
#[tauri::command]
pub async fn updates_check(app: AppHandle, scope: CheckScope) -> Result<CheckReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LibraryState>();
        let job = state.with(|library| library.prepare_check(&scope))?;
        let outcome = job.run();
        state.with(|library| library.finish_check(outcome))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The update state of an artifact, `None` when it matches its source.
#[tauri::command]
pub fn update_status(
    state: State<'_, LibraryState>,
    id: String,
) -> Result<Option<UpdateStatus>, String> {
    state.with(|library| library.update_status(&id))
}

/// What updating an artifact would change, file by file. Copies the upstream version into a
/// staging folder (a clone when none is kept), so it runs on the blocking pool without the
/// library lock.
#[tauri::command]
pub async fn update_diff(app: AppHandle, id: String) -> Result<Vec<FileDiff>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LibraryState>();
        let job = state.with(|library| library.prepare_update(&id))?;
        job.run()
            .and_then(|prepared| prepared.diff())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Updates one artifact as the user chose; the lock is held only for the reads and the swap.
#[tauri::command]
pub async fn update_apply(
    app: AppHandle,
    id: String,
    choice: UpdateChoice,
) -> Result<UpdateReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LibraryState>();
        match choice {
            UpdateChoice::KeepMine => state.with_mut(|library| library.update(&id, choice)),
            UpdateChoice::TakeUpstream => {
                let job = state.with(|library| library.prepare_update(&id))?;
                let prepared = job.run().map_err(|e| e.to_string())?;
                state.with_mut(|library| library.finish_update(prepared))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Updates every outdated artifact without local edits; the edited ones come back as skipped.
#[tauri::command]
pub async fn update_all(app: AppHandle) -> Result<Vec<UpdateReport>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LibraryState>();
        let plan = state.with(|library| library.plan_update_all())?;
        Ok(plan.run(|prepared| {
            // A failed copy is reported for that artifact only; the lock error text stays.
            state
                .with_mut(|library| library.finish_update(prepared))
                .map_err(|message| harneo_core::Error::Update { message })
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Links, bundles and settings ---------------------------------------------------------

/// What an artifact needs and what needs it.
#[tauri::command]
pub fn links_of(state: State<'_, LibraryState>, id: String) -> Result<Links, String> {
    state.with(|library| library.links_of(&id))
}

/// Confirms `from needs to` (origin `manual` or `ai`; reference links come from import).
#[tauri::command]
pub fn link_add(
    state: State<'_, LibraryState>,
    from: String,
    to: String,
    origin: LinkOrigin,
    reason: String,
) -> Result<(), String> {
    state.with(|library| library.add_link(&from, &to, origin, &reason))
}

/// Deletes a link.
#[tauri::command]
pub fn link_remove(state: State<'_, LibraryState>, from: String, to: String) -> Result<(), String> {
    state.with(|library| library.remove_link(&from, &to))
}

/// Remembers a refused proposal so it is not offered again.
#[tauri::command]
pub fn link_reject(state: State<'_, LibraryState>, from: String, to: String) -> Result<(), String> {
    state.with(|library| library.reject_link(&from, &to))
}

/// The artifact's bundle (transitive, by depth then name), without itself.
#[tauri::command]
pub fn bundle_of(state: State<'_, LibraryState>, id: String) -> Result<Vec<String>, String> {
    state.with(|library| library.bundle_of(&id))
}

/// One app setting, if set.
#[tauri::command]
pub fn settings_get(
    state: State<'_, LibraryState>,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    state.with(|library| library.settings_get(&key))
}

/// Stores one app setting.
#[tauri::command]
pub fn settings_set(
    state: State<'_, LibraryState>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    state.with(|library| library.settings_set(&key, &value))
}

// --- Claude detection ------------------------------------------------------------------

/// Whether the `claude` CLI is on the PATH, and its version.
#[tauri::command]
pub async fn claude_cli_status() -> Result<CliStatus, String> {
    tauri::async_runtime::spawn_blocking(|| cli_status(&ClaudeConfig::default()))
        .await
        .map_err(|e| e.to_string())
}

/// Event name for detection progress; the payload is `DetectProgress`.
const DETECT_PROGRESS_EVENT: &str = "bundles-detect-progress";

/// Asks Claude for links and tags over the ticked candidates of a preview. The library is read
/// first and released; the slow Claude calls run on the blocking pool without it.
#[tauri::command]
pub async fn bundles_detect_preview(
    app: AppHandle,
    preview: Preview,
    selection: Vec<String>,
) -> Result<Detection, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let job = app
            .state::<LibraryState>()
            .with(|library| library.prepare_preview_detection(&preview, &selection))?;
        job.run(&|done, total| {
            let _ = app.emit(DETECT_PROGRESS_EVENT, DetectProgress::new(done, total));
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Asks Claude for links and tags over stored copies (`ids`, or the whole library when null).
#[tauri::command]
pub async fn bundles_detect_library(
    app: AppHandle,
    ids: Option<Vec<String>>,
) -> Result<Detection, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let job = app
            .state::<LibraryState>()
            .with(|library| library.prepare_library_detection(ids.as_deref()))?;
        job.run(&|done, total| {
            let _ = app.emit(DETECT_PROGRESS_EVENT, DetectProgress::new(done, total));
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Collections -----------------------------------------------------------------------

/// Every collection with its member count.
#[tauri::command]
pub fn collections_list(state: State<'_, LibraryState>) -> Result<Vec<Collection>, String> {
    state.with(|library| library.collections())
}

/// Creates an empty manual collection.
#[tauri::command]
pub fn collection_create_manual(
    state: State<'_, LibraryState>,
    name: String,
) -> Result<Collection, String> {
    state.with(|library| library.create_manual_collection(&name))
}

/// Creates a smart collection from a rule.
#[tauri::command]
pub fn collection_create_smart(
    state: State<'_, LibraryState>,
    name: String,
    rule: Filter,
) -> Result<Collection, String> {
    state.with(|library| library.create_smart_collection(&name, rule))
}

/// Renames a collection.
#[tauri::command]
pub fn collection_rename(
    state: State<'_, LibraryState>,
    id: i64,
    name: String,
) -> Result<Collection, String> {
    state.with(|library| library.rename_collection(id, &name))
}

/// Replaces a smart collection's rule.
#[tauri::command]
pub fn collection_update_rule(
    state: State<'_, LibraryState>,
    id: i64,
    rule: Filter,
) -> Result<Collection, String> {
    state.with(|library| library.update_rule(id, rule))
}

/// Deletes a collection; its artifacts stay in the library.
#[tauri::command]
pub fn collection_delete(state: State<'_, LibraryState>, id: i64) -> Result<(), String> {
    state.with(|library| library.delete_collection(id))
}

/// Adds an artifact to a manual collection.
#[tauri::command]
pub fn collection_add(
    state: State<'_, LibraryState>,
    collection_id: i64,
    artifact_id: String,
) -> Result<(), String> {
    state.with(|library| library.add_to_collection(collection_id, &artifact_id))
}

/// Takes an artifact out of a manual collection.
#[tauri::command]
pub fn collection_remove(
    state: State<'_, LibraryState>,
    collection_id: i64,
    artifact_id: String,
) -> Result<(), String> {
    state.with(|library| library.remove_from_collection(collection_id, &artifact_id))
}

/// One page of a collection's members.
#[tauri::command]
pub fn collection_page(
    state: State<'_, LibraryState>,
    collection_id: i64,
    query: Query,
) -> Result<Page, String> {
    state.with(|library| library.collection_page(collection_id, &query))
}

/// The ids of the manual collections an artifact is in.
#[tauri::command]
pub fn artifact_collections(
    state: State<'_, LibraryState>,
    id: String,
) -> Result<Vec<i64>, String> {
    state.with(|library| library.artifact_collections(&id))
}

// --- Usage (Claude's session logs, names only) ------------------------------------------

/// Event name for index progress; the payload is `IndexProgress`.
const USAGE_PROGRESS_EVENT: &str = "usage-index-progress";

/// The usage index, opened once at startup. Kept apart from the library: its own database, and
/// an open failure is handed to the UI by each usage command.
pub struct UsageState(Result<UsageIndex, String>);

impl UsageState {
    /// Opens the usage index at the app's data dir.
    pub fn open() -> Self {
        Self(
            harneo_core::paths::data_dir()
                .and_then(|dir| UsageIndex::open(&dir))
                .map_err(|e| e.to_string()),
        )
    }

    fn index(&self) -> Result<&UsageIndex, String> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

/// Reads new lines of the session logs under `~/.claude/projects`, emitting
/// `usage-index-progress` after each file.
#[tauri::command]
pub async fn usage_index(app: AppHandle) -> Result<IndexReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let logs = harneo_core::paths::claude_home()
            .map_err(|e| e.to_string())?
            .join("projects");
        let state = app.state::<UsageState>();
        let events = app.clone();
        state
            .index()?
            .index(&logs, move |progress| {
                let _ = events.emit(USAGE_PROGRESS_EVENT, progress);
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sessions, weekly graph, last use and projects of one artifact (matched by kind and name).
#[tauri::command]
pub fn usage_of(
    state: State<'_, UsageState>,
    kind: ArtifactKind,
    name: String,
) -> Result<Usage, String> {
    state
        .index()?
        .usage_of(kind, &name)
        .map_err(|e| e.to_string())
}

/// Sessions per week over all tracked uses.
#[tauri::command]
pub fn usage_summary(state: State<'_, UsageState>) -> Result<UsageSummary, String> {
    state.index()?.usage_summary().map_err(|e| e.to_string())
}

// --- Profiles --------------------------------------------------------------------------

/// Every profile with what its current version holds.
#[tauri::command]
pub fn profiles_list(state: State<'_, LibraryState>) -> Result<Vec<ProfileSummary>, String> {
    state.with(|library| library.profiles())
}

/// A profile at `version` (the current one when absent), with its history.
#[tauri::command]
pub fn profile_get(
    state: State<'_, LibraryState>,
    id: ProfileId,
    version: Option<u32>,
) -> Result<ProfileView, String> {
    state.with(|library| library.profile(id, version))
}

/// Creates a profile at version 1.
#[tauri::command]
pub fn profile_create(
    state: State<'_, LibraryState>,
    name: String,
    description: String,
    spec: ProfileSpec,
) -> Result<ProfileId, String> {
    state.with(|library| library.create_profile(&name, &description, &spec))
}

/// Saves the spec as the next version; resolves with the version now current.
#[tauri::command]
pub fn profile_save(
    state: State<'_, LibraryState>,
    id: ProfileId,
    spec: ProfileSpec,
) -> Result<u32, String> {
    state.with(|library| library.save_profile(id, &spec))
}

/// Renames a profile (not a new version).
#[tauri::command]
pub fn profile_rename(
    state: State<'_, LibraryState>,
    id: ProfileId,
    name: String,
) -> Result<(), String> {
    state.with(|library| library.rename_profile(id, &name))
}

/// Changes a profile's description (not a new version).
#[tauri::command]
pub fn profile_describe(
    state: State<'_, LibraryState>,
    id: ProfileId,
    description: String,
) -> Result<(), String> {
    state.with(|library| library.set_profile_description(id, &description))
}

/// Deletes a profile and its versions.
#[tauri::command]
pub fn profile_delete(state: State<'_, LibraryState>, id: ProfileId) -> Result<(), String> {
    state.with(|library| library.delete_profile(id))
}

/// Copies a profile under a new name.
#[tauri::command]
pub fn profile_clone(
    state: State<'_, LibraryState>,
    id: ProfileId,
    name: String,
) -> Result<ProfileId, String> {
    state.with(|library| library.clone_profile(id, &name))
}

/// A new profile holding a collection's artifacts.
#[tauri::command]
pub fn profile_from_collection(
    state: State<'_, LibraryState>,
    collection_id: i64,
    name: String,
) -> Result<ProfileId, String> {
    state.with(|library| library.profile_from_collection(collection_id, &name))
}

/// What a spec installs: bundles pulled in, Settings and MCP servers, missing ids.
#[tauri::command]
pub fn profile_resolve(
    state: State<'_, LibraryState>,
    spec: ProfileSpec,
) -> Result<Resolved, String> {
    state.with(|library| library.resolve(&spec))
}

/// The file tree a spec installs, with conflicts.
#[tauri::command]
pub fn profile_layout(state: State<'_, LibraryState>, spec: ProfileSpec) -> Result<Layout, String> {
    state.with(|library| library.layout(&spec))
}

/// Writes the profile file to `path` (the UI's native save dialog picked it). The bytes are
/// written here with `std::fs`, never through a shell.
#[tauri::command]
pub fn profile_export(
    state: State<'_, LibraryState>,
    id: ProfileId,
    version: Option<u32>,
    path: String,
) -> Result<(), String> {
    let bytes = state.with(|library| library.export_profile(id, version))?;
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Reads a profile file the user picked, refusing one over the size cap before reading it.
fn read_profile_file(path: &str) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    if meta.len() > MAX_FILE_BYTES as u64 {
        return Err("the profile file is larger than 50 MiB".to_owned());
    }
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// What importing the profile file at `path` would do.
#[tauri::command]
pub fn profile_import_preview(
    state: State<'_, LibraryState>,
    path: String,
) -> Result<ProfileImportPreview, String> {
    let bytes = read_profile_file(&path)?;
    state.with(|library| library.import_profile_file(&bytes))
}

/// Imports the profile file at `path`; `name` overrides the file's name.
#[tauri::command]
pub fn profile_import_commit(
    state: State<'_, LibraryState>,
    path: String,
    name: Option<String>,
) -> Result<ProfileImportReport, String> {
    let bytes = read_profile_file(&path)?;
    state.with_mut(|library| library.import_profile_file_commit(&bytes, name.as_deref()))
}

/// Sets a skill's library default for manual-only invocation.
#[tauri::command]
pub fn artifact_set_manual_only(
    state: State<'_, LibraryState>,
    id: String,
    manual_only: bool,
) -> Result<(), String> {
    state.with(|library| library.set_manual_only(&id, manual_only))
}
// --- Install engine: scaffold, apply, projects ------------------------------------------

/// What scaffolding a profile into a folder would write. Writes nothing.
#[tauri::command]
pub fn scaffold_plan(
    state: State<'_, LibraryState>,
    target: String,
    profile_id: i64,
    version: Option<u32>,
) -> Result<Plan, String> {
    state.with(|library| library.plan_scaffold(std::path::Path::new(&target), profile_id, version))
}

/// Compares a project's harness with a profile, item by item. Reads the project, writes nothing.
/// Runs on the blocking pool: it scans the folder.
#[tauri::command]
pub async fn refit_compare(
    app: AppHandle,
    target: String,
    profile_id: i64,
    version: Option<u32>,
) -> Result<Comparison, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.compare(std::path::Path::new(&target), profile_id, version))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The plan for the user's choices (item to action; an item without one takes its default).
#[tauri::command]
pub async fn refit_plan(
    app: AppHandle,
    target: String,
    profile_id: i64,
    version: Option<u32>,
    choices: std::collections::BTreeMap<String, RefitAction>,
) -> Result<Plan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>().with(|library| {
            library.refit_plan(std::path::Path::new(&target), profile_id, version, &choices)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Writes a plan into its project, backing up first and rolling back on a failure. Runs on the
/// blocking pool: it may wait for the data-dir lock.
#[tauri::command]
pub async fn install_apply(app: AppHandle, plan: Plan) -> Result<ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.apply(&plan))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Every project Harneo installed a profile into.
#[tauri::command]
pub fn projects_list(state: State<'_, LibraryState>) -> Result<Vec<Project>, String> {
    state.with(|library| library.projects())
}

/// One project.
#[tauri::command]
pub fn project_get(state: State<'_, LibraryState>, id: i64) -> Result<Project, String> {
    state.with(|library| library.project(id))
}

/// The manifest of a project: the files Harneo wrote.
#[tauri::command]
pub fn project_files(
    state: State<'_, LibraryState>,
    id: i64,
) -> Result<Vec<InstalledFile>, String> {
    state.with(|library| library.installed_files(id))
}

/// The current content of one managed file of a project (cut at a size cap).
#[tauri::command]
pub fn project_file_content(
    state: State<'_, LibraryState>,
    id: i64,
    path: String,
) -> Result<ProjectFile, String> {
    state.with(|library| library.project_file_content(id, &path))
}

/// The profiles whose current version installs the artifact.
#[tauri::command]
pub fn artifact_profiles(
    state: State<'_, LibraryState>,
    id: String,
) -> Result<Vec<ProfileUse>, String> {
    state.with(|library| library.profiles_using(&id))
}
/// The plan to move a project to another profile or version: only the difference.
#[tauri::command]
pub fn switch_plan(
    state: State<'_, LibraryState>,
    project_id: i64,
    profile_id: i64,
    version: Option<u32>,
) -> Result<Plan, String> {
    state.with(|library| library.plan_switch(project_id, profile_id, version))
}

/// The plan to remove a project's profile: Harneo's files out, the originals back.
#[tauri::command]
pub fn removal_plan(state: State<'_, LibraryState>, project_id: i64) -> Result<Plan, String> {
    state.with(|library| library.plan_removal(project_id))
}

/// The backup sets of a project, newest first.
#[tauri::command]
pub fn project_backups(state: State<'_, LibraryState>, id: i64) -> Result<Vec<Backup>, String> {
    state.with(|library| library.backups(id))
}

/// Puts a backup set back into its project (the current versions are backed up first). Runs on
/// the blocking pool: it may wait for the data-dir lock.
#[tauri::command]
pub async fn backup_restore(app: AppHandle, backup_id: i64) -> Result<ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.restore_backup(backup_id))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Deletes a backup set from disk and from the list.
#[tauri::command]
pub async fn backup_delete(app: AppHandle, backup_id: i64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryState>()
            .with(|library| library.delete_backup(backup_id))
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
    fn every_profile_command_runs_through_managed_state() -> Result<(), Box<dyn std::error::Error>>
    {
        let data = tempfile::tempdir()?;
        let source = tempfile::tempdir()?;
        std::fs::create_dir_all(source.path().join("skills/alpha"))?;
        std::fs::write(
            source.path().join("skills/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: A skill\n---\nBody\n",
        )?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(data.path())?)));
        let state = || app.state::<LibraryState>();
        let src = Source::Local {
            path: source.path().to_path_buf(),
        };
        let preview = state().with(|l| l.preview(&src))?;
        let ids = vec![preview.candidates[0].id.clone()];
        state().with_mut(|l| l.import(&preview, &ids, &HashMap::new()))?;
        let alpha = ids[0].clone();

        assert!(profiles_list(state())?.is_empty());
        let spec = ProfileSpec {
            items: vec![alpha.clone()],
            ..ProfileSpec::default()
        };
        let id = profile_create(state(), "Light".into(), "d".into(), spec.clone())?;
        assert_eq!(profiles_list(state())?.len(), 1);
        assert_eq!(profile_get(state(), id, None)?.name, "Light");
        assert_eq!(profile_save(state(), id, spec.clone())?, 1);
        profile_rename(state(), id, "Lighter".into())?;
        profile_describe(state(), id, "new".into())?;
        assert_eq!(profile_get(state(), id, Some(1))?.description, "new");
        assert_eq!(profile_resolve(state(), spec.clone())?.artifacts.len(), 1);
        assert!(!profile_layout(state(), spec)?.paths.is_empty());
        artifact_set_manual_only(state(), alpha.clone(), true)?;
        assert_eq!(artifact_profiles(state(), alpha.clone())?.len(), 1);

        let copy = profile_clone(state(), id, "Copy".into())?;
        let manual = state().with(|l| l.create_manual_collection("kit"))?;
        state().with(|l| l.add_to_collection(manual.id, &alpha))?;
        let from = profile_from_collection(state(), manual.id, "From kit".into())?;

        let file = data.path().join("light.harneo-profile.json");
        let path = file.to_string_lossy().into_owned();
        profile_export(state(), id, None, path.clone())?;
        assert!(file.is_file());
        let shown = profile_import_preview(state(), path.clone())?;
        assert!(shown.name_clash);
        let report = profile_import_commit(state(), path, Some("Imported".into()))?;
        assert_eq!(report.name, "Imported");

        for gone in [id, copy, from, report.profile_id] {
            profile_delete(state(), gone)?;
        }
        assert!(profiles_list(state())?.is_empty());
        assert!(profile_get(state(), id, None).is_err());
        Ok(())
    }

    #[test]
    fn read_profile_file_reads_and_reports_missing() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("a.harneo-profile.json");
        std::fs::write(&file, b"{}")?;
        assert_eq!(read_profile_file(&file.to_string_lossy())?, b"{}");
        let err = read_profile_file(&dir.path().join("nope").to_string_lossy()).err();
        assert!(err.is_some_and(|m| m.contains("cannot read")));
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
    fn tag_commands_create_assign_and_report_duplicates() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let state = LibraryState::from_result(Ok(Library::open(dir.path())?));
        let tag = state.with(|l| l.create_tag("rust", TagColor::Cyan))?;
        let dup = state.with(|l| l.create_tag("Rust", TagColor::Sky)).err();
        assert!(dup.is_some_and(|m| m.contains("already")));
        let json = serde_json::to_value(state.with(|l| l.tags())?)?;
        assert_eq!(json[0]["name"], "rust");
        assert_eq!(json[0]["count"], 0);
        let unknown = state.with(|l| l.tag_artifact("nope", tag.id)).err();
        assert!(unknown.is_some());
        Ok(())
    }

    #[test]
    fn every_tag_command_runs_through_managed_state() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir.path())?)));
        let state = || app.state::<LibraryState>();
        let tag = tag_create(state(), "rust".into(), TagColor::Cyan)?;
        assert_eq!(tag_rename(state(), tag.id, "rusty".into())?.name, "rusty");
        assert_eq!(
            tag_recolor(state(), tag.id, TagColor::Sky)?.color,
            TagColor::Sky
        );
        assert_eq!(tags_list(state())?.len(), 1);
        assert!(artifact_tags(state(), "nope".into())?.is_empty());
        assert!(artifact_tag_add(state(), "nope".into(), tag.id).is_err());
        assert!(artifact_tag_remove(state(), "nope".into(), tag.id).is_ok());
        tag_delete(state(), tag.id)?;
        assert!(tags_list(state())?.is_empty());
        Ok(())
    }

    #[test]
    fn link_and_settings_commands_run_through_managed_state(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir.path())?)));
        let state = || app.state::<LibraryState>();
        assert!(links_of(state(), "nope".into()).is_err());
        assert!(link_add(
            state(),
            "a".into(),
            "b".into(),
            LinkOrigin::Manual,
            String::new()
        )
        .is_err());
        assert!(link_remove(state(), "a".into(), "b".into()).is_ok());
        assert!(link_reject(state(), "a".into(), "b".into()).is_err());
        assert!(bundle_of(state(), "nope".into()).is_err());
        assert_eq!(settings_get(state(), "bundles.model".into())?, None);
        settings_set(state(), "bundles.model".into(), serde_json::json!("haiku"))?;
        assert_eq!(
            settings_get(state(), "bundles.model".into())?,
            Some(serde_json::json!("haiku"))
        );
        Ok(())
    }

    #[test]
    fn update_status_runs_through_managed_state() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir.path())?)));
        assert!(update_status(app.state::<LibraryState>(), "nope".into()).is_err());
        Ok(())
    }

    #[test]
    fn every_collection_command_runs_through_managed_state(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir.path())?)));
        let state = || app.state::<LibraryState>();
        let manual = collection_create_manual(state(), "kit".into())?;
        let rule: Filter = serde_json::from_value(serde_json::json!({
            "types": ["skill"], "tags": [], "sources": [], "text": null
        }))?;
        let smart = collection_create_smart(state(), "skills".into(), rule.clone())?;
        assert_eq!(collections_list(state())?.len(), 2);
        assert_eq!(
            collection_rename(state(), manual.id, "kit 2".into())?.name,
            "kit 2"
        );
        assert!(collection_update_rule(state(), smart.id, rule).is_ok());
        assert!(collection_add(state(), manual.id, "nope".into()).is_err());
        assert!(collection_remove(state(), manual.id, "nope".into()).is_ok());
        assert_eq!(
            collection_page(state(), manual.id, Query::default())?.total,
            0
        );
        assert!(artifact_collections(state(), "nope".into())?.is_empty());
        collection_delete(state(), manual.id)?;
        assert_eq!(collections_list(state())?.len(), 1);
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

    #[test]
    fn install_commands_run_through_managed_state() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let app = tauri::test::mock_app();
        app.manage(LibraryState::from_result(Ok(Library::open(dir.path())?)));
        let state = || app.state::<LibraryState>();
        let target = tempfile::tempdir()?;
        let target_text = target.path().to_string_lossy().into_owned();
        assert!(scaffold_plan(state(), target_text, 1, None).is_err());
        assert!(projects_list(state())?.is_empty());
        assert!(project_get(state(), 1).is_err());
        assert!(project_files(state(), 1).is_err());
        assert!(project_file_content(state(), 1, "a".into()).is_err());
        assert!(switch_plan(state(), 1, 1, None).is_err());
        assert!(removal_plan(state(), 1).is_err());
        assert!(project_backups(state(), 1).is_err());
        Ok(())
    }
}
