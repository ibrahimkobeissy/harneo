//! Tags: names with a color from the `DESIGN.md` tag palette, kept only in the index.

use std::collections::HashMap;

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::artifact::Artifact;
use crate::error::{Error, Result};

/// The longest tag name, in characters.
pub const MAX_NAME_CHARS: usize = 40;
/// The most suggestions one artifact gets.
const MAX_SUGGESTIONS: usize = 10;

/// What an import applies: artifact id → tag names (missing tags are created).
pub type TagChoices = HashMap<String, Vec<String>>;

/// A tag color: exactly the tint names of the tag palette in `DESIGN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TagColor {
    Cyan,
    Sky,
    Indigo,
    Violet,
    Fuchsia,
    Stone,
    Slate,
    Red,
    Orange,
    Amber,
    Lime,
    Emerald,
    Teal,
    Pink,
}

impl TagColor {
    pub(crate) const ALL: [Self; 14] = [
        Self::Cyan,
        Self::Sky,
        Self::Indigo,
        Self::Violet,
        Self::Fuchsia,
        Self::Stone,
        Self::Slate,
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Lime,
        Self::Emerald,
        Self::Teal,
        Self::Pink,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Cyan => "cyan",
            Self::Sky => "sky",
            Self::Indigo => "indigo",
            Self::Violet => "violet",
            Self::Fuchsia => "fuchsia",
            Self::Stone => "stone",
            Self::Slate => "slate",
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Amber => "amber",
            Self::Lime => "lime",
            Self::Emerald => "emerald",
            Self::Teal => "teal",
            Self::Pink => "pink",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }

    /// A stable color for a name, so the same new tag always starts with the same color.
    pub(crate) fn default_for(name: &str) -> Self {
        let digest = Sha256::digest(name.to_lowercase().as_bytes());
        Self::ALL[usize::from(digest[0]) % Self::ALL.len()]
    }
}

/// A tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: TagColor,
}

/// A tag with the number of library artifacts that carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TagWithCount {
    pub id: i64,
    pub name: String,
    pub color: TagColor,
    pub count: u32,
}

/// Trims a tag name and checks it is usable.
pub(crate) fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::InvalidTag {
            reason: "a tag needs a name".to_owned(),
        });
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(Error::InvalidTag {
            reason: format!("a tag name is at most {MAX_NAME_CHARS} characters"),
        });
    }
    Ok(name.to_owned())
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == ErrorCode::ConstraintViolation)
}

fn color_of(text: &str) -> Result<TagColor> {
    TagColor::parse(text).ok_or_else(|| Error::InvalidTag {
        reason: format!("unknown tag color {text}"),
    })
}

pub(crate) fn list(conn: &Connection) -> Result<Vec<TagWithCount>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, COUNT(a.artifact_id)
         FROM tags t LEFT JOIN artifact_tags a ON a.tag_id = t.id
         GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?, r.get(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, color, count) = row?;
        out.push(TagWithCount {
            id,
            name,
            color: color_of(&color)?,
            count,
        });
    }
    Ok(out)
}

pub(crate) fn create(conn: &Connection, name: &str, color: TagColor) -> Result<Tag> {
    let name = clean_name(name)?;
    match conn.execute(
        "INSERT INTO tags (name, color) VALUES (?1, ?2)",
        params![name, color.as_str()],
    ) {
        Ok(_) => Ok(Tag {
            id: conn.last_insert_rowid(),
            name,
            color,
        }),
        Err(e) if is_unique_violation(&e) => Err(Error::TagExists { name }),
        Err(e) => Err(e.into()),
    }
}

/// The tag called `name` (already cleaned), or a new one: `color`, else a stable default.
pub(crate) fn find_or_create(
    conn: &Connection,
    name: &str,
    color: Option<TagColor>,
) -> Result<Tag> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
            r.get(0)
        })
        .optional()?;
    match existing {
        Some(id) => get(conn, id),
        None => create(
            conn,
            name,
            color.unwrap_or_else(|| TagColor::default_for(name)),
        ),
    }
}

pub(crate) fn rename(conn: &Connection, id: i64, name: &str) -> Result<Tag> {
    let name = clean_name(name)?;
    match conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", params![name, id]) {
        Ok(0) => Err(Error::TagNotFound { id }),
        Ok(_) => get(conn, id),
        Err(e) if is_unique_violation(&e) => Err(Error::TagExists { name }),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn recolor(conn: &Connection, id: i64, color: TagColor) -> Result<Tag> {
    match conn.execute(
        "UPDATE tags SET color = ?1 WHERE id = ?2",
        params![color.as_str(), id],
    )? {
        0 => Err(Error::TagNotFound { id }),
        _ => get(conn, id),
    }
}

pub(crate) fn delete(conn: &Connection, id: i64) -> Result<()> {
    match conn.execute("DELETE FROM tags WHERE id = ?1", params![id])? {
        0 => Err(Error::TagNotFound { id }),
        _ => Ok(()),
    }
}

fn get(conn: &Connection, id: i64) -> Result<Tag> {
    let row = conn
        .query_row(
            "SELECT name, color FROM tags WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    let (name, color) = row.ok_or(Error::TagNotFound { id })?;
    Ok(Tag {
        id,
        name,
        color: color_of(&color)?,
    })
}

pub(crate) fn tag_artifact(conn: &Connection, artifact_id: &str, tag_id: i64) -> Result<()> {
    let artifact = conn.query_row(
        "SELECT 1 FROM artifacts WHERE id = ?1",
        params![artifact_id],
        |_| Ok(()),
    );
    match artifact {
        Ok(()) => {}
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Err(Error::NotInLibrary {
                id: artifact_id.to_owned(),
            })
        }
        Err(e) => return Err(e.into()),
    }
    get(conn, tag_id)?;
    conn.execute(
        "INSERT OR IGNORE INTO artifact_tags (artifact_id, tag_id) VALUES (?1, ?2)",
        params![artifact_id, tag_id],
    )?;
    Ok(())
}

pub(crate) fn untag_artifact(conn: &Connection, artifact_id: &str, tag_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM artifact_tags WHERE artifact_id = ?1 AND tag_id = ?2",
        params![artifact_id, tag_id],
    )?;
    Ok(())
}

pub(crate) fn of_artifact(conn: &Connection, artifact_id: &str) -> Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tags t JOIN artifact_tags a ON a.tag_id = t.id
         WHERE a.artifact_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let ids = stmt
        .query_map(params![artifact_id], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| get(conn, id)).collect()
}

/// Applies tag names to an artifact, creating the missing tags with a default color. Meant to
/// run inside the import transaction.
pub(crate) fn apply_names(conn: &Connection, artifact_id: &str, names: &[String]) -> Result<()> {
    for name in names {
        let name = clean_name(name)?;
        let existing: Option<i64> = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
                r.get(0)
            })
            .optional()?;
        let tag_id = match existing {
            Some(id) => id,
            None => create(conn, &name, TagColor::default_for(&name))?.id,
        };
        tag_artifact(conn, artifact_id, tag_id)?;
    }
    Ok(())
}

/// Checks every chosen name up front, so a bad one fails before anything is copied.
pub(crate) fn validate_choices(choices: &TagChoices) -> Result<()> {
    choices
        .values()
        .flatten()
        .try_for_each(|name| clean_name(name).map(drop))
}

/// Tag suggestions from an artifact's frontmatter `keywords` or `tags`: a string (split on
/// commas) or a list of strings; trimmed, valid, deduplicated ignoring case, at most 10.
pub(crate) fn suggestions(artifact: &Artifact) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for key in ["keywords", "tags"] {
        let raw: Vec<&str> = match artifact.metadata.get(key) {
            Some(serde_json::Value::String(s)) => s.split(',').collect(),
            Some(serde_json::Value::Array(items)) => {
                items.iter().filter_map(|v| v.as_str()).collect()
            }
            _ => continue,
        };
        for item in raw {
            let Ok(name) = clean_name(item) else { continue };
            let lower = name.to_lowercase();
            if !out.iter().any(|o| o.to_lowercase() == lower) {
                out.push(name);
            }
        }
    }
    out.truncate(MAX_SUGGESTIONS);
    out
}
