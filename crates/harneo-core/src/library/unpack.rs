//! Unpacked plugins at import (`spec/library.md`, "Unpacking a plugin"): a plugin is in the
//! library whole or as some of its parts, never both, and the choice is kept on its source.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde_json::Value;

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

/// The plugin an artifact is, when it is a whole Plugin.
fn whole_of(artifact: &Artifact) -> Option<(String, String)> {
    (artifact.kind == ArtifactKind::Plugin).then(|| (artifact.name.clone(), artifact.path.clone()))
}

fn exclusive(plugin: &str, detail: String) -> Error {
    Error::PluginExclusive {
        plugin: plugin.to_owned(),
        detail,
    }
}

/// The plugin folders (and names) of the library's artifacts of `source_id`, whole and unpacked.
fn held(
    conn: &Connection,
    source_id: &str,
) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
    let mut stmt = conn.prepare("SELECT artifact FROM artifacts WHERE source_id = ?1")?;
    let rows = stmt.query_map(params![source_id], |r| r.get::<_, String>(0))?;
    let (mut whole, mut parts) = (HashMap::new(), HashMap::new());
    for row in rows {
        let Ok(artifact) = serde_json::from_str::<Artifact>(&row?) else {
            continue;
        };
        if let Some((name, folder)) = whole_of(&artifact) {
            whole.insert(folder, name);
        } else if let Some((name, folder)) = part_of(&artifact) {
            parts.insert(folder, name);
        }
    }
    Ok((whole, parts))
}

/// Refuses a selection that breaks the rules: a part that only works with its plugin, a plugin
/// and its parts picked together, parts while the plugin is in the library whole, or the plugin
/// whole while parts of it are. `chosen` is the whole selection; `fresh` the part of it not in
/// the library yet.
pub(super) fn check(
    conn: &Connection,
    source_id: &str,
    chosen: &[&Candidate],
    fresh: &[&Candidate],
) -> Result<()> {
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
    let (in_whole, in_parts) = held(conn, source_id)?;
    for c in fresh {
        if let Some((name, folder)) = part_of(&c.artifact) {
            if in_whole.contains_key(&folder) {
                return Err(exclusive(
                    &name,
                    format!("the plugin is in the library whole; remove the plugin {name} first to import its parts"),
                ));
            }
        }
        if let Some((name, folder)) = whole_of(&c.artifact) {
            if in_parts.contains_key(&folder) {
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
