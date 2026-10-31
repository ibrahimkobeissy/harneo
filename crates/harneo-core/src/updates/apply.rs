//! Updating a library copy from its source, in three steps so the library's lock is not held
//! while a source is cloned or copied: [`Library::prepare_update`] reads (under the lock),
//! [`UpdateJob::run`] copies the upstream files into a staging folder and audits them (no lock,
//! slow), [`Library::finish_update`] backs up an edited copy, swaps the folder and writes the
//! records (under the lock, short). [`Library::update`] chains them.
//!
//! The swap is all or nothing: the staged copy replaces the old folder only together with the
//! database transaction, and any failure puts the old folder back.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::diff::{diff_dirs, FileDiff};
use super::remote::{self, Failure, Session};
use super::{copy_digest, record_copy_digest, IMPORTED_COMMIT};
use crate::artifact::{Artifact, ScanOptions};
use crate::audit::{self, Audit};
use crate::error::{Error, Result};
use crate::library::git::GitConfig;
use crate::library::layout_claude::{StoredClaude, StoredLayout};
use crate::library::records::{self, SourceRecord};
use crate::library::{copy_artifact, copy_limit, ImportHooks, Library, NoHooks};

/// What the user chose for an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum UpdateChoice {
    /// Replace the library copy with the upstream one (an edited copy is backed up first).
    TakeUpstream,
    /// Keep the library copy: no flag until upstream changes again.
    KeepMine,
}

/// What happened to one artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum UpdateOutcome {
    /// The library copy now matches upstream.
    Updated,
    /// The user kept their copy.
    Kept,
    /// Left alone by *Update all* because it has local edits.
    SkippedLocalEdits,
    /// The update failed; the old copy is untouched (`message` says why).
    Failed,
}

/// The result of one update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UpdateReport {
    pub id: String,
    pub outcome: UpdateOutcome,
    /// Where the edited copy was saved before it was replaced.
    pub backup: Option<String>,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    /// Why it failed.
    pub message: Option<String>,
}

impl UpdateReport {
    fn new(id: &str, outcome: UpdateOutcome, versions: (Option<String>, Option<String>)) -> Self {
        Self {
            id: id.to_owned(),
            outcome,
            backup: None,
            from_version: versions.0,
            to_version: versions.1,
            message: None,
        }
    }
}

/// What an update reads from the library. Owns everything, so it can leave the lock.
#[derive(Debug)]
pub struct UpdateJob {
    git: GitConfig,
    session: Arc<Session>,
    library_root: PathBuf,
    current: PathBuf,
    id: String,
    source: SourceRecord,
    source_path: String,
    /// The source's import opted in to large files: the update copies them too.
    large_files: bool,
    /// The artifact's own entry when Claude placed it, replayed to find it again.
    layout: StoredLayout,
    versions: (Option<String>, Option<String>),
    /// *Update all*: leave the copy alone if it turns out to be edited.
    protect_edits: bool,
    /// What the user was shown, to be unchanged when the update is applied.
    seen: Seen,
}

/// The state the update was prepared against: the `upstream` row and the library copy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    state: String,
    upstream_digest: Option<String>,
    copy_digest: Option<String>,
}

/// The upstream copy, staged and audited, waiting to be swapped in.
#[derive(Debug)]
pub struct PreparedUpdate {
    id: String,
    artifact: Artifact,
    staging: tempfile::TempDir,
    current: PathBuf,
    audit: Audit,
    copy_digest: String,
    replaced_secret_keys: Vec<String>,
    versions: (Option<String>, Option<String>),
    protect_edits: bool,
    seen: Seen,
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// An artifact flagged removed or layout-changed has nothing to update from.
fn refuse_gone(state: &str) -> Result<()> {
    let message = match state {
        "removed" => "it is removed at its source",
        "layout_changed" => "its place in the source changed; ask Claude again",
        _ => return Ok(()),
    };
    Err(Error::Update {
        message: message.to_owned(),
    })
}

fn unavailable(failure: Failure) -> Error {
    Error::Update {
        message: match failure {
            Failure::Missing => "the source folder is gone".to_owned(),
            Failure::Unreachable(reason) => reason,
        },
    }
}

impl UpdateJob {
    /// Copies the artifact from its source into a staging folder (no lock needed): per-type
    /// copy with secrets replaced, from the clone the check kept (or a new one). Audits what
    /// was copied.
    pub fn run(&self) -> Result<PreparedUpdate> {
        self.run_with(&NoHooks)
    }

    pub(crate) fn run_with(&self, hooks: &dyn ImportHooks) -> Result<PreparedUpdate> {
        let located = remote::locate(&self.git, &self.session, &self.library_root, &self.source)
            .map_err(unavailable)?;
        let scanned =
            remote::scan(&located.root, &self.source, &self.layout).map_err(unavailable)?;
        let mut artifact = scanned
            .artifacts
            .into_iter()
            .find(|a| a.path == self.source_path)
            .ok_or_else(|| Error::Update {
                message: "it is gone from its source".to_owned(),
            })?;
        let staging = tempfile::Builder::new()
            .prefix(".update-")
            .tempdir_in(&self.library_root)
            .map_err(|e| io_error(&self.library_root, e))?;
        let staged = staging.path().join(&self.id);
        fs::create_dir(&staged).map_err(|e| io_error(&staged, e))?;
        let copied = copy_artifact(
            &located.root,
            &artifact,
            &self.id,
            &staged,
            hooks,
            copy_limit(self.large_files),
        )?;
        let mut audit = audit::audit(&artifact, &located.root, &ScanOptions::default());
        audit
            .unaudited
            .retain(|f| !copied.skipped.iter().any(|s| &s.file == f));
        if let Some(commit) = &located.commit {
            artifact.metadata.insert(
                IMPORTED_COMMIT.to_owned(),
                serde_json::Value::String(commit.clone()),
            );
        }
        Ok(PreparedUpdate {
            id: self.id.clone(),
            artifact,
            copy_digest: copy_digest(&staged)?,
            staging,
            current: self.current.clone(),
            audit,
            replaced_secret_keys: copied.replaced_secret_keys,
            versions: self.versions.clone(),
            protect_edits: self.protect_edits,
            seen: self.seen.clone(),
        })
    }
}

impl PreparedUpdate {
    /// What the update would change: the staged upstream copy against the library copy.
    pub fn diff(&self) -> Result<Vec<FileDiff>> {
        diff_dirs(&self.current, &self.staging.path().join(&self.id))
    }
}

/// The artifacts *Update all* will update (as jobs), and the ones it leaves out.
#[derive(Debug)]
pub struct UpdateAllPlan {
    jobs: Vec<UpdateJob>,
    skipped: Vec<UpdateReport>,
}

impl UpdateAllPlan {
    /// Runs every job (no lock) and hands each prepared copy to `finish`, which takes the lock
    /// and applies it. One failing never stops the others. Returns the updated ones, the
    /// failed ones, then the ones skipped for local edits.
    pub fn run(
        self,
        mut finish: impl FnMut(PreparedUpdate) -> Result<UpdateReport>,
    ) -> Vec<UpdateReport> {
        let mut out = Vec::new();
        let mut skipped = self.skipped;
        for job in self.jobs {
            let report = match job.run().and_then(&mut finish) {
                Ok(report) => report,
                Err(error) => {
                    let mut report =
                        UpdateReport::new(&job.id, UpdateOutcome::Failed, job.versions);
                    report.message = Some(error.to_string());
                    report
                }
            };
            if report.outcome == UpdateOutcome::SkippedLocalEdits {
                skipped.push(report);
            } else {
                out.push(report);
            }
        }
        out.extend(skipped);
        out
    }
}

impl Library {
    /// Step one: reads what updating `id` needs. The artifact must be flagged outdated (or
    /// skipped, when the user changed their mind).
    pub fn prepare_update(&self, id: &str) -> Result<UpdateJob> {
        self.job_for(id, false)
    }

    fn job_for(&self, id: &str, protect_edits: bool) -> Result<UpdateJob> {
        let detail = records::load(&self.store.conn, id)?
            .ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })?;
        let (state, from, to) = self.flagged(id)?;
        refuse_gone(&state)?;
        let current = self.store.artifacts_dir().join(id);
        let seen = Seen {
            state,
            upstream_digest: self.upstream_digest(id)?,
            copy_digest: if current.is_dir() {
                Some(copy_digest(&current)?)
            } else {
                None
            },
        };
        let layout = StoredLayout {
            claude: StoredClaude::of(&detail.artifact, &detail.provenance.source_path)
                .into_iter()
                .collect(),
            ignore: Vec::new(),
        };
        let large_files: bool = self.store.conn.query_row(
            "SELECT large_files FROM sources WHERE id = ?1",
            rusqlite::params![detail.provenance.source.id],
            |r| r.get(0),
        )?;
        Ok(UpdateJob {
            git: self.git.clone(),
            session: Arc::clone(&self.session),
            library_root: self.store.root().to_path_buf(),
            current: self.store.artifacts_dir().join(id),
            id: id.to_owned(),
            source: detail.provenance.source,
            source_path: detail.provenance.source_path,
            large_files,
            layout,
            versions: (from, to),
            protect_edits,
            seen,
        })
    }

    fn upstream_digest(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .store
            .conn
            .query_row(
                "SELECT upstream_digest FROM upstream WHERE artifact_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Refuses an update whose preparation is stale: the flag changed (a *Keep mine* or a newer
    /// check), the library copy changed (a promote), or the staged upstream is not the content
    /// the check saw.
    fn check_still_current(&self, prepared: &PreparedUpdate) -> Result<()> {
        let id = prepared.id.as_str();
        let stale = |what: &str| Error::Update {
            message: format!("{what} since it was prepared; check again"),
        };
        let (state, ..) = self.flagged(id).map_err(|_| stale("its flag changed"))?;
        let upstream_digest = self.upstream_digest(id)?;
        if state != prepared.seen.state || upstream_digest != prepared.seen.upstream_digest {
            return Err(stale("its flag changed"));
        }
        let folder = self.store.artifacts_dir().join(id);
        let copy = if folder.is_dir() {
            Some(copy_digest(&folder)?)
        } else {
            None
        };
        if copy != prepared.seen.copy_digest {
            return Err(stale("the library copy changed"));
        }
        if prepared.artifact.digest != upstream_digest {
            return Err(stale("its source changed"));
        }
        Ok(())
    }

    /// The row of `upstream` for `id`: its state and versions; an error when there is none.
    fn flagged(&self, id: &str) -> Result<(String, Option<String>, Option<String>)> {
        self.store
            .conn
            .query_row(
                "SELECT state, from_version, to_version FROM upstream WHERE artifact_id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| Error::Update {
                message: "nothing to update: it matches its source".to_owned(),
            })
    }

    /// Updates `id` as the user chose. *Take upstream* backs up an edited copy first, then
    /// swaps in the upstream copy; *Keep mine* silences the flag until upstream changes again.
    pub fn update(&mut self, id: &str, choice: UpdateChoice) -> Result<UpdateReport> {
        self.update_with(id, choice, &NoHooks)
    }

    pub(crate) fn update_with(
        &mut self,
        id: &str,
        choice: UpdateChoice,
        hooks: &dyn ImportHooks,
    ) -> Result<UpdateReport> {
        match choice {
            UpdateChoice::KeepMine => self.keep_mine(id),
            UpdateChoice::TakeUpstream => {
                let prepared = self.prepare_update(id)?.run_with(hooks)?;
                self.finish_with(prepared, hooks)
            }
        }
    }

    /// *Keep mine*: the flag stays off until the upstream version changes again.
    fn keep_mine(&self, id: &str) -> Result<UpdateReport> {
        let (state, from, to) = self.flagged(id)?;
        refuse_gone(&state)?;
        self.store.conn.execute(
            "UPDATE upstream SET state = 'skipped' WHERE artifact_id = ?1",
            params![id],
        )?;
        Ok(UpdateReport::new(id, UpdateOutcome::Kept, (from, to)))
    }

    /// Step three: backs up the copy when it was edited, swaps the staged copy in and writes
    /// the records (provenance digest, copy digest, `updated_at`, audit, reference links).
    pub fn finish_update(&mut self, prepared: PreparedUpdate) -> Result<UpdateReport> {
        self.finish_with(prepared, &NoHooks)
    }

    fn finish_with(
        &mut self,
        prepared: PreparedUpdate,
        hooks: &dyn ImportHooks,
    ) -> Result<UpdateReport> {
        let id = prepared.id.clone();
        self.check_still_current(&prepared)?;
        let edited = self.has_local_edits(&id)?;
        if edited && prepared.protect_edits {
            return Ok(UpdateReport::new(
                &id,
                UpdateOutcome::SkippedLocalEdits,
                prepared.versions,
            ));
        }
        let backup = if edited {
            Some(backup_copy(self.store.root(), &id, &prepared.current)?)
        } else {
            None
        };
        self.swap_in(&prepared, hooks)?;
        let mut report = UpdateReport::new(&id, UpdateOutcome::Updated, prepared.versions);
        report.backup = backup.map(|p| p.display().to_string());
        Ok(report)
    }

    fn swap_in(&mut self, prepared: &PreparedUpdate, hooks: &dyn ImportHooks) -> Result<()> {
        let id = prepared.id.as_str();
        let target = self.store.artifacts_dir().join(id);
        let staged = prepared.staging.path().join(id);
        let library_root = self.store.root().to_path_buf();
        // Not swept on open: if a crash leaves the old copy here, it is still the user's data.
        let parked_in = tempfile::Builder::new()
            .prefix(".swap-")
            .tempdir_in(&library_root)
            .map_err(|e| io_error(&library_root, e))?;
        let parked = parked_in.path().join(id);
        let mut moved_old = false;
        let mut placed_new = false;
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            let artifact = &prepared.artifact;
            let changed = tx.execute(
                "UPDATE artifacts SET name = ?2, description = ?3, digest = ?4, artifact = ?5,
                        replaced_secret_keys = ?6, updated_at = ?7 WHERE id = ?1",
                params![
                    id,
                    artifact.name,
                    artifact.description,
                    artifact.digest,
                    serde_json::to_string(artifact)?,
                    serde_json::to_string(&prepared.replaced_secret_keys)?,
                    now
                ],
            )?;
            if changed == 0 {
                return Err(Error::NotInLibrary { id: id.to_owned() });
            }
            records::update_audit(&tx, id, &prepared.audit, now)?;
            record_copy_digest(&tx, id, &prepared.copy_digest)?;
            tx.execute("DELETE FROM upstream WHERE artifact_id = ?1", params![id])?;
            // What the new version references replaces what the old one did; links the user
            // made or rejected stay.
            tx.execute(
                "DELETE FROM links WHERE from_id = ?1 AND origin = 'reference'
                   AND status = 'confirmed'",
                params![id],
            )?;
            crate::bundles::add_reference_links(&tx, &[id], now)?;
            match fs::rename(&target, &parked) {
                Ok(()) => moved_old = true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(&target, e)),
            }
            fs::rename(&staged, &target).map_err(|e| io_error(&target, e))?;
            placed_new = true;
            hooks
                .before_commit()
                .map_err(|e| io_error(&library_root, e))?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            // Best effort: the error being reported matters more than a failed restore.
            if placed_new {
                let _ = fs::remove_dir_all(&target);
            }
            if moved_old {
                let _ = fs::rename(&parked, &target);
            }
            return Err(error);
        }
        Ok(())
    }

    /// *Update all*: the outdated artifacts without local edits as jobs, the edited ones as
    /// skipped reports (they ask Yours or Upstream one by one).
    pub fn plan_update_all(&self) -> Result<UpdateAllPlan> {
        let ids: Vec<String> = {
            let mut stmt = self.store.conn.prepare(
                "SELECT artifact_id FROM upstream WHERE state = 'outdated' ORDER BY artifact_id",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let (mut jobs, mut skipped) = (Vec::new(), Vec::new());
        for id in ids {
            if self.has_local_edits(&id)? {
                let (_, from, to) = self.flagged(&id)?;
                skipped.push(UpdateReport::new(
                    &id,
                    UpdateOutcome::SkippedLocalEdits,
                    (from, to),
                ));
            } else {
                jobs.push(self.job_for(&id, true)?);
            }
        }
        Ok(UpdateAllPlan { jobs, skipped })
    }

    /// Updates every outdated artifact without local edits and returns what happened to each,
    /// the skipped ones included. Holds whatever lock the caller has; the app uses
    /// [`Library::plan_update_all`] and applies each copy separately.
    pub fn update_all(&mut self) -> Result<Vec<UpdateReport>> {
        let plan = self.plan_update_all()?;
        Ok(plan.run(|prepared| self.finish_update(prepared)))
    }
}

/// Copies the library copy to `<library>/.backups/<id>/<UTC timestamp>/` and returns that
/// folder.
pub(crate) fn backup_copy(library_root: &Path, id: &str, from: &Path) -> Result<PathBuf> {
    let parent = library_root.join(".backups").join(id);
    fs::create_dir_all(&parent).map_err(|e| io_error(&parent, e))?;
    let stamp = utc_stamp(records::now());
    let mut target = parent.join(&stamp);
    let mut n = 1;
    while target.exists() {
        n += 1;
        target = parent.join(format!("{stamp}-{n}"));
    }
    copy_tree(from, &target)?;
    Ok(target)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(|e| io_error(to, e))?;
    for entry in fs::read_dir(from).map_err(|e| io_error(from, e))? {
        let entry = entry.map_err(|e| io_error(from, e))?;
        let kind = entry.file_type().map_err(|e| io_error(from, e))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| io_error(&target, e))?;
        }
    }
    Ok(())
}

/// `20261023T141500Z` for Unix seconds.
fn utc_stamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod stamp_tests {
    use super::utc_stamp;

    #[test]
    fn formats_unix_seconds_as_utc() {
        assert_eq!(utc_stamp(0), "19700101T000000Z");
        assert_eq!(utc_stamp(1_792_765_800), "20261023T143000Z");
        assert_eq!(utc_stamp(951_782_400 + 86_399), "20000229T235959Z");
    }
}
