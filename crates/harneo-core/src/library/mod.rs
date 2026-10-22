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

mod copy;
mod git;
#[cfg(test)]
mod git_tests;
pub mod ids;
#[cfg(test)]
mod import_tests;
mod records;
#[cfg(test)]
mod secrets_tests;
mod source;
mod store;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::{AgentAdapter, Artifact, ClaudeAdapter, Diagnostic, ScanOptions};
use crate::error::{Error, Result};
use copy::{copy_artifact, ImportHooks, NoHooks};
pub use copy::{SkipReason, SkippedFile};
use git::{Checkout, GitConfig};
pub use records::{ArtifactDetail, Provenance, SourceRecord};
pub use source::Source;
use store::Store;

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
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ImportReport {
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
    store: Store,
    git: GitConfig,
    /// The clone of the last git preview, kept until `import` copies from it. At most one: a
    /// newer git preview replaces (and deletes) it, `import` consumes it, and dropping the
    /// library deletes it. See [`PendingClone`].
    pending: Mutex<Option<PendingClone>>,
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
        Ok(Self {
            store: Store::open(data_dir)?,
            git: GitConfig::default(),
            pending: Mutex::new(None),
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
        match source {
            Source::Local { path } => {
                let identity = source.identity()?;
                self.scan_candidates(source, &identity, path)
            }
            Source::Git {
                url,
                branch,
                subfolder,
            } => self.git_preview(url, branch.as_deref(), subfolder.as_deref()),
        }
    }

    fn git_preview(
        &self,
        url: &str,
        branch: Option<&str>,
        subfolder: Option<&str>,
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
        let preview = self.scan_candidates(&resolved, &identity, &root)?;
        if preview.candidates.is_empty() {
            return Err(Error::NothingFound {
                source_name: source::redact(url),
            });
        }
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(PendingClone { identity, checkout });
        Ok(preview)
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

    fn scan_candidates(&self, source: &Source, identity: &str, root: &Path) -> Result<Preview> {
        let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
        let ids: Vec<String> = report
            .artifacts
            .iter()
            .map(|a| ids::artifact_id(identity, &a.path))
            .collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let known = records::existing(&self.store.conn, &id_refs)?;
        let candidates = report
            .artifacts
            .into_iter()
            .zip(ids)
            .map(|(artifact, id)| Candidate {
                already_in_library: known.contains(&id),
                replaced_secret_keys: copy::replaced_secret_keys(&artifact),
                id,
                artifact,
            })
            .collect();
        Ok(Preview {
            source: source.clone(),
            candidates,
            diagnostics: report.diagnostics,
        })
    }

    /// Copies the selected candidates into the library. Each must be in `preview`; ones already
    /// in the library are skipped. Either everything lands or nothing does: a failed copy or
    /// commit removes the folders and rolls the index back.
    pub fn import(&mut self, preview: &Preview, selection: &[ArtifactId]) -> Result<ImportReport> {
        self.import_with(preview, selection, &NoHooks)
    }

    fn import_with(
        &mut self,
        preview: &Preview,
        selection: &[ArtifactId],
        hooks: &dyn ImportHooks,
    ) -> Result<ImportReport> {
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
        // A git preview's clone is consumed here and deleted when this function returns,
        // whatever the outcome.
        let clone = match &preview.source {
            Source::Local { .. } => None,
            Source::Git { .. } => Some(self.take_pending(&identity)?),
        };
        let (existing, fresh): (Vec<&Candidate>, Vec<&Candidate>) =
            chosen.into_iter().partition(|c| known.contains(&c.id));
        let mut report = ImportReport {
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
        let staging = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        let mut replaced: HashMap<String, Vec<String>> = HashMap::new();
        for candidate in &fresh {
            let dir = staging.path().join(&candidate.id);
            fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
            let copied = copy_artifact(root, &candidate.artifact, &candidate.id, &dir, hooks)?;
            report.skipped_files.extend(copied.skipped);
            replaced.insert(candidate.id.clone(), copied.replaced_secret_keys);
        }

        let artifacts_dir = self.store.artifacts_dir();
        let library_root = self.store.root().to_path_buf();
        let mut placed: Vec<PathBuf> = Vec::new();
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            records::insert_source(&tx, &source_id, &preview.source, commit)?;
            let now = records::now();
            for candidate in &fresh {
                let keys = replaced.get(&candidate.id).map_or(&[][..], Vec::as_slice);
                records::insert_artifact(
                    &tx,
                    &candidate.id,
                    &source_id,
                    &candidate.artifact,
                    now,
                    keys,
                )?;
            }
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

    /// Deletes an artifact's copy and its index rows (tags and collection entries go with them).
    pub fn remove(&mut self, id: &str) -> Result<()> {
        let target = self.store.artifacts_dir().join(id);
        let trash = tempfile::Builder::new()
            .prefix(".remove-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        let parked = trash.path().join(id);
        // Park the folder first: a rename is atomic, so a failure below can put it back.
        let moved = match fs::rename(&target, &parked) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(io_error(&target, e)),
        };
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            if !records::delete(&tx, id)? {
                return Err(Error::NotInLibrary { id: id.to_owned() });
            }
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            if moved {
                let _ = fs::rename(&parked, &target);
            }
            return Err(error);
        }
        Ok(())
    }
}

/// A git subfolder as a relative path of plain components: no `..`, no absolute path.
fn git_subfolder(subfolder: Option<&str>) -> Result<PathBuf> {
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

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Regular files below `dir`, relative to it with forward slashes, sorted.
fn stored_files(dir: &Path) -> Result<Vec<String>> {
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
