//! Resolving a profile: what it installs once bundles are pulled in.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ProfileSpec;
use crate::artifact::ArtifactKind;
use crate::bundles::bundle_of;
use crate::error::Result;
use crate::library::{ArtifactId, Library};

/// Why an artifact is in the resolved profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Why {
    /// The user picked it.
    Picked,
    /// It is in the bundle of a picked artifact (the one named, however many links away).
    PulledBy { artifact: ArtifactId },
    /// It is the Settings template.
    Settings,
    /// It is one of the MCP servers.
    Mcp,
}

/// One artifact a profile installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ResolvedItem {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    pub why: Why,
    /// A skill installed with `disable-model-invocation`: the profile's choice, else the
    /// library default. Always false for other kinds.
    pub manual_only: bool,
    /// Files that travel with the artifact.
    pub file_count: u32,
}

/// What a spec installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Resolved {
    /// Picked items, each followed by the bundle members it pulls in, then the Settings
    /// template, then the MCP servers.
    pub artifacts: Vec<ResolvedItem>,
    /// Ids the spec names that are no longer in the library.
    pub missing: Vec<ArtifactId>,
    /// The lowest trust score among the audited artifacts; `None` when none is audited.
    pub worst_trust: Option<u8>,
}

/// The index columns resolving needs.
pub(super) struct Meta {
    pub kind: ArtifactKind,
    name: String,
    trust: Option<u8>,
    manual_only: bool,
    file_count: u32,
}

pub(super) fn meta(conn: &Connection, id: &str) -> Result<Option<Meta>> {
    let found = conn
        .query_row(
            "SELECT kind, name, trust_score, manual_only,
                    COALESCE(json_array_length(json_extract(artifact, '$.files')), 0)
             FROM artifacts WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    found
        .map(|(kind, name, trust, manual_only, files)| {
            Ok(Meta {
                kind: serde_json::from_value(serde_json::Value::String(kind))?,
                name,
                trust: trust.map(|t| u8::try_from(t.clamp(0, 100)).unwrap_or(100)),
                manual_only,
                file_count: u32::try_from(files).unwrap_or(0),
            })
        })
        .transpose()
}

pub(super) fn resolve(conn: &Connection, spec: &ProfileSpec) -> Result<Resolved> {
    let mut out = Resolved {
        artifacts: Vec::new(),
        missing: Vec::new(),
        worst_trust: None,
    };
    let mut seen: HashSet<String> = HashSet::new();
    let named: HashSet<&str> = spec
        .items
        .iter()
        .map(String::as_str)
        .chain(spec.settings.as_deref())
        .chain(spec.mcp.iter().map(|m| m.artifact.as_str()))
        .collect();
    let mut add = |out: &mut Resolved, id: &str, why: Why| -> Result<bool> {
        if !seen.insert(id.to_owned()) {
            return Ok(false);
        }
        let Some(found) = meta(conn, id)? else {
            out.missing.push(id.to_owned());
            return Ok(false);
        };
        let manual_only = found.kind == ArtifactKind::Skill
            && spec
                .manual_only
                .get(id)
                .copied()
                .unwrap_or(found.manual_only);
        if let Some(trust) = found.trust {
            out.worst_trust = Some(out.worst_trust.map_or(trust, |w| w.min(trust)));
        }
        out.artifacts.push(ResolvedItem {
            id: id.to_owned(),
            kind: found.kind,
            name: found.name,
            why,
            manual_only,
            file_count: found.file_count,
        });
        Ok(true)
    };
    for id in &spec.items {
        if add(&mut out, id, Why::Picked)? {
            for member in bundle_of(conn, id)? {
                if !named.contains(member.as_str()) {
                    add(
                        &mut out,
                        &member,
                        Why::PulledBy {
                            artifact: id.clone(),
                        },
                    )?;
                }
            }
        }
    }
    if let Some(id) = &spec.settings {
        add(&mut out, id, Why::Settings)?;
    }
    for choice in &spec.mcp {
        add(&mut out, &choice.artifact, Why::Mcp)?;
    }
    Ok(out)
}

impl Library {
    /// What `spec` installs: the picked artifacts with their bundles pulled in (and why), the
    /// Settings template and MCP servers, the ids no longer in the library, and the worst trust.
    pub fn resolve(&self, spec: &ProfileSpec) -> Result<Resolved> {
        resolve(&self.store.conn, spec)
    }
}
