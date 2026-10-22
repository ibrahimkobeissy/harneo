//! The SQL for sources and artifact rows, and the record types the UI reads back.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::source::Source;
use crate::artifact::Artifact;
use crate::error::Result;

/// Where an artifact came from, as recorded at import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SourceRecord {
    pub id: String,
    /// `local` or `git`.
    pub kind: String,
    /// The absolute path (local) or the URL (git).
    pub location: String,
    pub branch: Option<String>,
    pub subfolder: Option<String>,
    pub commit_sha: Option<String>,
}

/// Everything the library remembers about where an artifact came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Provenance {
    pub source: SourceRecord,
    /// The artifact's path in its source.
    pub source_path: String,
    /// The scan digest at import.
    pub digest: Option<String>,
    /// Unix seconds.
    pub imported_at: i64,
    /// Key names whose literal secrets were replaced by placeholders.
    pub replaced_secret_keys: Vec<String>,
}

/// One library artifact in full.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ArtifactDetail {
    pub id: String,
    /// The scan result as it was at import (paths relative to the source).
    pub artifact: Artifact,
    pub provenance: Provenance,
    /// The files stored in the library copy, relative to the copy, sorted.
    pub files: Vec<String>,
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}

/// Which of `ids` are already indexed.
pub(crate) fn existing(conn: &Connection, ids: &[&str]) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare_cached("SELECT 1 FROM artifacts WHERE id = ?1")?;
    let mut found = HashSet::new();
    for id in ids {
        if stmt.exists(params![id])? {
            found.insert((*id).to_owned());
        }
    }
    Ok(found)
}

pub(crate) fn insert_source(conn: &Connection, id: &str, source: &Source) -> Result<()> {
    match source {
        Source::Local { path } => {
            let canonical = path.canonicalize().map_err(|e| crate::Error::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            conn.execute(
                "INSERT OR IGNORE INTO sources (id, kind, location) VALUES (?1, 'local', ?2)",
                params![id, canonical.display().to_string()],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn insert_artifact(
    conn: &Connection,
    id: &str,
    source_id: &str,
    artifact: &Artifact,
    imported_at: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO artifacts
            (id, source_id, kind, name, description, source_path, digest, imported_at, artifact)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            source_id,
            serde_json::to_value(artifact.kind)?
                .as_str()
                .unwrap_or_default(),
            artifact.name,
            artifact.description,
            artifact.path,
            artifact.digest,
            imported_at,
            serde_json::to_string(artifact)?,
        ],
    )?;
    Ok(())
}

/// The record without its stored file list, or `None` when the id is unknown.
pub(crate) fn load(conn: &Connection, id: &str) -> Result<Option<ArtifactDetail>> {
    let row = conn
        .query_row(
            "SELECT a.artifact, a.source_path, a.digest, a.imported_at, a.replaced_secret_keys,
                    s.id, s.kind, s.location, s.branch, s.subfolder, s.commit_sha
             FROM artifacts a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(4)?,
                    Provenance {
                        source_path: r.get(1)?,
                        digest: r.get(2)?,
                        imported_at: r.get(3)?,
                        replaced_secret_keys: Vec::new(),
                        source: SourceRecord {
                            id: r.get(5)?,
                            kind: r.get(6)?,
                            location: r.get(7)?,
                            branch: r.get(8)?,
                            subfolder: r.get(9)?,
                            commit_sha: r.get(10)?,
                        },
                    },
                ))
            },
        )
        .optional()?;
    let Some((json, keys, mut provenance)) = row else {
        return Ok(None);
    };
    provenance.replaced_secret_keys = serde_json::from_str(&keys)?;
    Ok(Some(ArtifactDetail {
        id: id.to_owned(),
        artifact: serde_json::from_str(&json)?,
        provenance,
        files: Vec::new(),
    }))
}

/// Deletes the artifact row, and its source when no artifact is left on it.
pub(crate) fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let source_id: Option<String> = conn
        .query_row(
            "SELECT source_id FROM artifacts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(source_id) = source_id else {
        return Ok(false);
    };
    conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id])?;
    conn.execute(
        "DELETE FROM sources WHERE id = ?1
           AND NOT EXISTS (SELECT 1 FROM artifacts WHERE source_id = ?1)",
        params![source_id],
    )?;
    Ok(true)
}
