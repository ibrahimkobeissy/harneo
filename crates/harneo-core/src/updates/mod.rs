//! Updates: telling the user which library artifacts changed at their source.
//!
//! An artifact is outdated only when its own files changed upstream: the scan digest of the
//! artifact at the same path in the source differs from the digest recorded at import. A repo
//! commit that touched other artifacts is not an update. A git source is asked for its commit
//! first (`ls-remote`) and only cloned and rescanned when that commit moved; a local source is
//! rescanned in place. Results are stored in the `upstream` table, so the library list can show
//! them without a network.
//!
//! The library copy has its own digest (`copy_digest`, over the stored files), recorded at
//! import. A copy whose files no longer match it has *local edits*. Rows imported before the
//! digest existed get it on their first check and count as unedited.

mod apply;
#[cfg(test)]
mod apply_tests;
mod check;
pub(crate) mod diff;
#[cfg(test)]
mod diff_tests;
pub(crate) mod remote;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::artifact::Artifact;
use crate::error::{Error, Result};
use crate::library::records::{self, SourceRecord};
use crate::library::{Library, LibraryItem};
pub(crate) use apply::backup_copy;
pub use apply::{UpdateAllPlan, UpdateChoice, UpdateJob, UpdateOutcome, UpdateReport};
pub use check::{CheckJob, CheckOutcome};
pub(crate) use diff::hunks;
pub use diff::{DiffLine, DiffStatus, FileDiff, Hunk, LineKind};

/// The metadata key that records the commit an artifact was last updated from (git sources).
/// Without it the commit is the source's, from import.
pub(crate) const IMPORTED_COMMIT: &str = "importedCommit";

/// What to check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum CheckScope {
    /// Every source.
    All,
    /// One source, by its library id.
    Source { id: String },
    /// One artifact (its source is looked at, only this artifact is judged).
    Artifact { id: String },
}

/// What looking at one source found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SourceResult {
    /// Nothing differs from the library.
    Current,
    /// Some artifacts differ.
    Changed {
        /// Artifacts of the library whose own files changed upstream.
        outdated: u32,
        /// Artifacts of the library that are gone from the source.
        removed: u32,
        /// Artifacts in the source that are not in the library.
        #[serde(rename = "newUpstream")]
        new_upstream: u32,
    },
    /// Offline, no access, unknown branch, git missing or too slow. Others were still checked.
    Unreachable { reason: String },
    /// The local folder (or the repository's subfolder) is gone.
    Missing,
}

/// One source's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SourceCheck {
    /// The library's id for the source.
    pub id: String,
    pub result: SourceResult,
}

/// The outcome of [`Library::check_updates`], one entry per source looked at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct CheckReport {
    pub sources: Vec<SourceCheck>,
}

/// How an artifact differs from its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum UpstreamState {
    /// Its own files changed upstream.
    Outdated,
    /// Gone from the source (never deleted automatically).
    Removed,
    /// The user kept their copy for the current upstream version: no flag until it changes again.
    Skipped,
}

impl UpstreamState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Outdated => "outdated",
            Self::Removed => "removed",
            Self::Skipped => "skipped",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "removed" => Self::Removed,
            "skipped" => Self::Skipped,
            _ => Self::Outdated,
        }
    }
}

/// The update state of one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: UpstreamState,
    /// The version in the library: the artifact's `version` field, else the short commit it was
    /// imported at; `None` shows as "changed".
    pub from_version: Option<String>,
    /// The same for the upstream version.
    pub to_version: Option<String>,
    /// The library copy differs from what was stored at import.
    pub local_edits: bool,
}

/// What a card shows for an outdated artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UpdateBadge {
    pub to_version: Option<String>,
    pub local_edits: bool,
}

/// A stored artifact as the check sees it.
#[derive(Debug)]
struct Stored {
    id: String,
    source_path: String,
    digest: Option<String>,
    copy_digest: Option<String>,
    artifact: Artifact,
}

/// One row of `upstream`.
struct Row {
    state: UpstreamState,
    upstream_digest: Option<String>,
}

/// What differs for one artifact, before it is stored.
enum Verdict {
    Current,
    Outdated(Box<Artifact>),
    Removed,
}

/// An artifact is outdated only when its own files changed upstream: the repo moving is not
/// enough. A copy with no recorded digest cannot be judged and counts as current.
fn classify(stored: &Stored, upstream: Option<&Artifact>) -> Verdict {
    match upstream {
        None => Verdict::Removed,
        Some(a) => match (stored.digest.as_deref(), a.digest.as_deref()) {
            (Some(was), Some(now)) if was != now => Verdict::Outdated(Box::new(a.clone())),
            _ => Verdict::Current,
        },
    }
}

/// SHA-256 over the regular files below `dir` (path, a separator, length, content, in sorted
/// path order). Symlinks and other entries are ignored.
pub(crate) fn copy_digest(dir: &Path) -> Result<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, std::path::PathBuf)>) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| io_error(dir, e))? {
            let entry = entry.map_err(|e| io_error(dir, e))?;
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            let kind = entry.file_type().map_err(|e| io_error(dir, e))?;
            if kind.is_dir() {
                walk(&entry.path(), &format!("{name}/"), out)?;
            } else if kind.is_file() {
                out.push((name, entry.path()));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for (name, path) in files {
        let mut file = File::open(&path).map_err(|e| io_error(&path, e))?;
        let length = file.metadata().map_err(|e| io_error(&path, e))?.len();
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(length.to_le_bytes());
        let mut buffer = [0u8; 16 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|e| io_error(&path, e))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        }))
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Stores the digest of an artifact's stored files (at import, and after each update).
pub(crate) fn record_copy_digest(conn: &Connection, id: &str, digest: &str) -> Result<()> {
    conn.execute(
        "UPDATE artifacts SET copy_digest = ?2 WHERE id = ?1",
        params![id, digest],
    )?;
    Ok(())
}

fn version_of(artifact: &Artifact) -> Option<String> {
    artifact
        .metadata
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// The version shown for each side: the artifact's own `version` when one side has one and they
/// differ, else the short commits (git), else none ("changed").
fn versions(
    stored: &Artifact,
    upstream: Option<&Artifact>,
    from_commit: Option<&str>,
    to_commit: Option<&str>,
) -> (Option<String>, Option<String>) {
    let from = version_of(stored);
    let to = upstream.and_then(version_of);
    if (from.is_some() || to.is_some()) && from != to {
        return (from, to);
    }
    match (from_commit, to_commit) {
        (Some(a), Some(b)) if a != b => (Some(short(a)), Some(short(b))),
        _ => (None, None),
    }
}

impl Library {
    /// Records the copy digest of rows imported before it existed, from what is stored now.
    fn backfill_copy_digests(&self, stored: &[Stored]) -> Result<()> {
        for item in stored.iter().filter(|s| s.copy_digest.is_none()) {
            let folder = self.store.artifacts_dir().join(&item.id);
            if folder.is_dir() {
                record_copy_digest(&self.store.conn, &item.id, &copy_digest(&folder)?)?;
            }
        }
        Ok(())
    }

    /// The update state of an artifact, or `None` when it matches its source (or was never
    /// checked).
    pub fn update_status(&self, id: &str) -> Result<Option<UpdateStatus>> {
        let conn = &self.store.conn;
        let known: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM artifacts WHERE id = ?1)",
            params![id],
            |r| r.get(0),
        )?;
        if !known {
            return Err(Error::NotInLibrary { id: id.to_owned() });
        }
        let row = conn
            .query_row(
                "SELECT state, from_version, to_version FROM upstream WHERE artifact_id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((state, from_version, to_version)) = row else {
            return Ok(None);
        };
        Ok(Some(UpdateStatus {
            state: UpstreamState::parse(&state),
            from_version,
            to_version,
            local_edits: self.has_local_edits(id)?,
        }))
    }

    /// The stored copy no longer matches its recorded digest. A copy with no recorded digest
    /// (not checked yet) counts as unedited.
    pub(crate) fn has_local_edits(&self, id: &str) -> Result<bool> {
        let recorded: Option<String> = self
            .store
            .conn
            .query_row(
                "SELECT copy_digest FROM artifacts WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let Some(recorded) = recorded else {
            return Ok(false);
        };
        let folder = self.store.artifacts_dir().join(id);
        Ok(folder.is_dir() && copy_digest(&folder)? != recorded)
    }

    /// Fills `update` on listed items: a badge for each outdated artifact.
    pub(crate) fn attach_badges(&self, items: &mut [LibraryItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut stmt = self.store.conn.prepare_cached(
            "SELECT to_version FROM upstream WHERE artifact_id = ?1 AND state = 'outdated'",
        )?;
        let mut badges: HashMap<String, Option<String>> = HashMap::new();
        for item in items.iter() {
            let found = stmt
                .query_row(params![item.id], |r| r.get::<_, Option<String>>(0))
                .optional()?;
            if let Some(to_version) = found {
                badges.insert(item.id.clone(), to_version);
            }
        }
        drop(stmt);
        for item in items {
            if let Some(to_version) = badges.remove(&item.id) {
                item.update = Some(UpdateBadge {
                    to_version,
                    local_edits: self.has_local_edits(&item.id)?,
                });
            }
        }
        Ok(())
    }
}

fn source_rows(
    conn: &Connection,
    only: Option<&str>,
) -> Result<Vec<(SourceRecord, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, location, branch, subfolder, commit_sha, remote_commit FROM sources
         WHERE (?1 IS NULL OR id = ?1) ORDER BY id",
    )?;
    let rows = stmt.query_map(params![only], |r| {
        Ok((
            SourceRecord {
                id: r.get(0)?,
                kind: r.get(1)?,
                location: r.get(2)?,
                branch: r.get(3)?,
                subfolder: r.get(4)?,
                commit_sha: r.get(5)?,
            },
            r.get::<_, Option<String>>(6)?,
        ))
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn stored_of_source(conn: &Connection, source_id: &str) -> Result<Vec<Stored>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_path, digest, copy_digest, artifact FROM artifacts
         WHERE source_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![source_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, source_path, digest, copy_digest, json) = row?;
        out.push(Stored {
            id,
            source_path,
            digest,
            copy_digest,
            artifact: serde_json::from_str(&json)?,
        });
    }
    Ok(out)
}

fn existing_row(conn: &Connection, id: &str) -> Result<Option<Row>> {
    Ok(conn
        .query_row(
            "SELECT state, upstream_digest FROM upstream WHERE artifact_id = ?1",
            params![id],
            |r| {
                Ok(Row {
                    state: UpstreamState::parse(&r.get::<_, String>(0)?),
                    upstream_digest: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// What gets written to an `upstream` row.
struct NewRow<'a> {
    id: &'a str,
    state: UpstreamState,
    upstream_digest: Option<String>,
    upstream_commit: &'a Option<String>,
    from_version: Option<String>,
    to_version: Option<String>,
    checked_at: i64,
}

fn write_row(conn: &Connection, row: &NewRow<'_>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO upstream
            (artifact_id, state, upstream_digest, upstream_commit, from_version, to_version,
             checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.state.as_str(),
            row.upstream_digest,
            row.upstream_commit,
            row.from_version,
            row.to_version,
            row.checked_at
        ],
    )?;
    Ok(())
}

/// The source's result from what the table holds for its artifacts.
fn result_from_table(
    conn: &Connection,
    source_id: &str,
    new_upstream: u32,
) -> Result<SourceResult> {
    let count = |state: &str| -> Result<u32> {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM upstream u JOIN artifacts a ON a.id = u.artifact_id
             WHERE a.source_id = ?1 AND u.state = ?2",
            params![source_id, state],
            |r| r.get(0),
        )?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    };
    let (outdated, removed) = (count("outdated")?, count("removed")?);
    if outdated + removed + new_upstream == 0 {
        return Ok(SourceResult::Current);
    }
    Ok(SourceResult::Changed {
        outdated,
        removed,
        new_upstream,
    })
}
