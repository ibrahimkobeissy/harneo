//! Profiles: named, versioned selections of library artifacts.
//!
//! A profile stores what the user picked (a [`ProfileSpec`]); bundles are pulled in when it is
//! [resolved](Library::resolve), never copied in. Every save that changes something writes a new
//! immutable version, so a project can pin one. Renaming and describing a profile are not
//! versions.

mod file;
#[cfg(test)]
mod file_tests;
pub(crate) mod layout;
#[cfg(test)]
mod layout_tests;
mod resolve;
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod tests;
mod used_in;
#[cfg(test)]
mod used_in_tests;

use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::library::{records, ArtifactId, Filter, KindCount, Library, Query, SortKey, MAX_LIMIT};

pub use file::{
    ImportArtifact, ImportedProfile, ProfileImportPreview, ProfileImportReport, FORMAT,
    FORMAT_VERSION, MAX_FILE_BYTES,
};
pub use layout::{Generated, Layout, LayoutConflict, PathFrom, PlannedPath};
pub use resolve::{Resolved, ResolvedItem, Why};
pub use used_in::ProfileUse;

/// A profile id (exported to TypeScript as `bigint`, like tag and collection ids).
pub type ProfileId = i64;

/// The longest profile name, in characters.
pub const MAX_NAME_CHARS: usize = 60;

/// An MCP server the profile ships: the library artifact and its environment variables, whose
/// values are `${NAME}` placeholders only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct McpChoice {
    pub artifact: ArtifactId,
    pub env: BTreeMap<String, String>,
}

/// What a profile version holds: the JSON stored per version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileSpec {
    /// The artifacts the user picked (never Settings or MCP), each pulling its bundle.
    pub items: Vec<ArtifactId>,
    /// The Settings template, if any.
    pub settings: Option<ArtifactId>,
    pub mcp: Vec<McpChoice>,
    /// Per-profile override of a skill's manual-only default (true = manual only).
    pub manual_only: BTreeMap<ArtifactId, bool>,
}

/// A profile in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: ProfileId,
    pub name: String,
    pub description: String,
    /// The current version.
    pub version: u32,
    /// Artifacts per kind, bundles included.
    pub counts: Vec<KindCount>,
    /// Files those artifacts bring.
    pub file_count: u32,
    /// Projects the install registry has this profile in.
    pub project_count: u32,
    /// Unix seconds.
    pub updated_at: i64,
}

/// One saved version in a profile's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: u32,
    /// Unix seconds.
    pub saved_at: i64,
}

/// One version of a profile, with its history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileView {
    pub id: ProfileId,
    pub name: String,
    pub description: String,
    /// The version `spec` is from.
    pub version: u32,
    /// The newest version.
    pub current_version: u32,
    pub spec: ProfileSpec,
    /// Every version, oldest first.
    pub versions: Vec<VersionInfo>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn invalid(reason: impl Into<String>) -> Error {
    Error::InvalidProfile {
        reason: reason.into(),
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("a profile needs a name"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(format!(
            "a profile name is at most {MAX_NAME_CHARS} characters"
        )));
    }
    Ok(name.to_owned())
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == ErrorCode::ConstraintViolation)
}

/// True for `${NAME}` with `NAME` made of `A-Z`, `0-9` and `_`.
pub(crate) fn is_placeholder(value: &str) -> bool {
    value
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        })
}

impl ProfileSpec {
    /// Refuses a spec that cannot be saved: unknown artifacts, wrong kinds, duplicates, literal
    /// MCP env values, or nothing in it at all.
    fn validate(&self, conn: &Connection) -> Result<()> {
        let kind_of = |id: &str| -> Result<ArtifactKind> {
            resolve::meta(conn, id)?
                .map(|m| m.kind)
                .ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })
        };
        if self.items.is_empty() && self.settings.is_none() && self.mcp.is_empty() {
            return Err(invalid("a profile needs at least one artifact"));
        }
        let mut seen = HashSet::new();
        for id in &self.items {
            if !seen.insert(id) {
                return Err(invalid("an artifact is listed twice"));
            }
            match kind_of(id)? {
                ArtifactKind::Settings => {
                    return Err(invalid("a Settings template is chosen as the template"));
                }
                ArtifactKind::Mcp => return Err(invalid("an MCP server is added as a server")),
                _ => {}
            }
        }
        if let Some(id) = &self.settings {
            if kind_of(id)? != ArtifactKind::Settings {
                return Err(invalid("the Settings template must be a Settings artifact"));
            }
        }
        let mut servers = HashSet::new();
        for choice in &self.mcp {
            if !servers.insert(&choice.artifact) {
                return Err(invalid("an MCP server is listed twice"));
            }
            if kind_of(&choice.artifact)? != ArtifactKind::Mcp {
                return Err(invalid("an MCP server must be an MCP artifact"));
            }
            for (key, value) in &choice.env {
                if !is_placeholder(value) {
                    return Err(invalid(format!(
                        "{key} looked like a real secret, so it wasn't saved; Harneo stores \
                         placeholders like ${{NAME}} only"
                    )));
                }
            }
        }
        for id in self.manual_only.keys() {
            kind_of(id)?;
        }
        Ok(())
    }

    /// Every artifact id the spec names directly.
    fn mentions(&self, id: &str) -> bool {
        self.items.iter().any(|i| i == id)
            || self.settings.as_deref() == Some(id)
            || self.mcp.iter().any(|m| m.artifact == id)
            || self.manual_only.contains_key(id)
    }

    /// Takes `id` out of every place it can be.
    fn without(&self, id: &str) -> Self {
        Self {
            items: self.items.iter().filter(|i| *i != id).cloned().collect(),
            settings: self.settings.clone().filter(|s| s != id),
            mcp: self
                .mcp
                .iter()
                .filter(|m| m.artifact != id)
                .cloned()
                .collect(),
            manual_only: self
                .manual_only
                .iter()
                .filter(|(k, _)| *k != id)
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        }
    }
}

struct Row {
    name: String,
    description: String,
    current_version: u32,
    created_at: i64,
    updated_at: i64,
}

fn row(conn: &Connection, id: ProfileId) -> Result<Row> {
    conn.query_row(
        "SELECT name, description, current_version, created_at, updated_at
         FROM profiles WHERE id = ?1",
        params![id],
        |r| {
            Ok(Row {
                name: r.get(0)?,
                description: r.get(1)?,
                current_version: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or(Error::ProfileNotFound { id })
}

fn spec_of(conn: &Connection, id: ProfileId, version: u32) -> Result<ProfileSpec> {
    let text: Option<String> = conn
        .query_row(
            "SELECT spec FROM profile_versions WHERE profile_id = ?1 AND version = ?2",
            params![id, version],
            |r| r.get(0),
        )
        .optional()?;
    let text = text.ok_or(Error::ProfileNotFound { id })?;
    Ok(serde_json::from_str(&text)?)
}

fn insert_version(
    conn: &Connection,
    id: ProfileId,
    version: u32,
    spec: &ProfileSpec,
    at: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO profile_versions (profile_id, version, spec, saved_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, version, serde_json::to_string(spec)?, at],
    )?;
    conn.execute(
        "UPDATE profiles SET current_version = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, version, at],
    )?;
    Ok(())
}

/// Inserts a profile with its version 1; the spec is checked by the caller.
fn insert_profile(
    conn: &Connection,
    name: &str,
    description: &str,
    spec: &ProfileSpec,
) -> Result<ProfileId> {
    let name = clean_name(name)?;
    let at = records::now();
    let tx = conn.unchecked_transaction()?;
    match tx.execute(
        "INSERT INTO profiles (name, description, current_version, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?3)",
        params![name, description.trim(), at],
    ) {
        Ok(_) => {}
        Err(e) if is_unique_violation(&e) => return Err(Error::ProfileExists { name }),
        Err(e) => return Err(e.into()),
    }
    let id = tx.last_insert_rowid();
    insert_version(&tx, id, 1, spec, at)?;
    tx.commit()?;
    Ok(id)
}

/// Gives every profile whose current version holds any of `artifacts` one new version without
/// them. Runs inside the library's removal transaction.
pub(crate) fn remove_artifacts(conn: &Connection, artifacts: &[&str]) -> Result<()> {
    let ids = conn
        .prepare("SELECT id FROM profiles ORDER BY id")?
        .query_map([], |r| r.get::<_, ProfileId>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let at = records::now();
    for id in ids {
        let version = row(conn, id)?.current_version;
        let spec = spec_of(conn, id, version)?;
        if artifacts.iter().any(|a| spec.mentions(a)) {
            let pruned = artifacts.iter().fold(spec, |s, a| s.without(a));
            insert_version(conn, id, version + 1, &pruned, at)?;
        }
    }
    Ok(())
}

impl Library {
    /// Every profile by name, with what its current version resolves to.
    pub fn profiles(&self) -> Result<Vec<ProfileSummary>> {
        let conn = &self.store.conn;
        let ids = conn
            .prepare("SELECT id FROM profiles ORDER BY name COLLATE NOCASE")?
            .query_map([], |r| r.get::<_, ProfileId>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::new();
        for id in ids {
            let found = row(conn, id)?;
            let spec = spec_of(conn, id, found.current_version)?;
            let resolved = resolve::resolve(conn, &spec)?;
            let mut counts: BTreeMap<ArtifactKind, u32> = BTreeMap::new();
            for item in &resolved.artifacts {
                *counts.entry(item.kind).or_default() += 1;
            }
            out.push(ProfileSummary {
                id,
                name: found.name,
                description: found.description,
                version: found.current_version,
                counts: counts
                    .into_iter()
                    .map(|(kind, count)| KindCount { kind, count })
                    .collect(),
                file_count: resolved.artifacts.iter().map(|i| i.file_count).sum(),
                project_count: crate::install::project_count(conn, id)?,
                updated_at: found.updated_at,
            });
        }
        Ok(out)
    }

    /// A profile at `version` (the current one when `None`). An old version stays readable.
    pub fn profile(&self, id: ProfileId, version: Option<u32>) -> Result<ProfileView> {
        let conn = &self.store.conn;
        let found = row(conn, id)?;
        let shown = version.unwrap_or(found.current_version);
        let spec = spec_of(conn, id, shown)?;
        let versions = conn
            .prepare(
                "SELECT version, saved_at FROM profile_versions
                 WHERE profile_id = ?1 ORDER BY version",
            )?
            .query_map(params![id], |r| {
                Ok(VersionInfo {
                    version: r.get(0)?,
                    saved_at: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ProfileView {
            id,
            name: found.name,
            description: found.description,
            version: shown,
            current_version: found.current_version,
            spec,
            versions,
            created_at: found.created_at,
            updated_at: found.updated_at,
        })
    }

    /// Creates a profile at version 1. The spec is validated first.
    pub fn create_profile(
        &self,
        name: &str,
        description: &str,
        spec: &ProfileSpec,
    ) -> Result<ProfileId> {
        clean_name(name)?;
        spec.validate(&self.store.conn)?;
        insert_profile(&self.store.conn, name, description, spec)
    }

    /// Saves `spec` as the next version, or returns the current version when nothing changed.
    pub fn save_profile(&self, id: ProfileId, spec: &ProfileSpec) -> Result<u32> {
        let conn = &self.store.conn;
        spec.validate(conn)?;
        let tx = conn.unchecked_transaction()?;
        let version = row(&tx, id)?.current_version;
        if spec_of(&tx, id, version)? == *spec {
            return Ok(version);
        }
        insert_version(&tx, id, version + 1, spec, records::now())?;
        tx.commit()?;
        Ok(version + 1)
    }

    /// Renames a profile (same rules as on creation). Not a new version.
    pub fn rename_profile(&self, id: ProfileId, name: &str) -> Result<()> {
        let name = clean_name(name)?;
        match self.store.conn.execute(
            "UPDATE profiles SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, records::now()],
        ) {
            Ok(0) => Err(Error::ProfileNotFound { id }),
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => Err(Error::ProfileExists { name }),
            Err(e) => Err(e.into()),
        }
    }

    /// Changes a profile's description. Not a new version.
    pub fn set_profile_description(&self, id: ProfileId, description: &str) -> Result<()> {
        match self.store.conn.execute(
            "UPDATE profiles SET description = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, description.trim(), records::now()],
        )? {
            0 => Err(Error::ProfileNotFound { id }),
            _ => Ok(()),
        }
    }

    /// Deletes a profile and all its versions; refused while a project uses it.
    pub fn delete_profile(&self, id: ProfileId) -> Result<()> {
        let used = crate::install::project_names(&self.store.conn, id)?;
        if !used.is_empty() {
            return Err(Error::ProfileInUse {
                name: row(&self.store.conn, id)?.name,
                projects: used.join(", "),
            });
        }
        match self
            .store
            .conn
            .execute("DELETE FROM profiles WHERE id = ?1", params![id])?
        {
            0 => Err(Error::ProfileNotFound { id }),
            _ => Ok(()),
        }
    }

    /// A new profile with the current version's content, described "Cloned from X vN".
    pub fn clone_profile(&self, id: ProfileId, new_name: &str) -> Result<ProfileId> {
        let conn = &self.store.conn;
        clean_name(new_name)?;
        let found = row(conn, id)?;
        let spec = spec_of(conn, id, found.current_version)?;
        let description = format!("Cloned from {} v{}", found.name, found.current_version);
        insert_profile(conn, new_name, &description, &spec)
    }

    /// A new profile holding a collection's artifacts (a smart one is evaluated now). Settings
    /// and MCP artifacts are left out; bundles come with each item when the profile resolves.
    pub fn profile_from_collection(&self, collection_id: i64, name: &str) -> Result<ProfileId> {
        clean_name(name)?;
        let collection = self
            .collections()?
            .into_iter()
            .find(|c| c.id == collection_id)
            .ok_or(Error::CollectionNotFound { id: collection_id })?;
        let mut items = Vec::new();
        let mut query = Query {
            filter: Filter::default(),
            sort: SortKey::Position,
            descending: false,
            offset: 0,
            limit: MAX_LIMIT,
        };
        loop {
            let page = self.collection_page(collection_id, &query)?;
            let fetched = u32::try_from(page.items.len()).unwrap_or(u32::MAX);
            items.extend(
                page.items
                    .into_iter()
                    .filter(|i| !matches!(i.kind, ArtifactKind::Settings | ArtifactKind::Mcp))
                    .map(|i| i.id),
            );
            query.offset += fetched;
            if fetched == 0 || query.offset >= page.total {
                break;
            }
        }
        let spec = ProfileSpec {
            items,
            ..ProfileSpec::default()
        };
        spec.validate(&self.store.conn)?;
        let description = format!("Created from the collection {}", collection.name);
        insert_profile(&self.store.conn, name, &description, &spec)
    }

    /// Sets a skill's library default for manual-only invocation. Only the index changes, never
    /// the library file.
    pub fn set_manual_only(&self, artifact: &str, manual_only: bool) -> Result<()> {
        let Some(meta) = resolve::meta(&self.store.conn, artifact)? else {
            return Err(Error::NotInLibrary {
                id: artifact.to_owned(),
            });
        };
        if meta.kind != ArtifactKind::Skill {
            return Err(invalid("only a skill can be manual only"));
        }
        self.store.conn.execute(
            "UPDATE artifacts SET manual_only = ?2 WHERE id = ?1",
            params![artifact, manual_only],
        )?;
        Ok(())
    }
}
