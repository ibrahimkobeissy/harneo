//! Links between artifacts, bundles and the app settings store.
//!
//! A link is directed: `A needs B`. A's bundle is A plus everything reachable from it through
//! confirmed links; there is no bundle object to keep in sync. Rejected proposals stay in the
//! table with status `rejected` so a re-run never proposes them again.
//!
//! Links come from the structured references the scan resolves (origin `reference`, added at
//! import), from a Claude pass the user confirms (`ai`) and from the user (`manual`).

mod claude_cli;
#[cfg(test)]
mod claude_cli_tests;
mod detect;
#[cfg(test)]
mod detect_tests;
#[cfg(test)]
mod links_tests;

pub use claude_cli::{cli_status, ClaudeConfig, CliStatus, DEFAULT_MODEL};
pub(crate) use detect::store_accepted;
pub use detect::{
    AcceptedLink, Confidence, DetectProgress, Detection, DetectionJob, ProposedLink, TagSuggestion,
};

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::{Artifact, ArtifactKind};
use crate::error::{Error, Result};
use crate::library::{records, ArtifactId, Library};

/// Where a link came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum LinkOrigin {
    /// A structured reference the scan resolved.
    Reference,
    /// Proposed by Claude and confirmed by the user.
    Ai,
    /// Added by the user.
    Manual,
}

impl LinkOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Ai => "ai",
            Self::Manual => "manual",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "reference" => Self::Reference,
            "ai" => Self::Ai,
            _ => Self::Manual,
        }
    }
}

/// One end of a link as the UI lists it: the artifact on the other side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct LinkView {
    /// The artifact on the other end.
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    pub origin: LinkOrigin,
    /// Why the link exists (empty when nobody said).
    pub reason: String,
}

/// The confirmed links of one artifact, each list by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Links {
    /// What the artifact needs.
    pub needs: Vec<LinkView>,
    /// What needs the artifact.
    pub needed_by: Vec<LinkView>,
}

const MAX_KEY: usize = 120;

/// Shown as the reason of a reference link.
const REFERENCE_REASON: &str = "declared in its own fields";

fn kind_name(kind: ArtifactKind) -> Result<String> {
    Ok(serde_json::to_value(kind)?
        .as_str()
        .unwrap_or_default()
        .to_owned())
}

fn exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM artifacts WHERE id = ?1)",
        params![id],
        |r| r.get(0),
    )?)
}

fn require(conn: &Connection, id: &str) -> Result<()> {
    if exists(conn, id)? {
        Ok(())
    } else {
        Err(Error::NotInLibrary { id: id.to_owned() })
    }
}

fn view_rows(conn: &Connection, sql: &str, id: &str) -> Result<Vec<LinkView>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, kind, name, origin, reason) = row?;
        out.push(LinkView {
            id,
            kind: serde_json::from_value(serde_json::Value::String(kind))?,
            name,
            origin: LinkOrigin::parse(&origin),
            reason,
        });
    }
    Ok(out)
}

/// The confirmed links of `id`; an unknown id has none.
pub(crate) fn links_of(conn: &Connection, id: &str) -> Result<Links> {
    Ok(Links {
        needs: view_rows(
            conn,
            "SELECT a.id, a.kind, a.name, l.origin, l.reason
             FROM links l JOIN artifacts a ON a.id = l.to_id
             WHERE l.from_id = ?1 AND l.status = 'confirmed'
             ORDER BY a.name COLLATE NOCASE, a.id",
            id,
        )?,
        needed_by: view_rows(
            conn,
            "SELECT a.id, a.kind, a.name, l.origin, l.reason
             FROM links l JOIN artifacts a ON a.id = l.from_id
             WHERE l.to_id = ?1 AND l.status = 'confirmed'
             ORDER BY a.name COLLATE NOCASE, a.id",
            id,
        )?,
    })
}

/// Everything `id` needs, directly or not, by depth then name. `id` itself is not listed,
/// and a cycle never repeats a member.
pub(crate) fn bundle_of(conn: &Connection, id: &str) -> Result<Vec<ArtifactId>> {
    let mut stmt = conn.prepare_cached(
        "SELECT a.id FROM links l JOIN artifacts a ON a.id = l.to_id
         WHERE l.from_id = ?1 AND l.status = 'confirmed'
         ORDER BY a.name COLLATE NOCASE, a.id",
    )?;
    let mut seen: HashSet<String> = HashSet::from([id.to_owned()]);
    let mut out = Vec::new();
    let mut level = vec![id.to_owned()];
    while !level.is_empty() {
        let mut next: Vec<String> = Vec::new();
        for from in &level {
            let rows = stmt.query_map(params![from], |r| r.get::<_, String>(0))?;
            for row in rows {
                let to = row?;
                if seen.insert(to.clone()) {
                    next.push(to);
                }
            }
        }
        // Siblings from different parents must interleave by name, not by parent.
        next.sort_by_cached_key(|to| name_of(conn, to).unwrap_or_default().to_lowercase());
        out.extend(next.iter().cloned());
        level = next;
    }
    Ok(out)
}

fn name_of(conn: &Connection, id: &str) -> Result<String> {
    Ok(conn.query_row(
        "SELECT name FROM artifacts WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?)
}

/// The bundle size (members other than the artifact itself) of each of `ids`.
pub(crate) fn bundle_sizes(conn: &Connection, ids: &[&str]) -> Result<HashMap<String, u32>> {
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    let mut stmt =
        conn.prepare_cached("SELECT from_id, to_id FROM links WHERE status = 'confirmed'")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (from, to) = row?;
        edges.entry(from).or_default().push(to);
    }
    let mut sizes = HashMap::new();
    for id in ids {
        let mut seen: HashSet<&str> = HashSet::from([*id]);
        let mut stack: Vec<&str> = vec![id];
        while let Some(at) = stack.pop() {
            for to in edges.get(at).into_iter().flatten() {
                if seen.insert(to) {
                    stack.push(to);
                }
            }
        }
        sizes.insert(
            (*id).to_owned(),
            u32::try_from(seen.len() - 1).unwrap_or(u32::MAX),
        );
    }
    Ok(sizes)
}

fn check_pair(conn: &Connection, from: &str, to: &str) -> Result<()> {
    if from == to {
        return Err(Error::InvalidLink {
            message: "an artifact cannot be linked to itself".into(),
        });
    }
    require(conn, from)?;
    require(conn, to)
}

/// Stores a confirmed link `from needs to`. One that is already confirmed is left as it is; a
/// rejected one becomes confirmed with the new origin and reason.
pub(crate) fn add_link(
    conn: &Connection,
    from: &str,
    to: &str,
    origin: LinkOrigin,
    reason: &str,
) -> Result<()> {
    check_pair(conn, from, to)?;
    conn.execute(
        "INSERT INTO links (from_id, to_id, origin, reason, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'confirmed', ?5)
         ON CONFLICT (from_id, to_id) DO UPDATE
           SET origin = excluded.origin, reason = excluded.reason, status = 'confirmed',
               created_at = excluded.created_at
           WHERE links.status = 'rejected'",
        params![from, to, origin.as_str(), reason, records::now()],
    )?;
    Ok(())
}

/// Deletes the link; one that does not exist is fine.
pub(crate) fn remove_link(conn: &Connection, from: &str, to: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM links WHERE from_id = ?1 AND to_id = ?2",
        params![from, to],
    )?;
    Ok(())
}

/// Remembers that the user refused `from needs to`, whether or not it was ever stored.
pub(crate) fn reject_link(conn: &Connection, from: &str, to: &str) -> Result<()> {
    check_pair(conn, from, to)?;
    conn.execute(
        "INSERT INTO links (from_id, to_id, origin, reason, status, created_at)
         VALUES (?1, ?2, 'ai', '', 'rejected', ?3)
         ON CONFLICT (from_id, to_id) DO UPDATE SET status = 'rejected'",
        params![from, to, records::now()],
    )?;
    Ok(())
}

/// Adds origin-`reference` links for the artifacts just imported (`fresh`), in both directions:
/// what each one references that is in the library (or the same import), and the existing
/// artifacts whose references name it. Pairs the user rejected are not brought back. A plugin
/// is kept whole, so it never gets a link from its own contents, but it can be a target.
pub(crate) fn add_reference_links(conn: &Connection, fresh: &[&str], at: i64) -> Result<()> {
    let mut insert = conn.prepare_cached(
        "INSERT OR IGNORE INTO links (from_id, to_id, origin, reason, status, created_at)
         VALUES (?1, ?2, 'reference', ?3, 'confirmed', ?4)",
    )?;
    for id in fresh {
        let (kind, json): (String, String) = conn.query_row(
            "SELECT kind, artifact FROM artifacts WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if kind != "plugin" {
            let artifact: Artifact = serde_json::from_str(&json)?;
            for reference in &artifact.references {
                let target: Option<String> = conn
                    .query_row(
                        "SELECT id FROM artifacts WHERE kind = ?1 AND name = ?2 AND id <> ?3
                         ORDER BY imported_at DESC, id LIMIT 1",
                        params![kind_name(reference.kind)?, reference.target, id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(target) = target {
                    insert.execute(params![id, target, REFERENCE_REASON, at])?;
                }
            }
        }
        let artifact_kind = &kind;
        let name = name_of(conn, id)?;
        let mut dependents = conn.prepare_cached(
            "SELECT DISTINCT a.id FROM artifacts a, json_each(a.artifact, '$.references') r
             WHERE a.id <> ?3 AND a.kind <> 'plugin'
               AND json_extract(r.value, '$.kind') = ?1 AND json_extract(r.value, '$.target') = ?2",
        )?;
        let from: Vec<String> = dependents
            .query_map(params![artifact_kind, name, id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        for from in from {
            insert.execute(params![from, id, REFERENCE_REASON, at])?;
        }
    }
    Ok(())
}

fn check_key(key: &str) -> Result<()> {
    if key.trim().is_empty() || key.len() > MAX_KEY {
        return Err(Error::InvalidLink {
            message: format!("a setting key is 1 to {MAX_KEY} characters"),
        });
    }
    Ok(())
}

impl Library {
    /// The confirmed links of an artifact: what it needs and what needs it.
    pub fn links_of(&self, id: &str) -> Result<Links> {
        require(&self.store.conn, id)?;
        links_of(&self.store.conn, id)
    }

    /// The artifact's bundle: everything reachable from it through confirmed links, by depth
    /// then name, without the artifact itself. Safe on cycles.
    pub fn bundle_of(&self, id: &str) -> Result<Vec<ArtifactId>> {
        require(&self.store.conn, id)?;
        bundle_of(&self.store.conn, id)
    }

    /// Confirms `from needs to`. Self-links and unknown artifacts are refused.
    pub fn add_link(&self, from: &str, to: &str, origin: LinkOrigin, reason: &str) -> Result<()> {
        add_link(&self.store.conn, from, to, origin, reason.trim())
    }

    /// Deletes a link (confirmed or rejected); one that is not there is fine.
    pub fn remove_link(&self, from: &str, to: &str) -> Result<()> {
        remove_link(&self.store.conn, from, to)
    }

    /// Remembers a refused proposal so a re-run does not offer it again.
    pub fn reject_link(&self, from: &str, to: &str) -> Result<()> {
        reject_link(&self.store.conn, from, to)
    }

    /// The setting stored under `key` (keys are namespaced by module, `bundles.model`), if any.
    pub fn settings_get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        check_key(key)?;
        let text: Option<String> = self
            .store
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        text.map(|t| serde_json::from_str(&t).map_err(Error::from))
            .transpose()
    }

    /// Stores `value` under `key`, replacing what was there.
    pub fn settings_set(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        check_key(key)?;
        self.store.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
}
