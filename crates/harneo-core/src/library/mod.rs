//! The library: the user's imported artifacts, kept in the data folder as file copies plus a
//! SQLite index.
//!
//! ```text
//! <data dir>/library/library.db        the index (see `store`)
//! <data dir>/library/artifacts/<id>/   one copy per artifact (see `ids` for the id)
//! ```
//!
//! Importing is two steps: [`Library::preview`] scans a [`Source`] and lists [`Candidate`]s,
//! then [`Library::import`] copies the ticked ones (see `copy`).

#[cfg(test)]
mod audit_tests;
mod audits;
#[cfg(test)]
mod audits_tests;
mod collections;
#[cfg(test)]
mod collections_tests;
pub(crate) mod confirm;
#[cfg(test)]
mod confirm_tests;
mod copies;
#[cfg(test)]
mod copies_tests;
mod copy;
#[cfg(test)]
mod find_tests;
pub(crate) mod git;
#[cfg(test)]
mod git_tests;
pub mod ids;
#[cfg(test)]
mod import_tests;
pub(crate) mod layout_claude;
#[cfg(test)]
mod layout_claude_tests;
mod origin;
mod query;
#[cfg(test)]
mod query_tests;
pub(crate) mod readout;
pub(crate) mod records;
mod restore;
#[cfg(test)]
mod secrets_tests;
mod source;
mod store;
mod tags;
#[cfg(test)]
mod tags_tests;
mod unpack;
#[cfg(test)]
pub(crate) mod unpack_tests;
mod viewer;
#[cfg(test)]
mod viewer_tests;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::{
    AgentAdapter, Artifact, ArtifactKind, ClaudeAdapter, Diagnostic, ScanOptions, ScanReport,
};
use crate::audit::{self, Audit};
use crate::error::{Error, Result};
pub use audits::ArtifactAudit;
pub use collections::{Collection, CollectionKind};
pub use confirm::{
    ArtifactConfirmation, ConfirmJob, ConfirmOutcome, ConfirmProgress, ConfirmScope,
};
pub use copy::LARGE_FILE_CEILING;
pub(crate) use copy::{copy_artifact, copy_limit, ImportHooks, NoHooks};
pub(crate) use copy::{is_executable, make_executable, rewrite, safe_join, stored_view};
pub use copy::{SkipReason, SkippedFile};
use git::{Checkout, GitConfig};
pub use layout_claude::{IgnoredPath, LayoutJob, LayoutStage, Proposal};
pub(crate) use origin::derive_plugin_origin;
pub(crate) use query::source_label;
pub use query::{
    Facets, Filter, KindCount, LibraryItem, Page, Query, SortKey, SourceCount, SourceSummary,
    StatusCount, TagChip, TagCount, TierCount, MAX_LIMIT,
};
pub use records::{ArtifactDetail, Provenance, SourceRecord};
pub(crate) use restore::{Restored, PROFILE_FILE_SOURCE};
pub use source::Source;
use store::Store;
pub use tags::{Tag, TagChoices, TagColor, TagWithCount};
pub use unpack::PluginHold;
pub use viewer::{may_run_when_opened, FileView, MAX_VIEW_BYTES};

/// An artifact id: 16 lowercase hex characters (see [`ids`]).
pub type ArtifactId = String;

/// One artifact a preview found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Candidate {
    pub id: ArtifactId,
    pub artifact: Artifact,
    /// Already imported from this source: shown unticked and skipped by `import`.
    pub already_in_library: bool,
    /// `env.<KEY>` / `headers.<KEY>` entries whose literal secrets the copy will replace by
    /// `${KEY}` placeholders. Key names only, never values.
    pub replaced_secret_keys: Vec<String>,
    /// Names from the artifact's frontmatter `keywords` / `tags`, offered as tags.
    pub suggested_tags: Vec<String>,
    /// Paths of identical copies (same kind, name and digest) folded into this candidate.
    pub copies: Vec<String>,
    /// The security audit of the artifact in its source (for a git source, inside the clone).
    /// `import` stores this one.
    pub audit: Audit,
    /// For a Plugin: why it can only be taken whole, when it can (`plugin::must_stay_whole`).
    #[serde(default)]
    #[ts(optional)]
    pub must_stay_whole: Option<crate::plugin::WholeReason>,
}

/// What [`Library::reaudit_all`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ReauditReport {
    /// Artifacts audited (those whose stored copy could be read).
    pub audited: u32,
    /// Of those, how many now have a different audit than before (a never-audited artifact
    /// counts as changed).
    pub changed: u32,
}

/// What a source holds, before anything is copied. Hand it back to [`Library::import`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Preview {
    pub source: Source,
    pub candidates: Vec<Candidate>,
    /// The scan's findings that belong to no single artifact.
    pub diagnostics: Vec<Diagnostic>,
    /// Files that might be artifacts but lie outside every path tiers 1 and 2 claimed (paths
    /// only, capped by the scan). Tier 3 may place or hide them.
    #[serde(default)]
    #[ts(optional = false)]
    pub unplaced: Vec<String>,
    /// How many more unplaced paths there were beyond the scan's cap.
    #[serde(default)]
    #[ts(optional = false)]
    pub unplaced_overflow: u32,
    /// Paths Claude said to ignore in tier 3: they no longer count as unplaced.
    #[serde(default)]
    #[ts(optional = false)]
    pub layout_ignored: Vec<layout_claude::IgnoredPath>,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ImportReport {
    /// The library's id for the source, as `Query` filters (`source`) and `SourceSummary` use it.
    pub source_id: String,
    /// Newly stored, in selection order.
    pub imported: Vec<ArtifactId>,
    /// Selected but already in the library, so left untouched.
    pub already_in_library: Vec<ArtifactId>,
    /// Listed files that were not copied (symlinks, secret-named files...).
    pub skipped_files: Vec<SkippedFile>,
}

/// An open library. Opening creates the folders and the schema when they are missing.
#[derive(Debug)]
pub struct Library {
    pub(crate) store: Store,
    pub(crate) git: GitConfig,
    /// How the `claude` CLI is run for bundle detection; tests swap in a fixture script.
    pub(crate) claude: crate::bundles::ClaudeConfig,
    /// The clone of the last git preview, kept until `import` copies from it. At most one: a
    /// newer git preview replaces (and deletes) it, `import` consumes it, and dropping the
    /// library deletes it. See [`PendingClone`].
    pending: Mutex<Option<PendingClone>>,
    /// Clones the update check made, kept for the session (see `updates::remote`).
    pub(crate) session: std::sync::Arc<crate::updates::remote::Session>,
}

/// A git preview's clone, waiting for `import`. Previews are plain data that travel over IPC,
/// so the clone is found again through the source's identity (URL, resolved branch,
/// subfolder); the recorded commit comes from here, never from the preview the caller hands
/// back.
#[derive(Debug)]
struct PendingClone {
    identity: String,
    checkout: Checkout,
}

impl Library {
    /// Opens (creating it if needed) the library under `data_dir` and migrates its schema
    /// forward to the current version.
    pub fn open(data_dir: &Path) -> Result<Self> {
        let store = Store::open(data_dir)?;
        crate::usage::attach(&store.conn, data_dir)?;
        Ok(Self {
            store,
            git: GitConfig::default(),
            claude: crate::bundles::ClaudeConfig::default(),
            pending: Mutex::new(None),
            session: std::sync::Arc::default(),
        })
    }

    /// The schema version stored in the database (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<u32> {
        self.store.schema_version()
    }

    /// Scans `source` and lists what could be imported. Copies nothing. A git source is
    /// cloned first (shallow) and the returned preview names the resolved branch; the clone
    /// stays until `import`, the next git preview or the library being dropped.
    pub fn preview(&self, source: &Source) -> Result<Preview> {
        self.preview_with(source, &ScanOptions::default())
    }

    /// The plugins of `preview` (whole or unpacked) with what the library already holds of each,
    /// so the window can refuse a choice before import. Read-only.
    pub fn plugin_holds(&self, preview: &Preview) -> Result<Vec<PluginHold>> {
        unpack::holds(&self.store.conn, &preview.candidates)
    }

    /// [`Self::preview`] with the scan's options (for example `include_managed`).
    pub fn preview_with(&self, source: &Source, options: &ScanOptions) -> Result<Preview> {
        match source {
            Source::Local { path } => {
                let identity = source.identity()?;
                self.scan_candidates(source, &identity, path, options)
            }
            Source::Git {
                url,
                branch,
                subfolder,
            } => self.git_preview(url, branch.as_deref(), subfolder.as_deref(), options),
        }
    }

    fn git_preview(
        &self,
        url: &str,
        branch: Option<&str>,
        subfolder: Option<&str>,
        options: &ScanOptions,
    ) -> Result<Preview> {
        let relative = git_subfolder(subfolder)?;
        let checkout = git::clone(&self.git, self.store.root(), url, branch)?;
        let resolved = Source::Git {
            url: url.to_owned(),
            branch: checkout.branch.clone(),
            subfolder: subfolder
                .map(|s| s.trim().trim_matches('/'))
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        };
        let identity = resolved.identity()?;
        let root = checkout.path().join(&relative);
        let shown = subfolder.unwrap_or_default().to_owned();
        let inside = root
            .canonicalize()
            .ok()
            .zip(checkout.path().canonicalize().ok())
            .is_some_and(|(root, base)| root.is_dir() && root.starts_with(base));
        if !inside {
            return Err(if root.exists() {
                Error::UnsafePath { path: shown }
            } else {
                Error::GitSubfolderMissing { subfolder: shown }
            });
        }
        let preview = self.scan_candidates(&resolved, &identity, &root, options)?;
        // Unplaced files alone are still worth a preview: they are what tier 3 asks Claude about.
        if preview.candidates.is_empty() && preview.unplaced.is_empty() {
            return Err(Error::NothingFound {
                source_name: source::redact(url),
            });
        }
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(PendingClone { identity, checkout });
        Ok(preview)
    }

    /// Runs `f` with the folder a preview's candidates are read from (the local folder, or the
    /// pending clone, which stays for `import`).
    pub(crate) fn with_preview_root<T>(
        &self,
        preview: &Preview,
        f: impl FnOnce(&Path) -> Result<T>,
    ) -> Result<T> {
        match &preview.source {
            Source::Local { path } => f(path),
            Source::Git { subfolder, .. } => {
                let identity = preview.source.identity()?;
                let slot = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
                match slot.as_ref() {
                    Some(pending) if pending.identity == identity => f(&pending
                        .checkout
                        .path()
                        .join(git_subfolder(subfolder.as_deref())?)),
                    _ => Err(Error::PreviewExpired),
                }
            }
        }
    }

    /// The commit a git preview was read at, or an empty text for a local folder or when the
    /// clone is no longer held.
    pub(crate) fn preview_commit(&self, preview: &Preview) -> String {
        let Ok(identity) = preview.source.identity() else {
            return String::new();
        };
        let slot = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.as_ref() {
            Some(pending) if pending.identity == identity => pending.checkout.commit.clone(),
            _ => String::new(),
        }
    }

    /// The clone behind a git preview, taken out of the slot so it is deleted when the caller
    /// is done with it, however that goes.
    fn take_pending(&self, identity: &str) -> Result<PendingClone> {
        let mut slot = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.take() {
            Some(pending) if pending.identity == identity => Ok(pending),
            other => {
                *slot = other;
                Err(Error::PreviewExpired)
            }
        }
    }

    fn scan_candidates(
        &self,
        source: &Source,
        identity: &str,
        root: &Path,
        options: &ScanOptions,
    ) -> Result<Preview> {
        let git_repo = match source {
            Source::Git { url, .. } => crate::artifact::github_repo(url),
            Source::Local { .. } => None,
        };
        let options = &ScanOptions {
            repo: git_repo.or_else(|| options.repo.clone()),
            ..options.clone()
        };
        let mut report = ClaudeAdapter.scan(root, options)?;
        let by_default = self.unpacked_by_default(identity, root, options, &report)?;
        if !by_default.is_empty() {
            let mut unpack = options.unpack.clone();
            unpack.extend(by_default);
            let again = ScanOptions {
                unpack,
                ..options.clone()
            };
            report = ClaudeAdapter.scan(root, &again)?;
        }
        // A source imported before replays what Claude placed in it, without asking again.
        let lost = self.replay_stored_layout(identity, root, &mut report)?;
        let (kind, location) = match source {
            Source::Git { url, .. } => ("git", url.as_str()),
            Source::Local { path } => ("local", path.to_str().unwrap_or_default()),
        };
        for artifact in &mut report.artifacts {
            if let Some(origin) = origin::derive_plugin_origin(artifact, kind, location) {
                artifact.origin = origin;
            }
        }
        let mut diagnostics = report.diagnostics;
        for path in lost {
            diagnostics.push(Diagnostic::warning(
                "layout.changed",
                Path::new(&path),
                "Claude placed an artifact here when this source was imported, but the path is \
gone or no longer parses",
            ));
        }
        let candidates = self.candidates_of(identity, root, report.artifacts)?;
        let candidates = copies::collapse_copies(root, candidates);
        Ok(Preview {
            source: source.clone(),
            candidates,
            diagnostics,
            unplaced: report.unplaced,
            unplaced_overflow: report.unplaced_overflow,
            layout_ignored: Vec::new(),
        })
    }

    /// The plugin folders a preview unpacks on its own (ADR-004): every plugin the user made no
    /// choice about (in neither `options.unpack` nor `options.whole`), that is not in the library
    /// yet and does not have to stay whole.
    fn unpacked_by_default(
        &self,
        identity: &str,
        root: &Path,
        options: &ScanOptions,
        report: &ScanReport,
    ) -> Result<Vec<String>> {
        if !options.unpack_by_default {
            return Ok(Vec::new());
        }
        let mut walker = crate::artifact::walk::Walker::new(root, options);
        let mut folders = Vec::new();
        for plugin in report
            .artifacts
            .iter()
            .filter(|a| a.kind == ArtifactKind::Plugin)
        {
            let chosen = options
                .unpack
                .iter()
                .chain(&options.whole)
                .any(|f| same_folder(f, &plugin.path));
            if chosen || crate::plugin::must_stay_whole(&mut walker, plugin).is_some() {
                continue;
            }
            // Already imported from this source: its choice was made then, and stays.
            let id = ids::artifact_id(identity, &plugin.path);
            if records::existing(&self.store.conn, &[id.as_str()])?.is_empty() {
                folders.push(plugin.path.clone());
            }
        }
        Ok(folders)
    }

    /// Replays what Claude placed in the source named `identity` when it was imported, over a
    /// fresh scan of tiers 1 and 2. Returns the stored paths that could not be replayed.
    fn replay_stored_layout(
        &self,
        identity: &str,
        root: &Path,
        report: &mut ScanReport,
    ) -> Result<Vec<String>> {
        let layout = records::stored_layout(&self.store.conn, &ids::source_id(identity))?;
        Ok(layout.replay(root, report))
    }

    /// Candidates for scanned `artifacts` of the source named `identity`: each gets its stable
    /// id, whether it is already in the library, and its audit.
    pub(crate) fn candidates_of(
        &self,
        identity: &str,
        root: &Path,
        artifacts: Vec<Artifact>,
    ) -> Result<Vec<Candidate>> {
        let ids: Vec<String> = artifacts
            .iter()
            .map(|a| ids::artifact_id(identity, &a.path))
            .collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let known = records::existing(&self.store.conn, &id_refs)?;
        let scan_options = ScanOptions::default();
        let mut walker = crate::artifact::walk::Walker::new(root, &scan_options);
        Ok(artifacts
            .into_iter()
            .zip(ids)
            .map(|(artifact, id)| Candidate {
                already_in_library: known.contains(&id),
                replaced_secret_keys: copy::replaced_secret_keys(&artifact),
                suggested_tags: tags::suggestions(&artifact),
                copies: Vec::new(),
                audit: audit::audit(&artifact, root, &scan_options),
                must_stay_whole: crate::plugin::must_stay_whole(&mut walker, &artifact),
                id,
                artifact,
            })
            .collect())
    }

    /// Copies the selected candidates into the library. Each must be in `preview`; ones already
    /// in the library are skipped. Either everything lands or nothing does: a failed copy or
    /// commit removes the folders and rolls the index back.
    ///
    /// `tags` maps an artifact id to the tag names it gets; missing tags are created, in the
    /// same transaction as the import.
    pub fn import(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        tags: &TagChoices,
    ) -> Result<ImportReport> {
        self.import_with(preview, selection, tags, &NoHooks)
    }

    /// [`Self::import`] that also stores the links the user accepted (`from needs to`, with a
    /// reason) as `ai` links, in the same transaction. Pairs naming an unknown artifact and
    /// self-links are dropped.
    pub fn import_with_links(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        tags: &TagChoices,
        links: &[crate::bundles::AcceptedLink],
    ) -> Result<ImportReport> {
        self.import_inner(preview, selection, tags, links, false, &NoHooks)
    }

    /// [`Self::import_with_links`] that, with `large_files`, also copies files over the scan's
    /// 1 MiB limit, up to [`LARGE_FILE_CEILING`] each. They are listed as not audited, and the
    /// choice is kept on the source so updates copy them too.
    pub fn import_with_options(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        tags: &TagChoices,
        links: &[crate::bundles::AcceptedLink],
        large_files: bool,
    ) -> Result<ImportReport> {
        self.import_inner(preview, selection, tags, links, large_files, &NoHooks)
    }

    fn import_with(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        tag_choices: &TagChoices,
        hooks: &dyn ImportHooks,
    ) -> Result<ImportReport> {
        self.import_inner(preview, selection, tag_choices, &[], false, hooks)
    }

    fn import_inner(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        tag_choices: &TagChoices,
        links: &[crate::bundles::AcceptedLink],
        large_files: bool,
        hooks: &dyn ImportHooks,
    ) -> Result<ImportReport> {
        tags::validate_choices(tag_choices)?;
        let identity = preview.source.identity()?;
        let source_id = ids::source_id(&identity);
        // The preview came back over IPC, so check each id belongs to its source and path.
        let mut chosen: Vec<&Candidate> = Vec::new();
        let mut seen = HashSet::new();
        for id in selection {
            let candidate = preview
                .candidates
                .iter()
                .find(|c| &c.id == id && c.id == ids::artifact_id(&identity, &c.artifact.path))
                .ok_or_else(|| Error::NotInPreview { id: id.clone() })?;
            if seen.insert(id.as_str()) {
                chosen.push(candidate);
            }
        }
        let chosen_ids: Vec<&str> = chosen.iter().map(|c| c.id.as_str()).collect();
        let known = records::existing(&self.store.conn, &chosen_ids)?;
        let fresh_chosen: Vec<&Candidate> = chosen
            .iter()
            .copied()
            .filter(|c| !known.contains(&c.id))
            .collect();
        unpack::check(&self.store.conn, &chosen, &fresh_chosen)?;
        // A git preview's clone is consumed here and deleted when this function returns,
        // whatever the outcome.
        let clone = match &preview.source {
            Source::Local { .. } => None,
            Source::Git { .. } => Some(self.take_pending(&identity)?),
        };
        let (existing, fresh): (Vec<&Candidate>, Vec<&Candidate>) =
            chosen.into_iter().partition(|c| known.contains(&c.id));
        let mut report = ImportReport {
            source_id: source_id.clone(),
            already_in_library: existing.iter().map(|c| c.id.clone()).collect(),
            ..ImportReport::default()
        };
        if fresh.is_empty() {
            return Ok(report);
        }

        let (root, commit) = match (&preview.source, &clone) {
            (Source::Git { subfolder, .. }, Some(clone)) => (
                clone
                    .checkout
                    .path()
                    .join(git_subfolder(subfolder.as_deref())?),
                Some(clone.checkout.commit.as_str()),
            ),
            (Source::Local { path }, _) => (path.clone(), None),
            (Source::Git { .. }, None) => return Err(Error::PreviewExpired),
        };
        let root = root.as_path();
        // The stored audit is computed here, from the files about to be copied; the preview's
        // `audit` came back over IPC and is never trusted.
        let audit_options = ScanOptions::default();
        let mut audits: HashMap<&str, Audit> = fresh
            .iter()
            .map(|c| {
                (
                    c.id.as_str(),
                    audit::audit(&c.artifact, root, &audit_options),
                )
            })
            .collect();
        let staging = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        let mut replaced: HashMap<String, Vec<String>> = HashMap::new();
        for candidate in &fresh {
            let dir = staging.path().join(&candidate.id);
            fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
            let copied = copy_artifact(
                root,
                &candidate.artifact,
                &candidate.id,
                &dir,
                hooks,
                copy_limit(large_files),
            )?;
            // Only what was copied can be "not audited": a file left out is not in the library.
            if let Some(audit) = audits.get_mut(candidate.id.as_str()) {
                audit
                    .unaudited
                    .retain(|f| !copied.skipped.iter().any(|s| &s.file == f));
            }
            report.skipped_files.extend(copied.skipped);
            replaced.insert(candidate.id.clone(), copied.replaced_secret_keys);
        }

        let artifacts_dir = self.store.artifacts_dir();
        let library_root = self.store.root().to_path_buf();
        let mut placed: Vec<PathBuf> = Vec::new();
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            records::insert_source(&tx, &source_id, &preview.source, commit)?;
            if large_files {
                // Kept once chosen: a later import without it does not drop files on update.
                tx.execute(
                    "UPDATE sources SET large_files = 1 WHERE id = ?1",
                    rusqlite::params![source_id],
                )?;
            }
            let (unpacked, whole) = unpack::folders(&fresh);
            records::update_unpack(&tx, &source_id, &unpacked, &whole)?;
            records::store_layout(&tx, &source_id, &preview.layout_ignored)?;
            let now = records::now();
            for candidate in &fresh {
                let keys = replaced.get(&candidate.id).map_or(&[][..], Vec::as_slice);
                let audit =
                    audits
                        .get(candidate.id.as_str())
                        .ok_or_else(|| Error::NotInPreview {
                            id: candidate.id.clone(),
                        })?;
                records::insert_artifact(
                    &tx,
                    &candidate.id,
                    &source_id,
                    &candidate.artifact,
                    now,
                    keys,
                    audit,
                )?;
                let stored_copy = staging.path().join(&candidate.id);
                crate::updates::record_copy_digest(
                    &tx,
                    &candidate.id,
                    &crate::updates::copy_digest(&stored_copy)?,
                )?;
                if let Some(names) = tag_choices.get(&candidate.id) {
                    tags::apply_names(&tx, &candidate.id, names)?;
                }
            }
            let fresh_ids: Vec<&str> = fresh.iter().map(|c| c.id.as_str()).collect();
            crate::bundles::add_reference_links(&tx, &fresh_ids, now)?;
            crate::bundles::store_accepted(&tx, links)?;
            for candidate in &fresh {
                let target = artifacts_dir.join(&candidate.id);
                // A folder with no row is left over from a crash; it is not a library entry.
                if target.exists() {
                    fs::remove_dir_all(&target).map_err(|e| io_error(&target, e))?;
                }
                fs::rename(staging.path().join(&candidate.id), &target)
                    .map_err(|e| io_error(&target, e))?;
                placed.push(target);
            }
            hooks
                .before_commit()
                .map_err(|e| io_error(&library_root, e))?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            for folder in &placed {
                // Best effort: the error being reported matters more than a failed cleanup.
                let _ = fs::remove_dir_all(folder);
            }
            return Err(error);
        }
        report.imported = fresh.iter().map(|c| c.id.clone()).collect();
        Ok(report)
    }

    /// One library artifact with its provenance and the files of its stored copy.
    pub fn detail(&self, id: &str) -> Result<ArtifactDetail> {
        let mut detail = records::load(&self.store.conn, id)?
            .ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })?;
        detail.files = stored_files(&self.store.artifacts_dir().join(id))?;
        Ok(detail)
    }

    /// Re-runs the audit on every stored library copy (after a rule change) and stores the
    /// results. The audit runs on `<data>/library/artifacts/<id>/` with that folder as the scan
    /// root; the artifact's file list is mapped from source paths to stored paths first (see
    /// `copy::stored_view`) and the findings' files are mapped back, so they read the same as
    /// the ones the preview produced. An artifact whose copy is missing is skipped. One
    /// transaction: either every audit is stored or none.
    pub fn reaudit_all(&mut self) -> Result<ReauditReport> {
        let rows: Vec<(String, String)> = {
            let mut stmt = self
                .store
                .conn
                .prepare("SELECT id, artifact FROM artifacts ORDER BY id")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let options = ScanOptions::default();
        let artifacts_dir = self.store.artifacts_dir();
        let now = records::now();
        let enabled = records::confirm_enabled(&self.store.conn)?;
        let model = crate::bundles::model_of(self.settings_get("bundles.model")?.as_ref());
        let mut report = ReauditReport::default();
        let tx = self.store.conn.transaction()?;
        for (id, json) in rows {
            let folder = artifacts_dir.join(&id);
            if !folder.is_dir() {
                continue;
            }
            let artifact: Artifact = serde_json::from_str(&json)?;
            let phase1 = confirm::phase1_of(&artifact, &folder, &options)?;
            let candidates: HashSet<audit::Finding> =
                phase1.audit.findings.iter().cloned().collect();
            let reach = phase1.audit.reach.clone();
            // Verdicts already cached apply at once; the rest waits for Claude (`pending`).
            let fresh = if enabled {
                let cache = confirm::cached(&tx, &phase1.cands)?;
                audit::confirm::resolve_cached(phase1.audit, &phase1.cands, &cache, &model, now)
            } else {
                audit::confirm::off(phase1.audit)
            };
            let before = records::load_audit(&tx, &id)?.map(|(a, _)| a);
            records::update_audit(&tx, &id, &fresh, now)?;
            report.audited += 1;
            let same = before.as_ref().is_some_and(|b| {
                b.score == fresh.score
                    && b.reach == reach
                    && confirm::candidate_set(b) == candidates
            });
            if !same {
                report.changed += 1;
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Deletes an artifact's copy and its index rows (tags and collection entries go with them).
    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.remove_many(&[id.to_owned()])
    }

    /// Deletes several artifacts by the rules of [`Library::remove`], all or nothing: when one
    /// fails (an unknown id, a disk error), none is removed.
    pub fn remove_many(&mut self, ids: &[String]) -> Result<()> {
        let mut unique: Vec<&str> = Vec::new();
        for id in ids {
            // Validate before touching the disk: a crafted id (`..`, a path) must never reach a
            // rename.
            let is_hex =
                id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
            if !is_hex || records::load(&self.store.conn, id)?.is_none() {
                return Err(Error::NotInLibrary { id: id.clone() });
            }
            if !unique.contains(&id.as_str()) {
                unique.push(id);
            }
        }
        let trash = tempfile::Builder::new()
            .prefix(".remove-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        // Park the folders first: a rename is atomic, so a failure below can put them back.
        let mut parked: Vec<(PathBuf, PathBuf)> = Vec::new();
        let put_back = |parked: &[(PathBuf, PathBuf)]| {
            for (from, to) in parked.iter().rev() {
                let _ = fs::rename(to, from);
            }
        };
        for id in &unique {
            let target = self.store.artifacts_dir().join(id);
            let to = trash.path().join(id);
            match fs::rename(&target, &to) {
                Ok(()) => parked.push((target, to)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    put_back(&parked);
                    return Err(io_error(&target, e));
                }
            }
        }
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            crate::profiles::remove_artifacts(&tx, &unique)?;
            for id in &unique {
                if !records::delete(&tx, id)? {
                    return Err(Error::NotInLibrary {
                        id: (*id).to_owned(),
                    });
                }
            }
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            put_back(&parked);
            return Err(error);
        }
        Ok(())
    }

    /// What deleting everything `filter` matches would take away: each artifact with the
    /// profiles whose current version installs it. Nothing is changed.
    pub fn removal_plan(&self, filter: &Filter) -> Result<RemovalPlan> {
        let (clause, params) = query::filter_clause(filter)?;
        let sql = format!(
            "SELECT a.id, a.kind, a.name FROM artifacts a JOIN sources s ON s.id = a.source_id \
             WHERE {clause} ORDER BY a.name COLLATE NOCASE, a.id"
        );
        let rows = self
            .store
            .conn
            .prepare(&sql)?
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut membership = self.profile_membership()?;
        let mut artifacts = Vec::with_capacity(rows.len());
        for (id, kind, name) in rows {
            let kind: crate::artifact::ArtifactKind =
                serde_json::from_value(serde_json::Value::String(kind))?;
            let profiles = membership.remove(&id).unwrap_or_default();
            artifacts.push(RemovalItem {
                id,
                kind,
                name,
                profiles,
            });
        }
        Ok(RemovalPlan { artifacts })
    }
}

/// One artifact a bulk delete would remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RemovalItem {
    pub id: String,
    pub kind: crate::artifact::ArtifactKind,
    pub name: String,
    /// Profiles whose current version installs it (they get a new version without it).
    pub profiles: Vec<crate::profiles::ProfileUse>,
}

/// Everything a filter matches, across all pages, before it is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RemovalPlan {
    pub artifacts: Vec<RemovalItem>,
}

impl Library {
    /// Every tag with the number of artifacts carrying it, by name.
    pub fn tags(&self) -> Result<Vec<TagWithCount>> {
        tags::list(&self.store.conn)
    }

    /// Creates a tag. The name is trimmed, 1 to 40 characters, unique ignoring case.
    pub fn create_tag(&self, name: &str, color: TagColor) -> Result<Tag> {
        tags::create(&self.store.conn, name, color)
    }

    /// Renames a tag (same rules as [`Library::create_tag`]).
    pub fn rename_tag(&self, id: i64, name: &str) -> Result<Tag> {
        tags::rename(&self.store.conn, id, name)
    }

    /// Changes a tag's color.
    pub fn recolor_tag(&self, id: i64, color: TagColor) -> Result<Tag> {
        tags::recolor(&self.store.conn, id, color)
    }

    /// Deletes a tag and removes it from every artifact.
    pub fn delete_tag(&self, id: i64) -> Result<()> {
        tags::delete(&self.store.conn, id)
    }

    /// Puts a tag on an artifact; doing it twice is fine.
    pub fn tag_artifact(&self, artifact_id: &str, tag_id: i64) -> Result<()> {
        tags::tag_artifact(&self.store.conn, artifact_id, tag_id)
    }

    /// Puts the tag named `name` on every artifact `filter` matches (all pages), in one
    /// transaction. A missing tag is created with `color` (a stable default when `None`); an
    /// existing one keeps its own. Artifacts that already carry it are left alone. Returns the
    /// tag and how many artifacts newly got it.
    pub fn tag_filtered(
        &mut self,
        filter: &Filter,
        name: &str,
        color: Option<TagColor>,
    ) -> Result<(Tag, usize)> {
        let name = tags::clean_name(name)?;
        let (clause, params) = query::filter_clause(filter)?;
        let tx = self.store.conn.transaction()?;
        let tag = tags::find_or_create(&tx, &name, color)?;
        let sql = format!(
            "INSERT OR IGNORE INTO artifact_tags (artifact_id, tag_id) \
             SELECT a.id, ?1 FROM artifacts a JOIN sources s ON s.id = a.source_id WHERE {clause}"
        );
        let mut all: Vec<rusqlite::types::Value> = vec![tag.id.into()];
        all.extend(params);
        let added = tx.execute(&sql, rusqlite::params_from_iter(all.iter()))?;
        tx.commit()?;
        Ok((tag, added))
    }

    /// Takes a tag off an artifact; a tag it does not carry is fine.
    pub fn untag_artifact(&self, artifact_id: &str, tag_id: i64) -> Result<()> {
        tags::untag_artifact(&self.store.conn, artifact_id, tag_id)
    }

    /// The tags on one artifact, by name.
    pub fn artifact_tags(&self, artifact_id: &str) -> Result<Vec<Tag>> {
        tags::of_artifact(&self.store.conn, artifact_id)
    }
}

/// A git subfolder as a relative path of plain components: no `..`, no absolute path.
pub(crate) fn git_subfolder(subfolder: Option<&str>) -> Result<PathBuf> {
    let text = subfolder.unwrap_or_default().trim().trim_matches('/');
    let path = PathBuf::from(text);
    if path.components().all(|c| matches!(c, Component::Normal(_))) {
        Ok(path)
    } else {
        Err(Error::UnsafePath {
            path: text.to_owned(),
        })
    }
}

impl Library {
    /// The id of the artifact of `kind` named `name` (the newest when several sources hold
    /// one), for opening what a reference points at.
    pub fn find(
        &self,
        kind: crate::artifact::ArtifactKind,
        name: &str,
    ) -> Result<Option<ArtifactId>> {
        let kind = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let found = self.store.conn.query_row(
            "SELECT id FROM artifacts WHERE kind = ?1 AND name = ?2
             ORDER BY imported_at DESC, id LIMIT 1",
            rusqlite::params![kind, name],
            |row| row.get::<_, String>(0),
        );
        match found {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl Library {
    /// One page of the library: filtered, sorted, with the total and the facet counts (see
    /// `query` for the semantics).
    pub fn list(&self, query: &Query) -> Result<Page> {
        let mut page = query::list(&self.store.conn, query)?;
        self.attach_badges(&mut page.items)?;
        Ok(page)
    }
}

impl Library {
    /// Every collection by name, with its member count (smart ones are evaluated now).
    pub fn collections(&self) -> Result<Vec<Collection>> {
        collections::list(&self.store.conn)
    }

    /// Creates an empty manual collection. The name is trimmed, 1 to 60 characters, unique
    /// ignoring case.
    pub fn create_manual_collection(&self, name: &str) -> Result<Collection> {
        collections::create_manual(&self.store.conn, name)
    }

    /// Creates a smart collection whose members are whatever matches `rule` when it is read.
    /// An empty rule is refused.
    pub fn create_smart_collection(&self, name: &str, rule: Filter) -> Result<Collection> {
        collections::create_smart(&self.store.conn, name, rule)
    }

    /// Renames a collection (same rules as on creation).
    pub fn rename_collection(&self, id: i64, name: &str) -> Result<Collection> {
        collections::rename(&self.store.conn, id, name)
    }

    /// Replaces a smart collection's rule; a manual collection has none.
    pub fn update_rule(&self, id: i64, rule: Filter) -> Result<Collection> {
        collections::update_rule(&self.store.conn, id, rule)
    }

    /// Deletes a collection; its artifacts stay in the library.
    pub fn delete_collection(&self, id: i64) -> Result<()> {
        collections::delete(&self.store.conn, id)
    }

    /// Appends an artifact to a manual collection; one already there stays where it is.
    pub fn add_to_collection(&self, collection_id: i64, artifact_id: &str) -> Result<()> {
        collections::add(&self.store.conn, collection_id, artifact_id)
    }

    /// Takes an artifact out of a manual collection; one not in it is fine.
    pub fn remove_from_collection(&self, collection_id: i64, artifact_id: &str) -> Result<()> {
        collections::remove(&self.store.conn, collection_id, artifact_id)
    }

    /// Sets a manual collection's order. `ordered_ids` must hold each current member once.
    pub fn reorder_collection(&self, collection_id: i64, ordered_ids: &[String]) -> Result<()> {
        collections::reorder(&self.store.conn, collection_id, ordered_ids)
    }

    /// The ids of the manual collections an artifact is in.
    pub fn artifact_collections(&self, artifact_id: &str) -> Result<Vec<i64>> {
        collections::containing(&self.store.conn, artifact_id)
    }

    /// One page of a collection's members, through the same filters, sort, pages and facets as
    /// [`Library::list`]. A smart collection's rule is ANDed with the query's own filter. For a
    /// manual collection `SortKey::Position` keeps its manual order (ascending is first to
    /// last); for a smart one `Position` sorts by name.
    pub fn collection_page(&self, collection_id: i64, query: &Query) -> Result<Page> {
        let mut page = collections::page(&self.store.conn, collection_id, query)?;
        self.attach_badges(&mut page.items)?;
        Ok(page)
    }
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Regular files below `dir`, relative to it with forward slashes, sorted.
pub(crate) fn stored_files(dir: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| io_error(dir, e))? {
            let entry = entry.map_err(|e| io_error(dir, e))?;
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            let kind = entry.file_type().map_err(|e| io_error(dir, e))?;
            if kind.is_dir() {
                walk(&entry.path(), &format!("{name}/"), out)?;
            } else if kind.is_file() {
                out.push(name);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort();
    Ok(files)
}

/// Whether two spellings of a plugin folder name the same one (`./a/`, `a`, `.` for the root).
fn same_folder(a: &str, b: &str) -> bool {
    let normal = |p: &str| {
        let p = p.trim().trim_end_matches('/');
        let p = p.strip_prefix("./").unwrap_or(p);
        if p == "." {
            String::new()
        } else {
            p.to_owned()
        }
    };
    normal(a) == normal(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_reports_the_current_schema_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let library = Library::open(dir.path()).expect("open");
        assert_eq!(
            library.schema_version().expect("version"),
            store::SCHEMA_VERSION
        );
    }
}
