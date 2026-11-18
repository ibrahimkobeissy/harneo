//! ADR-008: when a kept plugin part names a part of the same plugin that a profile does not hold
//! (`/build` needs `incremental-implementation`), the profile builder shows it and lets the user
//! keep the needed part or drop both. Nothing is decided here.

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{plugin_record, ArtifactId, Library};
use crate::artifact::ArtifactKind;
use crate::error::Result;
use crate::profiles::ProfileSpec;

/// A part a profile holds that names a part of its plugin the profile lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PartNeed {
    /// The part the profile holds.
    pub part: ArtifactId,
    pub part_name: String,
    pub part_kind: ArtifactKind,
    /// The plugin both belong to.
    pub plugin: String,
    /// The name of the part it names.
    pub needs: String,
    pub needs_kind: Option<ArtifactKind>,
    /// The library's copy of the needed part, when the library holds it (a click adds it).
    pub available: Option<ArtifactId>,
}

impl Library {
    /// The dangling names of `spec`: for each plugin part it installs, the parts of the same
    /// plugin it names that the profile does not install. In the profile's order.
    pub fn part_needs(&self, spec: &ProfileSpec) -> Result<Vec<PartNeed>> {
        let conn = &self.store.conn;
        let resolved = self.resolve(spec)?;
        let mut held: Vec<(ArtifactId, crate::artifact::Artifact, String)> = Vec::new();
        for item in &resolved.artifacts {
            let source: Option<String> = conn
                .query_row(
                    "SELECT source_id FROM artifacts WHERE id = ?1",
                    params![item.id],
                    |r| r.get(0),
                )
                .optional()?;
            if let (Some(source), Some(artifact)) =
                (source, crate::profiles::layout::load(conn, &item.id)?)
            {
                held.push((item.id.clone(), artifact, source));
            }
        }
        let present: HashSet<(String, String, String)> = held
            .iter()
            .filter_map(|(_, a, source)| {
                let (_, folder) = plugin_record::plugin_of(a)?;
                Some((source.clone(), folder, a.name.clone()))
            })
            .collect();
        let mut out = Vec::new();
        for (id, artifact, source) in &held {
            let Some((plugin, folder)) = plugin_record::plugin_of(artifact) else {
                continue;
            };
            for name in plugin_record::uses_of(artifact) {
                if present.contains(&(source.clone(), folder.clone(), name.clone())) {
                    continue;
                }
                let found: Option<(String, String)> = conn
                    .query_row(
                        "SELECT id, kind FROM artifacts
                         WHERE source_id = ?1 AND name = ?2
                           AND json_extract(artifact, '$.metadata.fromPlugin.folder') = ?3
                         ORDER BY id LIMIT 1",
                        params![source, name, folder],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                out.push(PartNeed {
                    part: id.clone(),
                    part_name: artifact.name.clone(),
                    part_kind: artifact.kind,
                    plugin: plugin.clone(),
                    needs: name,
                    needs_kind: found.as_ref().and_then(|(_, kind)| {
                        serde_json::from_value(serde_json::Value::String(kind.clone())).ok()
                    }),
                    available: found.map(|(id, _)| id),
                });
            }
        }
        Ok(out)
    }
}
