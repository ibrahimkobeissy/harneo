//! Plugins in one place: what a plugin holds, and whether it must stay whole (ADR-004).
//!
//! A plugin's parts are the pickable ones (skills, agents, commands, output styles and MCP
//! servers, which work the same as loose files) and the whole-only ones (hooks, monitors, LSP
//! servers, mods, `bin/`, themes, workflows and the plugin's own `settings.json`), which Claude
//! Code wires only inside a plugin. A plugin must stay whole when it holds a whole-only part,
//! when a pickable part refers outside itself through `${CLAUDE_PLUGIN_ROOT}`, or when it is a
//! remote-only marketplace entry whose files are never fetched.
//!
//! The reading of a plugin folder stays with the Claude adapter (`artifact::claude`); this module
//! answers the questions on top of it and has no dependency on the library.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::claude::unpack_plugin;
use crate::artifact::walk::Walker;
use crate::artifact::{Artifact, ArtifactKind, Severity};

/// A kind of component a plugin can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum PluginPart {
    Skills,
    Agents,
    Commands,
    OutputStyles,
    McpServers,
    Hooks,
    Monitors,
    LspServers,
    Mods,
    Bin,
    Themes,
    Workflows,
    Settings,
}

impl PluginPart {
    /// The part as a sentence names it.
    fn label(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Agents => "agents",
            Self::Commands => "commands",
            Self::OutputStyles => "output styles",
            Self::McpServers => "MCP servers",
            Self::Hooks => "hooks",
            Self::Monitors => "monitors",
            Self::LspServers => "LSP servers",
            Self::Mods => "mods",
            Self::Bin => "bin tools",
            Self::Themes => "themes",
            Self::Workflows => "workflows",
            Self::Settings => "settings",
        }
    }

    fn of_kind(kind: ArtifactKind) -> Option<Self> {
        match kind {
            ArtifactKind::Skill => Some(Self::Skills),
            ArtifactKind::Agent => Some(Self::Agents),
            ArtifactKind::Command => Some(Self::Commands),
            ArtifactKind::OutputStyle => Some(Self::OutputStyles),
            ArtifactKind::Mcp => Some(Self::McpServers),
            _ => None,
        }
    }
}

/// What a plugin holds, by kind of component.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PluginContents {
    /// Parts that can become ordinary library artifacts.
    pub pickable: Vec<PluginPart>,
    /// Parts that only work inside the plugin.
    pub whole_only: Vec<PluginPart>,
    /// Pickable parts that refer outside themselves through `${CLAUDE_PLUGIN_ROOT}`: they only
    /// work with the whole plugin.
    pub root_ties: Vec<PluginPart>,
}

/// Why a plugin has to be taken whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum WholeReason {
    /// It holds parts that only work inside a plugin.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    WholeOnlyParts { parts: Vec<PluginPart> },
    /// Some of its parts refer outside themselves through `${CLAUDE_PLUGIN_ROOT}`.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    PluginRootTie { parts: Vec<PluginPart> },
    /// A marketplace entry pointing to another repo, which is never fetched.
    RemoteOnly,
    /// It has no skills, agents, commands, output styles or MCP servers to unpack. Only the
    /// library's unpack action reports it: an import leaves such a plugin whole with a warning.
    NoParts,
}

impl WholeReason {
    /// A short sentence for the reader.
    pub fn text(&self) -> String {
        match self {
            Self::WholeOnlyParts { parts } => {
                format!(
                    "Has {}: they only work inside the plugin.",
                    join(parts)
                )
            }
            Self::PluginRootTie { parts } => format!(
                "Its {} refer to files outside themselves: they only work inside the plugin.",
                join(parts)
            ),
            Self::NoParts => {
                "It has no skills, agents, commands, output styles or MCP servers to unpack."
                    .to_owned()
            }
            Self::RemoteOnly => {
                "Its files live in another repo that is never fetched, so it can only be taken whole."
                    .to_owned()
            }
        }
    }
}

/// `a`, `a and b`, `a, b and c`.
fn join(parts: &[PluginPart]) -> String {
    let labels: Vec<&str> = parts.iter().map(|p| p.label()).collect();
    match labels.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The mark in a plugin's path that says its files are in another repo: `<file>#<entry>`, a
/// marketplace entry that is never fetched.
const REMOTE_MARK: char = '#';

/// True when `path` is a remote-only entry's path.
pub(crate) fn is_remote_path(path: &str) -> bool {
    path.contains(REMOTE_MARK)
}

/// The file a remote-only entry's path names (the marketplace file), before the mark.
pub(crate) fn remote_file(path: &str) -> &str {
    path.split(REMOTE_MARK).next().unwrap_or_default()
}

/// True when the plugin is a marketplace entry pointing to another repo.
pub(crate) fn is_remote_only(plugin: &Artifact) -> bool {
    is_remote_path(&plugin.path)
}

/// The folder of a plugin whose files are on disk: `None` for any other artifact and for a
/// remote-only entry.
pub(crate) fn local_folder(artifact: &Artifact) -> Option<&Path> {
    (artifact.kind == ArtifactKind::Plugin && !is_remote_only(artifact))
        .then(|| Path::new(&artifact.path))
}

/// The name of the marketplace that lists the plugin, when it has one.
pub(crate) fn marketplace_of(artifact: &Artifact) -> Option<&str> {
    artifact.metadata.get("marketplace")?.as_str()
}

/// True when the plugin was read from a marketplace (whatever the entry's name is).
pub(crate) fn is_marketplace_entry(artifact: &Artifact) -> bool {
    artifact.metadata.contains_key("marketplace")
}

/// The manifest that named a plugin: a marketplace entry's `marketplace.json`, else the plugin's
/// own `plugin.json` (an installed plugin is read from its folder's `plugin.json`).
pub(crate) fn manifest_name(artifact: &Artifact) -> &'static str {
    if is_marketplace_entry(artifact) || artifact.path.contains("marketplace.json") {
        "marketplace.json"
    } else {
        "plugin.json"
    }
}

/// The plugin in a `plugin:name` namespaced skill, command or agent name.
pub(crate) fn namespaced_plugin(name: &str) -> Option<&str> {
    name.split_once(':')
        .map(|(plugin, _)| plugin)
        .filter(|p| !p.is_empty())
}

/// The `name@marketplace` keys a settings file's `enabledPlugins` lists, enabled or not.
pub(crate) fn enabled_keys(settings: &serde_json::Map<String, serde_json::Value>) -> Vec<&str> {
    match settings.get("enabledPlugins") {
        Some(serde_json::Value::Object(plugins)) => plugins.keys().map(String::as_str).collect(),
        _ => Vec::new(),
    }
}

/// The settings keys a profile's plugins add to a project's `settings.json`, which Harneo merges
/// instead of owning.
pub(crate) const MERGED_SETTINGS_KEYS: [&str; 2] = ["enabledPlugins", "extraKnownMarketplaces"];

/// Harnfit fits one skill, command or agent at a time: a plugin is refused.
pub(crate) fn ensure_fits_harnfit(kind: ArtifactKind) -> crate::error::Result<()> {
    if kind == ArtifactKind::Plugin {
        return Err(crate::error::Error::HarnfitPluginUnsupported);
    }
    Ok(())
}

/// The manifest field, or the file or folder Claude Code reads by default, makes the part present.
fn declared(plugin: &Artifact, field: &str) -> bool {
    plugin.metadata.contains_key(field)
        || plugin
            .metadata
            .get("experimental")
            .is_some_and(|e| e.get(field).is_some())
}

/// True when the plugin's scoped files hold `rel` or anything below it.
fn has_files(plugin: &Artifact, rel: &str) -> bool {
    let base = Path::new(&plugin.path).join(rel);
    plugin.files.iter().any(|f| Path::new(f).starts_with(&base))
}

/// The parts that only work inside a plugin, which `plugin` holds.
fn whole_only_parts(walker: &mut Walker, plugin: &Artifact) -> Vec<PluginPart> {
    let present = [
        (PluginPart::Hooks, "hooks", "hooks/hooks.json"),
        (PluginPart::Monitors, "monitors", "monitors/monitors.json"),
        (PluginPart::LspServers, "lspServers", ".lsp.json"),
        (PluginPart::Bin, "bin", "bin"),
        (PluginPart::Themes, "themes", "themes"),
        (PluginPart::Workflows, "workflows", "workflows"),
        (PluginPart::Settings, "settings", "settings.json"),
    ];
    let mut out: Vec<PluginPart> = present
        .into_iter()
        .filter(|(_, field, default)| declared(plugin, field) || has_files(plugin, default))
        .map(|(part, _, _)| part)
        .collect();
    // A mod is a hooks module: Claude Code reads it from `hooks/hooks.json`'s `modules` list
    // (`{ "modules": ["./register.tsx"] }`), next to or instead of ordinary hooks.
    let hooks_json = Path::new(&plugin.path).join("hooks/hooks.json");
    let json = walker
        .read_text(&hooks_json, &mut Vec::new())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    if json.as_ref().is_some_and(|j| j.get("modules").is_some()) {
        out.push(PluginPart::Mods);
        // A `hooks.json` that only lists modules holds mods, not ordinary hooks.
        let ordinary =
            declared(plugin, "hooks") || json.as_ref().is_some_and(|j| j.get("hooks").is_some());
        if !ordinary {
            out.retain(|p| *p != PluginPart::Hooks);
        }
    }
    out.sort();
    out
}

/// What `plugin` (a Plugin read from a local folder of `walker`'s scan root) holds. A remote-only
/// entry holds nothing we can see.
pub(crate) fn contents(walker: &mut Walker, plugin: &Artifact) -> PluginContents {
    if plugin.kind != ArtifactKind::Plugin || is_remote_only(plugin) {
        return PluginContents::default();
    }
    let mut pickable = Vec::new();
    let mut root_ties = Vec::new();
    for part in unpack_plugin(walker, plugin, &mut Vec::new()) {
        let Some(kind) = PluginPart::of_kind(part.kind) else {
            continue;
        };
        pickable.push(kind);
        let tied = part
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.code == "plugin.part-needs-plugin");
        if tied {
            root_ties.push(kind);
        }
    }
    for list in [&mut pickable, &mut root_ties] {
        list.sort();
        list.dedup();
    }
    PluginContents {
        pickable,
        whole_only: whole_only_parts(walker, plugin),
        root_ties,
    }
}

/// Why `plugin` can only be taken whole, or `None` when it can be unpacked.
pub(crate) fn must_stay_whole(walker: &mut Walker, plugin: &Artifact) -> Option<WholeReason> {
    if plugin.kind != ArtifactKind::Plugin {
        return None;
    }
    if is_remote_only(plugin) {
        return Some(WholeReason::RemoteOnly);
    }
    let held = contents(walker, plugin);
    if !held.whole_only.is_empty() {
        return Some(WholeReason::WholeOnlyParts {
            parts: held.whole_only,
        });
    }
    if !held.root_ties.is_empty() {
        return Some(WholeReason::PluginRootTie {
            parts: held.root_ties,
        });
    }
    None
}

#[cfg(test)]
mod tests;
