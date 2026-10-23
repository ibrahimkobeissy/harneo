//! Collections: named groups of library artifacts, kept only in the index.
//!
//! A *manual* collection is an ordered list of artifacts the user picked. A *smart* collection
//! stores a [`Filter`] as its rule and is evaluated whenever it is read, so it picks up
//! artifacts imported later and drops ones that stop matching. Members of either kind are
//! listed through the same machinery as [`Library::list`](super::Library::list).
//!
//! Removing an artifact from the library removes it from every manual collection (foreign key
//! cascade); deleting a collection leaves its artifacts alone.

use std::collections::HashSet;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::query::{self, filter_clause, Filter, Page, Query, Scope};
use crate::error::{Error, Result};

/// The longest collection name, in characters.
pub const MAX_NAME_CHARS: usize = 60;

/// How a collection decides its members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum CollectionKind {
    /// Hand-picked artifacts in a chosen order.
    Manual,
    /// Whatever matches the rule.
    Smart,
}

/// A collection and how many artifacts it holds right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub kind: CollectionKind,
    /// The smart rule; `None` for a manual collection.
    pub rule: Option<Filter>,
    pub count: u32,
}

fn invalid(reason: &str) -> Error {
    Error::InvalidCollection {
        reason: reason.to_owned(),
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("a collection needs a name"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(&format!(
            "a collection name is at most {MAX_NAME_CHARS} characters"
        )));
    }
    Ok(name.to_owned())
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == ErrorCode::ConstraintViolation)
}

fn is_empty_rule(rule: &Filter) -> bool {
    rule.types.is_empty()
        && rule.tags.is_empty()
        && rule.sources.is_empty()
        && rule.trust_min.is_none()
        && rule.tiers.is_empty()
        && !rule.has_findings
        && !rule.outdated
        && rule.statuses.is_empty()
        && rule.text.as_deref().is_none_or(|t| t.trim().is_empty())
}

fn check_rule(rule: &Filter) -> Result<()> {
    if is_empty_rule(rule) {
        return Err(invalid("a smart collection needs at least one rule"));
    }
    Ok(())
}

struct Row {
    id: i64,
    name: String,
    kind: CollectionKind,
    rule: Option<Filter>,
}

fn parse_kind(text: &str) -> CollectionKind {
    if text == "smart" {
        CollectionKind::Smart
    } else {
        CollectionKind::Manual
    }
}

fn row(conn: &Connection, id: i64) -> Result<Row> {
    let found = conn
        .query_row(
            "SELECT name, kind, rule FROM collections WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let (name, kind, rule) = found.ok_or(Error::CollectionNotFound { id })?;
    Ok(Row {
        id,
        name,
        kind: parse_kind(&kind),
        rule: rule.map(|text| serde_json::from_str(&text)).transpose()?,
    })
}

/// Members of a collection right now.
fn member_count(conn: &Connection, row: &Row) -> Result<u32> {
    let n: i64 = match &row.rule {
        Some(rule) => {
            let (clause, values) = filter_clause(rule)?;
            conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id
                     WHERE {clause}"
                ),
                params_from_iter(values.iter()),
                |r| r.get(0),
            )?
        }
        None => conn.query_row(
            "SELECT COUNT(*) FROM collection_items WHERE collection_id = ?1",
            params![row.id],
            |r| r.get(0),
        )?,
    };
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

fn with_count(conn: &Connection, row: Row) -> Result<Collection> {
    let count = member_count(conn, &row)?;
    Ok(Collection {
        id: row.id,
        name: row.name,
        kind: row.kind,
        rule: row.rule,
        count,
    })
}

pub(crate) fn list(conn: &Connection) -> Result<Vec<Collection>> {
    let ids = conn
        .prepare("SELECT id FROM collections ORDER BY name COLLATE NOCASE")?
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| with_count(conn, row(conn, id)?))
        .collect()
}

fn insert(conn: &Connection, name: &str, kind: &str, rule: Option<String>) -> Result<i64> {
    let name = clean_name(name)?;
    match conn.execute(
        "INSERT INTO collections (name, kind, rule) VALUES (?1, ?2, ?3)",
        params![name, kind, rule],
    ) {
        Ok(_) => Ok(conn.last_insert_rowid()),
        Err(e) if is_unique_violation(&e) => Err(Error::CollectionExists { name }),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn create_manual(conn: &Connection, name: &str) -> Result<Collection> {
    let id = insert(conn, name, "manual", None)?;
    with_count(conn, row(conn, id)?)
}

pub(crate) fn create_smart(conn: &Connection, name: &str, rule: Filter) -> Result<Collection> {
    clean_name(name)?;
    check_rule(&rule)?;
    let id = insert(conn, name, "smart", Some(serde_json::to_string(&rule)?))?;
    with_count(conn, row(conn, id)?)
}

pub(crate) fn rename(conn: &Connection, id: i64, name: &str) -> Result<Collection> {
    let name = clean_name(name)?;
    match conn.execute(
        "UPDATE collections SET name = ?1 WHERE id = ?2",
        params![name, id],
    ) {
        Ok(0) => Err(Error::CollectionNotFound { id }),
        Ok(_) => with_count(conn, row(conn, id)?),
        Err(e) if is_unique_violation(&e) => Err(Error::CollectionExists { name }),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn update_rule(conn: &Connection, id: i64, rule: Filter) -> Result<Collection> {
    let current = row(conn, id)?;
    if current.kind != CollectionKind::Smart {
        return Err(invalid("only a smart collection has a rule"));
    }
    check_rule(&rule)?;
    conn.execute(
        "UPDATE collections SET rule = ?1 WHERE id = ?2",
        params![serde_json::to_string(&rule)?, id],
    )?;
    with_count(conn, row(conn, id)?)
}

pub(crate) fn delete(conn: &Connection, id: i64) -> Result<()> {
    match conn.execute("DELETE FROM collections WHERE id = ?1", params![id])? {
        0 => Err(Error::CollectionNotFound { id }),
        _ => Ok(()),
    }
}

/// The collection, which must be manual.
fn manual(conn: &Connection, id: i64) -> Result<Row> {
    let found = row(conn, id)?;
    if found.kind != CollectionKind::Manual {
        return Err(invalid("a smart collection's members come from its rule"));
    }
    Ok(found)
}

pub(crate) fn add(conn: &Connection, collection_id: i64, artifact_id: &str) -> Result<()> {
    manual(conn, collection_id)?;
    let exists = conn
        .query_row(
            "SELECT 1 FROM artifacts WHERE id = ?1",
            params![artifact_id],
            |_| Ok(()),
        )
        .optional()?;
    if exists.is_none() {
        return Err(Error::NotInLibrary {
            id: artifact_id.to_owned(),
        });
    }
    conn.execute(
        "INSERT OR IGNORE INTO collection_items (collection_id, artifact_id, position)
         SELECT ?1, ?2, COALESCE(MAX(position) + 1, 0)
         FROM collection_items WHERE collection_id = ?1",
        params![collection_id, artifact_id],
    )?;
    Ok(())
}

pub(crate) fn remove(conn: &Connection, collection_id: i64, artifact_id: &str) -> Result<()> {
    manual(conn, collection_id)?;
    conn.execute(
        "DELETE FROM collection_items WHERE collection_id = ?1 AND artifact_id = ?2",
        params![collection_id, artifact_id],
    )?;
    Ok(())
}

pub(crate) fn reorder(conn: &Connection, collection_id: i64, ordered_ids: &[String]) -> Result<()> {
    manual(conn, collection_id)?;
    let current = conn
        .prepare("SELECT artifact_id FROM collection_items WHERE collection_id = ?1")?
        .query_map(params![collection_id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;
    let wanted: HashSet<&String> = ordered_ids.iter().collect();
    if wanted.len() != ordered_ids.len()
        || wanted.len() != current.len()
        || !wanted.iter().all(|id| current.contains(*id))
    {
        return Err(invalid(
            "the new order must list each artifact of the collection exactly once",
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for (position, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE collection_items SET position = ?1
             WHERE collection_id = ?2 AND artifact_id = ?3",
            params![
                i64::try_from(position).unwrap_or(i64::MAX),
                collection_id,
                id
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The manual collections that hold an artifact, by id.
pub(crate) fn containing(conn: &Connection, artifact_id: &str) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT collection_id FROM collection_items WHERE artifact_id = ?1 ORDER BY collection_id",
    )?;
    let ids = stmt
        .query_map(params![artifact_id], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

pub(crate) fn page(conn: &Connection, collection_id: i64, query: &Query) -> Result<Page> {
    let found = row(conn, collection_id)?;
    let scope = match &found.rule {
        Some(rule) => {
            let (clause, params) = filter_clause(rule)?;
            Scope {
                clause,
                params,
                manual_collection: None,
            }
        }
        None => Scope {
            clause: "a.id IN (SELECT artifact_id FROM collection_items WHERE collection_id = ?)"
                .to_owned(),
            params: vec![Value::Integer(collection_id)],
            manual_collection: Some(collection_id),
        },
    };
    query::list_in(conn, query, Some(&scope))
}
