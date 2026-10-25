//! Which files of a plugin folder belong to the plugin: tier 1 of the layout resolver, where the
//! manifest is authoritative (`spec/library.md`, `docs/decisions/ADR-001-layout-resolver.md`).
//!
//! The scope is the manifest (and a `marketplace.json` beside it), the components the manifest
//! declares or Claude Code's default folders hold, the files those components reference through
//! `${CLAUDE_PLUGIN_ROOT}`, and the root `README*` and `LICENSE*`. Nothing else in the folder.
//!
//! Manifest fields read (checked against Claude Code's plugin reference, 2026-10-25): `skills`,
//! `commands`, `agents`, `hooks`, `mcpServers`, `lspServers`, `outputStyles`; each a `./`-relative
//! path or an array of them, `commands` also an object map whose entries have a `source`, and
//! `hooks`, `mcpServers` and `lspServers` also inline objects. A present `skills`, `commands`,
//! `agents` or `outputStyles` field is used instead of its default folder; the default
//! `hooks/hooks.json`, `.mcp.json` and `.lsp.json` are always read, as Claude Code merges them.
//!
//! Diagnostic code: `plugin.path-escapes-plugin`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::plugin::{manifest_path, marketplace_path};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{rel_string, Diagnostic};

const ROOT_VAR: &str = "${CLAUDE_PLUGIN_ROOT}";

/// (manifest field, default location when the field is absent).
const REPLACED: [(&str, &str); 4] = [
    ("skills", "skills"),
    ("commands", "commands"),
    ("agents", "agents"),
    ("outputStyles", "output-styles"),
];

/// (manifest field, default file that is always read as well).
const MERGED: [(&str, &str); 3] = [
    ("hooks", "hooks/hooks.json"),
    ("mcpServers", ".mcp.json"),
    ("lspServers", ".lsp.json"),
];

/// What the scope collects: whole folders and single files, relative to the scan root.
#[derive(Default)]
struct Scope {
    dirs: BTreeSet<PathBuf>,
    files: BTreeSet<PathBuf>,
}

/// The listed files of `folder` that belong to the plugin whose manifest metadata is `metadata`
/// (what `plugin.json` holds besides `name` and `description`). Diagnostics about files outside
/// the scope are dropped; escaping paths are reported in `diagnostics`.
pub(super) fn scoped_files(
    walker: &mut Walker,
    folder: &Path,
    metadata: &Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let mut walk = Vec::new();
    let all = walker.list_files(folder, &mut walk);
    let mut scope = Scope::default();
    let manifest = manifest_path(folder);
    scope.files.insert(manifest.clone());
    scope.files.insert(marketplace_path(folder));

    for (field, default) in REPLACED {
        match metadata.get(field) {
            Some(value) => {
                for reference in paths_of(value) {
                    add_path(
                        walker,
                        folder,
                        &manifest,
                        &reference,
                        field == "skills",
                        &mut scope,
                        diagnostics,
                    );
                }
            }
            None => {
                scope.dirs.insert(folder.join(default));
            }
        }
    }
    let mut referencing: Vec<Value> = Vec::new();
    for (field, default) in MERGED {
        scope.files.insert(folder.join(default));
        match metadata.get(field) {
            Some(value) => {
                for reference in paths_of(value) {
                    add_path(
                        walker,
                        folder,
                        &manifest,
                        &reference,
                        false,
                        &mut scope,
                        diagnostics,
                    );
                    if let Ok(p) = resolve_reference(folder, &reference) {
                        referencing.push(Value::String(rel_string(&p)));
                    }
                }
                referencing.push(inline_of(value));
            }
            None => {}
        }
        referencing.push(Value::String(rel_string(&folder.join(default))));
    }
    if let Some(Value::Object(map)) = metadata.get("commands") {
        referencing.push(Value::Object(map.clone()));
    }
    add_references(
        walker,
        folder,
        &manifest,
        &referencing,
        &mut scope,
        diagnostics,
    );

    for name in root_docs(&all, folder) {
        scope.files.insert(PathBuf::from(name));
    }

    let kept: Vec<String> = all
        .into_iter()
        .filter(|f| {
            let p = Path::new(f);
            scope.files.contains(p) || scope.dirs.iter().any(|d| p.starts_with(d))
        })
        .collect();
    let kept_set: BTreeSet<&str> = kept.iter().map(String::as_str).collect();
    diagnostics.extend(
        walk.into_iter()
            .filter(|d| kept_set.contains(d.file.as_str())),
    );
    kept
}

/// The `./`-relative strings a component field names: a string, an array of strings, or the
/// `source` of each `commands` map entry. Inline objects name no path.
fn paths_of(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items.iter().flat_map(paths_of).collect(),
        Value::Object(map) => map
            .values()
            .filter_map(|v| v.get("source").and_then(Value::as_str))
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// The inline (non-path) parts of a component field, to search for plugin-root references.
fn inline_of(value: &Value) -> Value {
    match value {
        Value::String(_) => Value::Null,
        Value::Array(items) => Value::Array(items.iter().map(inline_of).collect()),
        other => other.clone(),
    }
}

/// Adds one declared path to the scope, or reports it when it leaves the plugin folder. With
/// `skills`, a path naming the plugin root adds only its skill folders, never the whole root.
fn add_path(
    walker: &Walker,
    folder: &Path,
    manifest: &Path,
    reference: &str,
    skills: bool,
    scope: &mut Scope,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = contained(folder, manifest, reference, diagnostics) else {
        return;
    };
    if skills && target == folder {
        scope.files.insert(folder.join("SKILL.md"));
        for sub in walker.list_dirs(folder) {
            if walker.exists(&sub.join("SKILL.md")) {
                scope.dirs.insert(sub);
            }
        }
    } else if walker.is_dir(&target) {
        scope.dirs.insert(target);
    } else {
        scope.files.insert(target);
    }
}

/// `reference` resolved inside `folder`; `None` (with a diagnostic) when it climbs out of it.
fn contained(
    folder: &Path,
    manifest: &Path,
    reference: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathBuf> {
    match resolve_reference(folder, reference) {
        Ok(p) if p.starts_with(folder) => Some(p),
        Ok(_) => {
            diagnostics.push(escapes(manifest, reference));
            None
        }
        Err(d) => {
            diagnostics.push(d);
            None
        }
    }
}

fn escapes(file: &Path, reference: &str) -> Diagnostic {
    Diagnostic::warning(
        "plugin.path-escapes-plugin",
        file,
        &format!("the path `{reference}` leaves the plugin folder and was not included"),
    )
}

/// Adds what the config files and inline configs in `sources` reference through
/// `${CLAUDE_PLUGIN_ROOT}`. A string naming a file is read as JSON and searched; any other string
/// is searched as it is.
fn add_references(
    walker: &mut Walker,
    folder: &Path,
    manifest: &Path,
    sources: &[Value],
    scope: &mut Scope,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut texts: Vec<String> = Vec::new();
    for source in sources {
        collect_strings(source, &mut texts);
    }
    let mut found = BTreeSet::new();
    for text in &texts {
        let path = Path::new(text);
        if path.starts_with(folder) && walker.is_file(path) {
            let mut ignored = Vec::new();
            if let Some(body) = walker.read_text(path, &mut ignored) {
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    let mut inner = Vec::new();
                    collect_strings(&json, &mut inner);
                    found.extend(inner.iter().flat_map(|s| root_references(s)));
                }
            }
        } else {
            found.extend(root_references(text));
        }
    }
    for reference in found {
        if let Some(target) = contained(folder, manifest, &reference, diagnostics) {
            if walker.is_dir(&target) {
                scope.dirs.insert(target);
            } else if walker.exists(&target) {
                scope.files.insert(target);
            }
        }
    }
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| collect_strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_strings(v, out)),
        _ => {}
    }
}

/// The plugin-relative paths written as `${CLAUDE_PLUGIN_ROOT}/…` in `text` (a command line,
/// an argument), quotes around the variable allowed.
fn root_references(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(ROOT_VAR) {
        rest = &rest[i + ROOT_VAR.len()..];
        let after = rest.trim_start_matches(['"', '\'', '\\']);
        let Some(path) = after.strip_prefix('/') else {
            continue;
        };
        let end = path
            .find(|c: char| c.is_whitespace() || "\"'`;|&<>()\\".contains(c))
            .unwrap_or(path.len());
        if end > 0 {
            out.push(path[..end].to_owned());
        }
    }
    out
}

/// The root `README*` and `LICENSE*` files among `all`.
fn root_docs(all: &[String], folder: &Path) -> Vec<String> {
    all.iter()
        .filter(|f| {
            let p = Path::new(f);
            p.parent() == Some(folder)
                && p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy().to_ascii_lowercase();
                    n.starts_with("readme") || n.starts_with("license")
                })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_references_in_command_lines() {
        assert_eq!(
            root_references("\"${CLAUDE_PLUGIN_ROOT}\"/scripts/x.sh --a"),
            ["scripts/x.sh"]
        );
        assert_eq!(
            root_references("node ${CLAUDE_PLUGIN_ROOT}/a/b.js ${CLAUDE_PLUGIN_ROOT}/c"),
            ["a/b.js", "c"]
        );
        assert!(root_references("${CLAUDE_PLUGIN_ROOT}").is_empty());
        assert!(root_references("no variable").is_empty());
    }

    #[test]
    fn paths_of_reads_strings_arrays_and_command_sources() {
        let v: Value = serde_json::json!({"a": {"source": "./c/a.md"}, "b": {"content": "x"}});
        assert_eq!(paths_of(&v), ["./c/a.md"]);
        assert_eq!(paths_of(&serde_json::json!(["./a", "./b"])), ["./a", "./b"]);
        assert_eq!(paths_of(&serde_json::json!("./a")), ["./a"]);
    }
}
