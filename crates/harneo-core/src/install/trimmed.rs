//! The manifest of a trimmed plugin (ADR-008): the plugin's own `plugin.json` with its component
//! paths narrowed to the parts a profile kept, and the keys that pointed only at dropped parts
//! removed. Everything else in the manifest (name, version, `userConfig`, `dependencies`...) is
//! kept as it is.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};

/// A kept part: its kind and its path relative to the plugin's folder (a skill's folder, or the
/// file of an agent, command or output style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeptPart {
    pub kind: ArtifactKind,
    pub path: String,
}

/// The manifest keys that name component paths: the kind, the key, and the folder Claude Code
/// reads by default.
const COMPONENTS: [(ArtifactKind, &str, &str); 4] = [
    (ArtifactKind::Skill, "skills", "skills"),
    (ArtifactKind::Agent, "agents", "agents"),
    (ArtifactKind::Command, "commands", "commands"),
    (ArtifactKind::OutputStyle, "outputStyles", "output-styles"),
];

fn normal(path: &str) -> &str {
    path.trim_start_matches("./").trim_end_matches('/')
}

/// True when the manifest entry `entry` is a kept part or a folder that holds one.
fn names_kept(entry: &str, kept: &[&str]) -> bool {
    let e = normal(entry);
    e.is_empty()
        || e == "."
        || kept
            .iter()
            .any(|p| *p == e || p.starts_with(&format!("{e}/")))
}

/// A manifest value narrowed to the kept parts; `None` when nothing it names is kept.
fn narrow_value(value: &Value, kept: &[&str]) -> Option<Value> {
    match value {
        Value::String(s) => names_kept(s, kept).then(|| value.clone()),
        Value::Array(items) => {
            let left: Vec<Value> = items
                .iter()
                .filter(|item| match item {
                    Value::String(s) => names_kept(s, kept),
                    _ => true,
                })
                .cloned()
                .collect();
            (!left.is_empty()).then_some(Value::Array(left))
        }
        // `{ "name": "./path" }` or `{ "name": { "source": "./path" } }`.
        Value::Object(map) => {
            let left: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(_, v)| match v {
                    Value::String(s) => names_kept(s, kept),
                    Value::Object(o) => o
                        .get("source")
                        .and_then(Value::as_str)
                        .is_none_or(|s| names_kept(s, kept)),
                    _ => true,
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            (!left.is_empty()).then_some(Value::Object(left))
        }
        other => Some(other.clone()),
    }
}

/// True when `path` is where Claude Code finds a `kind` part without the manifest naming it.
fn in_default_place(path: &str, folder: &str) -> bool {
    path.strip_prefix(folder)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

/// `manifest` narrowed to `kept`. `files` are the files of the trimmed folder (relative to it): a
/// `hooks` path stays only when its file is there. A manifest that needs no change is returned
/// byte for byte.
pub(crate) fn narrow(
    manifest: &str,
    kept: &[KeptPart],
    files: &BTreeSet<String>,
) -> Result<Vec<u8>> {
    let Value::Object(mut root) = serde_json::from_str::<Value>(manifest)? else {
        return Err(Error::Install {
            message: "a plugin's manifest is not a JSON object".to_owned(),
        });
    };
    let original = Value::Object(root.clone());
    for (kind, key, default) in COMPONENTS {
        let paths: Vec<&str> = kept
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| normal(&p.path))
            .collect();
        match root.get(key) {
            Some(value) => match narrow_value(value, &paths) {
                Some(narrowed) => {
                    root.insert(key.to_owned(), narrowed);
                }
                None => {
                    root.remove(key);
                }
            },
            // Parts outside the default folder are found only when the manifest names them.
            None if paths.iter().any(|p| !in_default_place(p, default)) => {
                let listed = paths
                    .iter()
                    .map(|p| Value::String(format!("./{p}")))
                    .collect();
                root.insert(key.to_owned(), Value::Array(listed));
            }
            None => {}
        }
    }
    if let Some(hooks) = root.get("hooks").cloned() {
        let present = |s: &str| {
            let e = normal(s);
            files.contains(e) || files.iter().any(|f| f.starts_with(&format!("{e}/")))
        };
        let narrowed = match &hooks {
            Value::String(s) => present(s).then(|| hooks.clone()),
            Value::Array(items) => {
                let left: Vec<Value> = items
                    .iter()
                    .filter(|i| i.as_str().is_none_or(present))
                    .cloned()
                    .collect();
                (!left.is_empty()).then_some(Value::Array(left))
            }
            _ => Some(hooks.clone()),
        };
        match narrowed {
            Some(v) => {
                root.insert("hooks".to_owned(), v);
            }
            None => {
                root.remove("hooks");
            }
        }
    }
    let narrowed = Value::Object(root);
    if narrowed == original {
        return Ok(manifest.as_bytes().to_vec());
    }
    let mut out = serde_json::to_vec_pretty(&narrowed)?;
    out.push(b'\n');
    Ok(out)
}
