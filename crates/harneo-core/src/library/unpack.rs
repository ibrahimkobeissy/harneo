//! Unpacked plugins at import (`spec/library.md`, "Unpacking a plugin"): a plugin is in the
//! library whole or as some of its parts, never both, and the choice is kept on its source.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::Candidate;
use crate::artifact::{Artifact, ArtifactKind, Severity};
use crate::error::{Error, Result};

const NEEDS_PLUGIN: &str = "plugin.part-needs-plugin";

/// The plugin an unpacked part came from: its name and folder in the source.
fn part_of(artifact: &Artifact) -> Option<(String, String)> {
    let from = artifact.metadata.get("fromPlugin")?;
    let folder = from.get("folder").and_then(Value::as_str)?;
    let name = from.get("name").and_then(Value::as_str).unwrap_or(folder);
    Some((name.to_owned(), folder.to_owned()))
}

/// The folder of the plugin an unpacked part came from.
pub(super) fn part_folder(artifact: &Artifact) -> Option<String> {
    part_of(artifact).map(|(_, folder)| folder)
}

/// The plugin an artifact is, when it is a whole Plugin.
fn whole_of(artifact: &Artifact) -> Option<(String, String)> {
    (artifact.kind == ArtifactKind::Plugin).then(|| (artifact.name.clone(), artifact.path.clone()))
}

/// Who a plugin is, wherever it was imported from: its name and marketplace (a local clone and
/// the git URL of the same repo are the same plugin).
/// A plugin's name and marketplace.
type Identity = (String, String);

fn identity(artifact: &Artifact) -> Option<Identity> {
    let (name, market) = if artifact.kind == ArtifactKind::Plugin {
        (
            artifact.name.clone(),
            crate::plugin::marketplace_of(artifact),
        )
    } else {
        let from = artifact.metadata.get("fromPlugin")?;
        let name = part_of(artifact)?.0;
        (name, from.get("marketplace").and_then(Value::as_str))
    };
    Some((name, market.unwrap_or_default().to_owned()))
}

fn exclusive(plugin: &str, detail: String) -> Error {
    Error::PluginExclusive {
        plugin: plugin.to_owned(),
        detail,
    }
}

/// The plugins the library holds, by [`identity`], whole and unpacked, from every source.
fn held(conn: &Connection) -> Result<(HashSet<Identity>, HashSet<Identity>)> {
    let mut stmt = conn.prepare("SELECT artifact FROM artifacts")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let (mut whole, mut parts) = (HashSet::new(), HashSet::new());
    for row in rows {
        let Ok(artifact) = serde_json::from_str::<Artifact>(&row?) else {
            continue;
        };
        let Some(id) = identity(&artifact) else {
            continue;
        };
        if artifact.kind == ArtifactKind::Plugin {
            whole.insert(id);
        } else {
            parts.insert(id);
        }
    }
    Ok((whole, parts))
}

/// Refuses a selection that breaks the rules: a part that only works with its plugin, a plugin
/// and its parts picked together, parts while the plugin is in the library whole, or the plugin
/// whole while parts of it are. `chosen` is the whole selection; `fresh` the part of it not in
/// the library yet.
pub(super) fn check(conn: &Connection, chosen: &[&Candidate], fresh: &[&Candidate]) -> Result<()> {
    for c in chosen {
        let needs = c
            .artifact
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.code == NEEDS_PLUGIN);
        if needs {
            return Err(Error::PartNeedsPlugin {
                name: c.artifact.name.clone(),
            });
        }
    }
    for whole in chosen.iter().filter_map(|c| whole_of(&c.artifact)) {
        let has_parts = chosen
            .iter()
            .filter_map(|c| part_of(&c.artifact))
            .any(|(_, folder)| folder == whole.1);
        if has_parts {
            return Err(exclusive(
                &whole.0,
                "the whole plugin and its parts were both picked; pick one".into(),
            ));
        }
    }
    let (in_whole, in_parts) = held(conn)?;
    for c in fresh {
        let Some(id) = identity(&c.artifact) else {
            continue;
        };
        if let Some((name, _)) = part_of(&c.artifact) {
            if in_whole.contains(&id) {
                return Err(exclusive(
                    &name,
                    format!("the plugin is in the library whole; remove the plugin {name} first to import its parts"),
                ));
            }
        }
        if let Some((name, _)) = whole_of(&c.artifact) {
            if in_parts.contains(&id) {
                return Err(exclusive(
                    &name,
                    format!("parts of the plugin are in the library; remove them first to import the plugin {name} whole"),
                ));
            }
        }
    }
    Ok(())
}

/// The plugin folders the fresh candidates unpack, and those they import whole.
pub(super) fn folders(fresh: &[&Candidate]) -> (Vec<String>, Vec<String>) {
    let unpacked = fresh
        .iter()
        .filter_map(|c| part_of(&c.artifact).map(|(_, folder)| folder))
        .collect();
    let whole = fresh
        .iter()
        .filter_map(|c| whole_of(&c.artifact).map(|(_, folder)| folder))
        .collect();
    (unpacked, whole)
}

/// A plugin of a preview (whole or unpacked) and what the library already holds of it, wherever
/// it was imported from. The window shows this before import; [`check`] stays the backstop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PluginHold {
    pub name: String,
    /// The plugin's folder in the source: what `ScanOptions.unpack` names.
    pub folder: String,
    /// The library holds the plugin whole.
    pub whole: bool,
    /// The library holds parts of the plugin.
    pub parts: bool,
}

/// The plugins in `candidates`, each once, with what the library holds of them.
pub(super) fn holds(conn: &Connection, candidates: &[Candidate]) -> Result<Vec<PluginHold>> {
    let (in_whole, in_parts) = held(conn)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in candidates {
        let Some((name, folder)) = whole_of(&c.artifact).or_else(|| part_of(&c.artifact)) else {
            continue;
        };
        if !seen.insert(folder.clone()) {
            continue;
        }
        let id = identity(&c.artifact);
        out.push(PluginHold {
            whole: id.as_ref().is_some_and(|i| in_whole.contains(i)),
            parts: id.as_ref().is_some_and(|i| in_parts.contains(i)),
            name,
            folder,
        });
    }
    Ok(out)
}
