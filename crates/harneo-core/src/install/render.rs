//! The bytes of every file a profile writes into a project, from the library copies.
//!
//! Deterministic: the same library, profile and date give the same bytes (JSON keys sorted,
//! files in layout order). Nothing here touches a project or the library's files.

use std::fs;
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use ts_rs::TS;

use crate::artifact::{Artifact, ArtifactKind, Origin};
use crate::error::{Error, Result};
use crate::library::{
    derive_plugin_origin, is_executable, records, safe_join, ArtifactId, Library,
};
use crate::profiles::layout::{load, placed};
use crate::profiles::{Generated, PathFrom, ProfileSpec, Resolved, ResolvedItem};

const SETTINGS: &str = ".claude/settings.json";
const CLAUDE_MD: &str = "CLAUDE.md";
const SERVER_FILE: &str = "server.json";
const IMPORT_LINE: &str = "@HARNESS.md";
const MANUAL_KEY: &str = "disable-model-invocation";

/// One file of a profile, with its exact content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RenderedFile {
    /// Relative to the project root, with forward slashes.
    pub path: String,
    /// The content; base64 over IPC.
    #[serde(with = "base64_bytes")]
    #[ts(type = "string")]
    pub bytes: Vec<u8>,
    pub from: PathFrom,
    /// The library copy is executable (a script): written 0o755 on unix. Absent means not.
    #[serde(default)]
    pub executable: bool,
}

/// Everything a profile version writes, sorted by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Rendered {
    pub files: Vec<RenderedFile>,
}

mod base64_bytes {
    use super::{Engine, STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// `text` with `disable-model-invocation` set to `on` in its frontmatter. Only that key's line
/// changes; the rest stays byte-equal. Created only when `on` and the file has no frontmatter.
/// A file that is not UTF-8 or whose frontmatter is never closed is returned as it is.
pub(super) fn set_manual_only(bytes: &[u8], on: bool) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let (bom, rest) = match text.strip_prefix('\u{feff}') {
        Some(r) => ("\u{feff}", r),
        None => ("", text),
    };
    let value = if on { "true" } else { "false" };
    let lines: Vec<&str> = rest.split_inclusive('\n').collect();
    let Some(open) = lines.first().filter(|l| l.trim_end() == "---") else {
        if !on {
            return bytes.to_vec();
        }
        return format!("{bom}---\n{MANUAL_KEY}: true\n---\n{rest}").into_bytes();
    };
    let eol = if open.ends_with("\r\n") { "\r\n" } else { "\n" };
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
    else {
        return bytes.to_vec();
    };
    let key = lines[1..close].iter().position(|l| {
        l.strip_prefix(MANUAL_KEY)
            .is_some_and(|r| r.trim_start().starts_with(':'))
    });
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    match key {
        Some(i) => {
            let line = lines[i + 1];
            let current = line
                .split_once(':')
                .map_or("", |(_, v)| v.trim().trim_matches(['"', '\'']));
            if current == value {
                return bytes.to_vec();
            }
            let end = &line[line.trim_end_matches(['\r', '\n']).len()..];
            out[i + 1] = format!("{MANUAL_KEY}: {value}{end}");
        }
        None if on => out.insert(close, format!("{MANUAL_KEY}: true{eol}")),
        None => return bytes.to_vec(),
    }
    format!("{bom}{}", out.concat()).into_bytes()
}

/// `text` followed by the `@HARNESS.md` import, unless a line already is that import.
pub(crate) fn with_harness_import(bytes: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    if text.lines().any(|l| l.trim() == IMPORT_LINE) {
        return bytes.to_vec();
    }
    let mut out = text.to_owned();
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(IMPORT_LINE);
    out.push('\n');
    out.into_bytes()
}

fn pretty(value: &Value) -> Result<Vec<u8>> {
    let mut out = serde_json::to_vec_pretty(value)?;
    out.push(b'\n');
    Ok(out)
}

/// Runs `change` on the object at `key` of `root`, making it an object first if it is absent or
/// holds something else.
fn update_object(
    root: &mut Map<String, Value>,
    key: &str,
    change: impl FnOnce(&mut Map<String, Value>),
) {
    let mut map = match root.remove(key) {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    change(&mut map);
    root.insert(key.to_owned(), Value::Object(map));
}

/// Why `plugin` cannot be enabled from `settings.json`, or `None` when it can: it needs a
/// marketplace and, unless that is Anthropic's own, the GitHub repo that marketplace lives in.
pub(crate) fn not_installable(plugin: &Artifact) -> Option<String> {
    match &plugin.origin {
        Origin::Marketplace { official: true, .. } | Origin::Marketplace { repo: Some(_), .. } => {
            None
        }
        Origin::Marketplace { name, .. } => Some(format!(
            "not installable: marketplace {name} has no known GitHub repo"
        )),
        _ => Some(
            match plugin.metadata.get("marketplace").and_then(Value::as_str) {
                Some(name) => {
                    format!("not installable: marketplace {name} has no known GitHub repo")
                }
                None => "not installable: no marketplace".to_owned(),
            },
        ),
    }
}

/// The template's settings (or `{}`) with the plugins enabled. With no plugins the template's
/// bytes are returned untouched. A plugin that cannot be enabled is left out here; the caller
/// reports it (see [`not_installable`]).
fn settings_bytes(template: Option<Vec<u8>>, plugins: &[Artifact]) -> Result<Vec<u8>> {
    if plugins.is_empty() {
        if let Some(bytes) = template {
            return Ok(bytes);
        }
    }
    let unreadable = || Error::UnreadableSettings {
        path: SETTINGS.to_owned(),
    };
    let mut root: Value = match template {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| unreadable())?,
        None => Value::Object(Map::new()),
    };
    let Value::Object(map) = &mut root else {
        return Err(unreadable());
    };
    for plugin in plugins.iter().filter(|p| not_installable(p).is_none()) {
        let Origin::Marketplace {
            name: market,
            repo,
            official,
        } = &plugin.origin
        else {
            continue;
        };
        update_object(map, "enabledPlugins", |m| {
            m.insert(format!("{}@{market}", plugin.name), Value::Bool(true));
        });
        if let (false, Some(repo)) = (official, repo) {
            update_object(map, "extraKnownMarketplaces", |m| {
                m.entry(market.clone())
                    .or_insert_with(|| json!({"source": {"source": "github", "repo": repo}}));
            });
        }
    }
    pretty(&root)
}

/// Said in `HARNESS.md` and by the scaffold and refit screens.
pub(crate) const PLUGINS_NOTE: &str = "Plugins are not copied: they install from their own marketplace. `.claude/settings.json` enables them, and Harneo installs them for this project through the `claude` CLI when it scaffolds or refits it. Anyone else who opens the project is offered them by Claude Code when trusting the folder.";

/// `name: reason` for every plugin that cannot be enabled.
fn warnings_of(plugins: &[Artifact]) -> Vec<String> {
    plugins
        .iter()
        .filter_map(|p| not_installable(p).map(|why| format!("{}: {why}", p.name)))
        .collect()
}

fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::ClaudeMd => "CLAUDE.md",
        ArtifactKind::Rule => "Rule",
        ArtifactKind::Skill => "Skill",
        ArtifactKind::Command => "Command",
        ArtifactKind::Agent => "Subagent",
        ArtifactKind::Settings => "Settings",
        ArtifactKind::Mcp => "MCP server",
        ArtifactKind::OutputStyle => "Output style",
        ArtifactKind::Workflow => "Workflow",
        ArtifactKind::Plugin => "Plugin",
    }
}

/// The first sentence of a description, on one line and safe inside a table cell.
fn first_sentence(description: &str) -> String {
    let flat = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let end = flat
        .char_indices()
        .find(|&(i, c)| c == '.' && flat[i + 1..].chars().next().is_none_or(char::is_whitespace))
        .map_or(flat.len(), |(i, _)| i);
    flat[..end].replace('|', "\\|")
}

/// Where the artifact came from, for the HARNESS.md table.
fn source_label(artifact: &Artifact, location: &str, git: bool) -> String {
    let text = match &artifact.origin {
        Origin::Marketplace { name, repo, .. } => repo.clone().unwrap_or_else(|| name.clone()),
        Origin::ClaudeSynced => "Claude synced".to_owned(),
        Origin::DevMod => "Dev mod".to_owned(),
        Origin::Own if git => {
            let trimmed = location.trim_end_matches('/').trim_end_matches(".git");
            let tail: Vec<&str> = trimmed.rsplit(['/', ':']).take(2).collect();
            match tail.as_slice() {
                [repo, owner] if !owner.is_empty() => format!("{owner}/{repo}"),
                _ => trimmed.to_owned(),
            }
        }
        Origin::Own => "Local".to_owned(),
    };
    text.replace('|', "\\|")
}

impl Library {
    /// The exact content of every file profile `profile_name` (at `version`, on `date`) writes
    /// into a project, in layout order. Library copies are byte-equal except for the
    /// documented edits; see the install-engine spec.
    pub fn render(
        &self,
        spec: &ProfileSpec,
        profile_name: &str,
        version: u32,
        date: &str,
    ) -> Result<Rendered> {
        let layout = self.layout(spec)?;
        let resolved = self.resolve(spec)?;
        let plugins = self.plugins_of(&resolved)?;
        let mut files = Vec::new();
        for planned in &layout.paths {
            let (bytes, executable) = match &planned.from {
                PathFrom::Generated {
                    generated: Generated::HarnessMd,
                } => continue,
                PathFrom::Generated {
                    generated: Generated::McpJson,
                } => (self.mcp_json(spec, &resolved)?, false),
                PathFrom::Generated {
                    generated: Generated::Settings,
                } => (settings_bytes(None, &plugins)?, false),
                PathFrom::Artifact { artifact } => {
                    self.artifact_bytes(artifact, &planned.path, &resolved, &plugins)?
                }
            };
            files.push(RenderedFile {
                path: planned.path.clone(),
                bytes,
                from: planned.from.clone(),
                executable,
            });
        }
        let harness = self.harness_md(
            &resolved,
            &layout.paths,
            &plugins,
            profile_name,
            version,
            date,
        )?;
        files.push(RenderedFile {
            path: "HARNESS.md".to_owned(),
            bytes: harness.into_bytes(),
            from: PathFrom::Generated {
                generated: Generated::HarnessMd,
            },
            executable: false,
        });
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Rendered { files })
    }

    /// The profile's plugins as stored, with the marketplace origin derived from the source for
    /// records imported before imports set it.
    fn plugins_of(&self, resolved: &Resolved) -> Result<Vec<Artifact>> {
        let mut plugins = Vec::new();
        for item in resolved
            .artifacts
            .iter()
            .filter(|i| i.kind == ArtifactKind::Plugin)
        {
            if let Some(mut a) = records::load(&self.store.conn, &item.id)?.map(|d| {
                let s = d.provenance.source;
                (d.artifact, s)
            }) {
                if let Some(origin) = derive_plugin_origin(&a.0, &a.1.kind, &a.1.location) {
                    a.0.origin = origin;
                }
                plugins.push(a.0);
            }
        }
        Ok(plugins)
    }

    /// One warning per plugin of the profile that its `settings.json` cannot enable.
    pub fn plugin_warnings(&self, spec: &ProfileSpec) -> Result<Vec<String>> {
        let resolved = self.resolve(spec)?;
        Ok(warnings_of(&self.plugins_of(&resolved)?))
    }

    fn artifact_bytes(
        &self,
        id: &ArtifactId,
        path: &str,
        resolved: &Resolved,
        plugins: &[Artifact],
    ) -> Result<(Vec<u8>, bool)> {
        let artifact =
            load(&self.store.conn, id)?.ok_or_else(|| Error::NotInLibrary { id: id.clone() })?;
        let (view, _) = crate::library::stored_view(&artifact)?;
        let stored = placed(&artifact)?
            .into_iter()
            .find(|(_, project)| project == path)
            .map(|(stored, _)| stored)
            .ok_or_else(|| Error::NotInLibrary { id: id.clone() })?;
        let file = safe_join(&self.store.artifacts_dir().join(id), &stored)?;
        let bytes = fs::read(&file).map_err(|e| io_error(&file, e))?;
        let main = stored == view.path;
        let executable = is_executable(&file);
        let bytes = match artifact.kind {
            ArtifactKind::Settings if main => settings_bytes(Some(bytes), plugins)?,
            ArtifactKind::ClaudeMd if path == CLAUDE_MD => with_harness_import(&bytes),
            ArtifactKind::Skill if stored == "SKILL.md" => {
                let manual = resolved
                    .artifacts
                    .iter()
                    .find(|i| &i.id == id)
                    .is_some_and(|i: &ResolvedItem| i.manual_only);
                set_manual_only(&bytes, manual)
            }
            _ => bytes,
        };
        Ok((bytes, executable))
    }

    fn mcp_json(&self, spec: &ProfileSpec, resolved: &Resolved) -> Result<Vec<u8>> {
        let mut servers = Map::new();
        for item in resolved
            .artifacts
            .iter()
            .filter(|i| i.kind == ArtifactKind::Mcp)
        {
            let file = safe_join(&self.store.artifacts_dir().join(&item.id), SERVER_FILE)?;
            let bytes = fs::read(&file).map_err(|e| io_error(&file, e))?;
            let Value::Object(defs) = serde_json::from_slice(&bytes)? else {
                continue;
            };
            let env = spec
                .mcp
                .iter()
                .find(|m| m.artifact == item.id)
                .map(|m| &m.env);
            for (name, mut def) in defs {
                if let (Some(env), Value::Object(fields)) = (env, &mut def) {
                    if !env.is_empty() {
                        update_object(fields, "env", |slot| {
                            for (k, v) in env {
                                slot.insert(k.clone(), Value::String(v.clone()));
                            }
                        });
                    }
                }
                servers.insert(name, def);
            }
        }
        pretty(&json!({ "mcpServers": servers }))
    }

    fn harness_md(
        &self,
        resolved: &Resolved,
        paths: &[crate::profiles::PlannedPath],
        plugins: &[Artifact],
        profile_name: &str,
        version: u32,
        date: &str,
    ) -> Result<String> {
        let mut out = format!(
            "# Harness\n\nInstalled by Harneo from profile **{profile_name} v{version}** on {date}.\n\n\
             | Artifact | Type | Source | Why it's here |\n|---|---|---|---|\n"
        );
        for item in &resolved.artifacts {
            let detail = self.detail(&item.id)?;
            let source = source_label(
                &detail.artifact,
                &detail.provenance.source.location,
                detail.provenance.source.kind == "git",
            );
            let why = detail
                .artifact
                .description
                .as_deref()
                .map(first_sentence)
                .unwrap_or_default();
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                item.name.replace('|', "\\|"),
                kind_label(item.kind),
                source,
                why
            ));
        }
        if !plugins.is_empty() {
            out.push_str(&format!("\n## Plugins\n\n{PLUGINS_NOTE}\n"));
            let missing = warnings_of(plugins);
            if !missing.is_empty() {
                out.push_str("\n### Not enabled\n\n");
                for line in missing {
                    out.push_str(&format!("- {line}\n"));
                }
            }
        }
        out.push_str("\n## Files\n\n");
        let mut listed: Vec<&str> = paths.iter().map(|p| p.path.as_str()).collect();
        listed.push("HARNESS.md");
        listed.sort_unstable();
        listed.dedup();
        for path in listed {
            out.push_str(&format!("- `{path}`\n"));
        }
        out.push_str("\nManaged by Harneo: edit the library copy, or your edits show as drift.\n");
        Ok(out)
    }
}
