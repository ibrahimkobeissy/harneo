//! Unpacking a plugin: its pickable parts reported as ordinary artifacts (`spec/library.md`,
//! "Unpacking a plugin"). The parts are the components `plugin_scope` resolves from the manifest
//! and Claude Code's default folders: skills, agents, commands, output styles and each MCP server.
//! Hooks, monitors, LSP servers, `bin/`, themes, workflows and the plugin's `settings.json` are
//! not parts: they only work with the whole plugin.
//!
//! Every part carries `metadata.fromPlugin` (`name`, `folder`, `marketplace`) and
//! `layoutBy: "plugin.json"`. Any part can be kept (ADR-008): what it needs from outside its own
//! files is Claude's answer, stored with the source, not an error here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::plugin_scope::{add_replaced, add_skills, contained, paths_of, Scope};
use super::rule::{read_file, Parser};
use super::{agent, command, mcp, output_style, skill};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, Diagnostic, ScanReport};

const MCP_FILE: &str = ".mcp.json";

/// The parts of `plugin` (a Plugin read from a local folder), sorted by path. Findings about the
/// plugin itself go to `root_diagnostics`.
pub(crate) fn unpack(
    walker: &mut Walker,
    plugin: &Artifact,
    root_diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Artifact> {
    let folder = Path::new(&plugin.path);
    let mut diagnostics: Vec<Diagnostic> = plugin
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("plugin.") || d.code.starts_with("marketplace."))
        .cloned()
        .collect();
    let mut parts = Vec::new();

    let mut skills = Scope::default();
    add_skills(
        walker,
        folder,
        &plugin.metadata,
        &mut skills,
        &mut diagnostics,
    );
    for dir in skill_dirs(walker, &skills) {
        let mut found = ScanReport::default();
        skill::read_skill(walker, &dir, &mut found);
        for part in found.artifacts {
            parts.push(part);
        }
    }

    let markdown: [(&str, &str, Parser); 3] = [
        ("agents", "agents", agent::parse),
        ("commands", "commands", command::parse),
        ("outputStyles", "output-styles", output_style::parse),
    ];
    for (field, default, parse) in markdown {
        let mut scope = Scope::default();
        add_replaced(
            walker,
            folder,
            &plugin.metadata,
            (field, default),
            &mut scope,
            &mut diagnostics,
        );
        for (rel, base) in markdown_files(walker, &scope) {
            let part = read_file(walker, &base, &rel, parse);
            parts.push(part);
        }
    }

    parts.extend(mcp_servers(walker, plugin, &mut diagnostics));

    let marketplace = plugin
        .metadata
        .get("marketplace")
        .cloned()
        .unwrap_or(Value::Null);
    for part in &mut parts {
        part.metadata.insert(
            "fromPlugin".into(),
            json!({"name": plugin.name, "folder": plugin.path, "marketplace": marketplace}),
        );
        part.metadata
            .insert("layoutBy".into(), "plugin.json".into());
    }
    parts.sort_by(|a, b| a.path.cmp(&b.path));
    for d in diagnostics {
        if !root_diagnostics.contains(&d) {
            root_diagnostics.push(d);
        }
    }
    parts
}

/// The skill folders a skills scope holds: folders with a `SKILL.md`, and the folder of a
/// `SKILL.md` the scope names.
fn skill_dirs(walker: &Walker, scope: &Scope) -> BTreeSet<PathBuf> {
    let from_files = scope
        .files
        .iter()
        .filter(|f| f.file_name().is_some_and(|n| n == "SKILL.md"))
        .filter_map(|f| f.parent().map(Path::to_path_buf));
    scope
        .dirs
        .iter()
        .cloned()
        .chain(from_files)
        .filter(|d| walker.exists(&d.join("SKILL.md")))
        .collect()
}

/// The Markdown files a scope holds, each with the folder the type counts its name from.
fn markdown_files(walker: &Walker, scope: &Scope) -> BTreeMap<PathBuf, PathBuf> {
    let is_md = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
    let mut out = BTreeMap::new();
    for dir in &scope.dirs {
        for file in walker.list_files(dir, &mut Vec::new()) {
            let file = PathBuf::from(file);
            if is_md(&file) {
                out.insert(file, dir.clone());
            }
        }
    }
    for file in &scope.files {
        if is_md(file) && walker.is_file(file) {
            let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
            out.entry(file.clone()).or_insert(base);
        }
    }
    out
}

/// Each server of the plugin's `mcpServers` field and default `.mcp.json`, one artifact each.
/// A server that names a later definition of the same name is replaced by it, as Claude Code
/// merges them.
fn mcp_servers(
    walker: &mut Walker,
    plugin: &Artifact,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Artifact> {
    let folder = Path::new(&plugin.path);
    let manifest = super::plugin::manifest_path(folder);
    let mut configs: Vec<(String, Value)> = Vec::new();
    let read_config = |walker: &mut Walker, path: &Path, configs: &mut Vec<(String, Value)>| {
        if !walker.is_file(path) {
            return;
        }
        let text = walker.read_text(path, &mut Vec::new());
        if let Some(json) = text.and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            configs.push((rel_string(path), json));
        }
    };
    read_config(walker, &folder.join(MCP_FILE), &mut configs);
    let mut inline: Vec<Value> = Vec::new();
    if let Some(field) = plugin.metadata.get("mcpServers") {
        for item in as_items(field) {
            match item {
                Value::String(_) => {
                    for reference in paths_of(item) {
                        if let Some(path) = contained(folder, &manifest, &reference, diagnostics) {
                            read_config(walker, &path, &mut configs);
                        }
                    }
                }
                Value::Object(_) => inline.push(item.clone()),
                _ => {}
            }
        }
    }
    configs.extend(inline.into_iter().map(|v| (rel_string(&manifest), v)));

    let mut servers: BTreeMap<String, Artifact> = BTreeMap::new();
    for (config, json) in configs {
        let map = match json.get("mcpServers") {
            Some(Value::Object(inner)) => inner,
            _ => match &json {
                Value::Object(map) => map,
                _ => continue,
            },
        };
        for (name, def) in map {
            if !def.is_object() {
                continue;
            }
            let part = mcp::server(&config, name, def, &|_| false);
            servers.insert(name.clone(), part);
        }
    }
    servers.into_values().collect()
}

/// A field value as a list: an array's items, or the value itself.
fn as_items(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    }
}
