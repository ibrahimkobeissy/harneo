//! The SQL for sources and artifact rows, and the record types the UI reads back.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::source::Source;
use crate::artifact::Artifact;
use crate::audit::{Audit, Reach, Tier};
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
    /// The security audit stored for the artifact; `None` until it is audited (artifacts
    /// imported before the audit existed show this until the next re-audit).
    pub audit: Option<Audit>,
    /// Unix seconds of that audit.
    pub audited_at: Option<i64>,
    /// What the artifact needs and what needs it (confirmed links).
    pub links: crate::bundles::Links,
    /// A skill installed with `disable-model-invocation` by default: the library flag, or the
    /// skill's own frontmatter.
    pub manual_only: bool,
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

/// Records the source row; `commit` is the cloned commit of a git source.
pub(crate) fn insert_source(
    conn: &Connection,
    id: &str,
    source: &Source,
    commit: Option<&str>,
) -> Result<()> {
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
        Source::Git {
            url,
            branch,
            subfolder,
        } => {
            conn.execute(
                "INSERT OR IGNORE INTO sources (id, kind, location, branch, subfolder, commit_sha)
                 VALUES (?1, 'git', ?2, ?3, ?4, ?5)",
                params![
                    id,
                    super::source::redact(url).trim_end_matches('/'),
                    branch,
                    subfolder,
                    commit
                ],
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
    replaced_secret_keys: &[String],
    audit: &Audit,
) -> Result<()> {
    conn.execute(
        "INSERT INTO artifacts
            (id, source_id, kind, name, description, source_path, digest, imported_at, artifact,
             replaced_secret_keys, trust_score, tier, findings, reach, audited_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            serde_json::to_string(replaced_secret_keys)?,
            audit.score,
            tier_name(audit.tier)?,
            serde_json::to_string(&audit.findings)?,
            serde_json::to_string(&audit.reach)?,
            imported_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn tier_name(tier: Tier) -> Result<String> {
    Ok(serde_json::to_value(tier)?
        .as_str()
        .unwrap_or_default()
        .to_owned())
}

/// Replaces the stored audit of `id`; false when there is no such row.
pub(crate) fn update_audit(conn: &Connection, id: &str, audit: &Audit, at: i64) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE artifacts SET trust_score = ?2, tier = ?3, findings = ?4, reach = ?5,
                audited_at = ?6 WHERE id = ?1",
        params![
            id,
            audit.score,
            tier_name(audit.tier)?,
            serde_json::to_string(&audit.findings)?,
            serde_json::to_string(&audit.reach)?,
            at,
        ],
    )?;
    Ok(changed > 0)
}

/// The stored audit of `id`, or `None` when the row is missing or was never audited.
pub(crate) fn load_audit(conn: &Connection, id: &str) -> Result<Option<(Audit, i64)>> {
    type Row = (
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT trust_score, tier, findings, reach, audited_at FROM artifacts WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((Some(score), Some(tier), Some(findings), Some(reach), Some(at))) = row else {
        return Ok(None);
    };
    let reach: Reach = serde_json::from_str(&reach)?;
    Ok(Some((
        Audit {
            score: u8::try_from(score).unwrap_or(0),
            tier: serde_json::from_value(serde_json::Value::String(tier))?,
            findings: serde_json::from_str(&findings)?,
            reach,
        },
        at,
    )))
}

/// The record without its stored file list, or `None` when the id is unknown.
pub(crate) fn load(conn: &Connection, id: &str) -> Result<Option<ArtifactDetail>> {
    let row = conn
        .query_row(
            "SELECT a.artifact, a.source_path, a.digest, a.imported_at, a.replaced_secret_keys,
                    s.id, s.kind, s.location, s.branch, s.subfolder, s.commit_sha,
                    a.manual_only
             FROM artifacts a JOIN sources s ON s.id = a.source_id WHERE a.id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, bool>(11)?,
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
    let Some((json, keys, flag, mut provenance)) = row else {
        return Ok(None);
    };
    provenance.replaced_secret_keys = serde_json::from_str(&keys)?;
    let stored = load_audit(conn, id)?;
    let artifact: Artifact = serde_json::from_str(&json)?;
    let own = match artifact.metadata.get("disable-model-invocation") {
        Some(serde_json::Value::Bool(on)) => *on,
        Some(serde_json::Value::String(text)) => text == "true",
        _ => false,
    };
    let manual_only = artifact.kind == crate::artifact::ArtifactKind::Skill && (flag || own);
    Ok(Some(ArtifactDetail {
        id: id.to_owned(),
        artifact,
        provenance,
        files: Vec::new(),
        audit: stored.as_ref().map(|(audit, _)| audit.clone()),
        audited_at: stored.map(|(_, at)| at),
        links: crate::bundles::links_of(conn, id)?,
        manual_only,
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
