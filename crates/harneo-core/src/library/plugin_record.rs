//! A plugin's record (ADR-008): what the library keeps of a plugin whose parts it holds, so a
//! profile can scaffold the kept parts as a trimmed plugin.
//!
//! The record holds the plugin's name, its manifest, and the supporting files its parts need
//! (the files and folders outside each part's own that Claude listed, or every unclaimed file of
//! the plugin's folder when Claude was not asked). The files are stored once per plugin and
//! source, not once per part: two parts that need `references/` share its files. Each part
//! records its plugin in `metadata.fromPlugin` (`name`, `folder`, `marketplace`, `manifest`) and
//! its own supporting paths in `fromPlugin.needs`. A part with no `needs` list predates this
//! record: it installs as a loose copy and is marked *Re-import to update*.
//!
//! Paths in the record are relative to the plugin's folder, the way they are written into the
//! trimmed plugin.

use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::copy::{refusal, safe_join};
use super::is_executable;
use crate::artifact::walk::Walker;
use crate::artifact::{Artifact, ScanOptions};
use crate::error::{Error, Result};

/// Where the manifest of a trimmed plugin is written, relative to the plugin's folder.
pub(crate) const MANIFEST_FILE: &str = ".claude-plugin/plugin.json";

/// One supporting file, relative to the plugin's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SupportFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// A plugin record read from a source, ready to be saved.
#[derive(Debug, Clone)]
pub(crate) struct Built {
    /// The plugin's folder in its source (`""` for a plugin at the root).
    pub folder: String,
    pub name: String,
    pub manifest: String,
    pub files: Vec<SupportFile>,
}

/// A plugin record as the library holds it.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct Stored {
    pub name: String,
    pub manifest: String,
    pub files: Vec<SupportFile>,
}

fn plugin_object(artifact: &Artifact) -> Option<&serde_json::Map<String, Value>> {
    artifact.metadata.get("fromPlugin")?.as_object()
}

/// The plugin a part came from: its name and folder.
pub(crate) fn plugin_of(artifact: &Artifact) -> Option<(String, String)> {
    let from = plugin_object(artifact)?;
    let folder = from.get("folder").and_then(Value::as_str)?;
    let name = from.get("name").and_then(Value::as_str).unwrap_or(folder);
    Some((name.to_owned(), folder.to_owned()))
}

/// The supporting paths a part records, `None` for a part that predates the record.
pub(crate) fn needs_of(artifact: &Artifact) -> Option<Vec<String>> {
    let list = plugin_object(artifact)?.get("needs")?.as_array()?;
    Some(
        list.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

/// The names of the other parts of its plugin that a part names, recorded at import.
pub(crate) fn uses_of(artifact: &Artifact) -> Vec<String> {
    plugin_object(artifact)
        .and_then(|from| from.get("uses"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Records the plugin's manifest file, the part's supporting paths and the other parts of the
/// plugin it names (`uses`) on the part.
pub(crate) fn stamp(artifact: &mut Artifact, needs: &[String], uses: &[String]) {
    if let Some(Value::Object(from)) = artifact.metadata.get_mut("fromPlugin") {
        from.insert("manifest".into(), json!(MANIFEST_FILE));
        from.insert("needs".into(), json!(needs));
        from.insert("uses".into(), json!(uses));
    }
}

fn is_dot(folder: &str) -> bool {
    folder.is_empty() || folder == "."
}

/// `path` relative to the plugin's `folder`, or `None` when it lies outside it.
pub(crate) fn within(folder: &str, path: &str) -> Option<String> {
    if is_dot(folder) {
        return Some(path.to_owned());
    }
    path.strip_prefix(folder)
        .and_then(|rest| rest.strip_prefix('/'))
        .map(str::to_owned)
}

/// Every file (relative to the source root) the `needs` of a part name inside the plugin's
/// folder: a file itself, or the files below a folder. The plugin's manifest folder is written
/// by the scaffold, so it is not carried.
pub(crate) fn expand(root: &Path, folder: &str, needs: &[String]) -> Vec<String> {
    let options = ScanOptions::default();
    let walker = Walker::new(root, &options);
    let mut out: Vec<String> = Vec::new();
    for need in needs {
        if within(folder, need).is_none() {
            continue;
        }
        let listed = if root.join(need).is_dir() {
            walker.list_files(Path::new(need), &mut Vec::new())
        } else {
            vec![need.clone()]
        };
        for file in listed {
            let Some(rel) = within(folder, &file) else {
                continue;
            };
            if rel.starts_with(".claude-plugin/") || out.contains(&file) {
                continue;
            }
            out.push(file);
        }
    }
    out.sort();
    out
}

/// The plugin's manifest as the source has it, or a minimal one naming the plugin (Claude Code
/// allows a plugin with no manifest).
fn manifest_text(root: &Path, folder: &str, name: &str) -> String {
    let rel = if is_dot(folder) {
        MANIFEST_FILE.to_owned()
    } else {
        format!("{folder}/{MANIFEST_FILE}")
    };
    let read = || -> Option<String> {
        if refusal(root, &rel, 1024 * 1024).ok()?.is_some() {
            return None;
        }
        let text = fs::read_to_string(safe_join(root, &rel).ok()?).ok()?;
        serde_json::from_str::<Value>(&text)
            .ok()
            .filter(Value::is_object)
            .map(|_| text)
    };
    read().unwrap_or_else(|| format!("{{\n  \"name\": {}\n}}\n", json!(name)))
}

/// Reads the plugin's record from `root`: its manifest and the files `needs` names, each
/// checked like any copied file (no symlink, no secret-named or local file, within `max`).
pub(crate) fn build(
    root: &Path,
    folder: &str,
    name: &str,
    needs: &[String],
    max: u64,
) -> Result<Built> {
    let mut files = Vec::new();
    for file in expand(root, folder, needs) {
        if refusal(root, &file, max)?.is_some() {
            continue;
        }
        let source = safe_join(root, &file)?;
        let bytes = fs::read(&source).map_err(|e| Error::Io {
            path: source.display().to_string(),
            source: e,
        })?;
        files.push(SupportFile {
            path: within(folder, &file).unwrap_or(file),
            bytes,
            executable: is_executable(&source),
        });
    }
    Ok(Built {
        folder: folder.to_owned(),
        name: name.to_owned(),
        manifest: manifest_text(root, folder, name),
        files,
    })
}

/// Saves the record of the plugin at `folder` of `source_id`, and its files: a file already
/// stored is replaced, so a file two parts need is kept once.
pub(crate) fn save(conn: &Connection, source_id: &str, built: &Built) -> Result<()> {
    conn.execute(
        "INSERT INTO plugin_records (source_id, folder, name, manifest) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (source_id, folder) DO UPDATE SET name = ?3, manifest = ?4",
        params![source_id, built.folder, built.name, built.manifest],
    )?;
    for file in &built.files {
        conn.execute(
            "INSERT INTO plugin_files (source_id, folder, path, bytes, executable)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (source_id, folder, path)
             DO UPDATE SET bytes = ?4, executable = ?5",
            params![
                source_id,
                built.folder,
                file.path,
                file.bytes,
                file.executable
            ],
        )?;
    }
    Ok(())
}

/// The record of the plugin at `folder` of `source_id`.
#[cfg(test)]
pub(crate) fn load(conn: &Connection, source_id: &str, folder: &str) -> Result<Option<Stored>> {
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT name, manifest FROM plugin_records WHERE source_id = ?1 AND folder = ?2",
            params![source_id, folder],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((name, manifest)) = found else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(
        "SELECT path, bytes, executable FROM plugin_files
         WHERE source_id = ?1 AND folder = ?2 ORDER BY path",
    )?;
    let files = stmt
        .query_map(params![source_id, folder], |r| {
            Ok(SupportFile {
                path: r.get(0)?,
                bytes: r.get(1)?,
                executable: r.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Some(Stored {
        name,
        manifest,
        files,
    }))
}

/// Whether the library holds the record of the plugin at `folder` of `source_id`.
pub(crate) fn exists(conn: &Connection, source_id: &str, folder: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM plugin_records WHERE source_id = ?1 AND folder = ?2",
            params![source_id, folder],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// The paths of the record's supporting files, sorted.
pub(crate) fn file_paths(conn: &Connection, source_id: &str, folder: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path FROM plugin_files WHERE source_id = ?1 AND folder = ?2 ORDER BY path",
    )?;
    let paths = stmt
        .query_map(params![source_id, folder], |r| r.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(paths)
}

/// One supporting file of the record.
pub(crate) fn file(
    conn: &Connection,
    source_id: &str,
    folder: &str,
    path: &str,
) -> Result<Option<SupportFile>> {
    Ok(conn
        .query_row(
            "SELECT bytes, executable FROM plugin_files
             WHERE source_id = ?1 AND folder = ?2 AND path = ?3",
            params![source_id, folder, path],
            |r| {
                Ok(SupportFile {
                    path: path.to_owned(),
                    bytes: r.get(0)?,
                    executable: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// The plugin's manifest text, as stored.
pub(crate) fn manifest(conn: &Connection, source_id: &str, folder: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT manifest FROM plugin_records WHERE source_id = ?1 AND folder = ?2",
            params![source_id, folder],
            |r| r.get(0),
        )
        .optional()?)
}

/// Deletes the records of `source_id` that no part is left to use.
pub(crate) fn prune(conn: &Connection, source_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM plugin_records WHERE source_id = ?1 AND NOT EXISTS (
           SELECT 1 FROM artifacts a WHERE a.source_id = plugin_records.source_id
             AND json_extract(a.artifact, '$.metadata.fromPlugin.folder') = plugin_records.folder)",
        params![source_id],
    )?;
    Ok(())
}
