//! Plugins in one place: what a plugin holds, and whether it stays whole (ADR-008).
//!
//! Any plugin can be split: import offers its parts (skills, agents, commands, output styles),
//! each kept or dropped, and a profile scaffolds the kept ones as a trimmed plugin under
//! `.claude/skills/`. Only a plugin with features such a plugin does not load stays whole:
//! `monitors`, MCP servers (`mcpServers`, `.mcp.json`) and LSP servers (`lspServers`,
//! `.lsp.json`). A remote-only marketplace entry (its files are never fetched) can only be whole
//! too. A whole plugin installs from its marketplace.
//!
//! The reading of a plugin folder stays with the Claude adapter (`artifact::claude`); this module
//! answers the questions on top of it and has no dependency on the library.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::walk::Walker;
use crate::artifact::{Artifact, ArtifactKind};

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
}

/// Why a plugin has to be taken whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum WholeReason {
    /// It holds features a plugin under `.claude/skills/` does not load (monitors, MCP servers,
    /// LSP servers), so it installs from its marketplace.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    WholeOnlyParts { parts: Vec<PluginPart> },
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
                    "Has {}, which a plugin scaffolded under .claude/skills/ does not load, so it stays whole and installs from its marketplace.",
                    join(parts)
                )
            }
            Self::NoParts => {
                "It has no skills, agents, commands or output styles to unpack.".to_owned()
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

/// The features that keep a plugin whole (ADR-008) which `plugin` holds: monitors, MCP servers
/// and LSP servers, declared in the manifest or present at the default place.
fn kept_whole_parts(walker: &Walker, plugin: &Artifact) -> Vec<PluginPart> {
    let present = [
        (PluginPart::McpServers, "mcpServers", ".mcp.json"),
        (PluginPart::Monitors, "monitors", "monitors/monitors.json"),
        (PluginPart::LspServers, "lspServers", ".lsp.json"),
    ];
    let mut out: Vec<PluginPart> = present
        .into_iter()
        .filter(|(_, field, default)| {
            declared(plugin, field) || walker.is_file(&Path::new(&plugin.path).join(default))
        })
        .map(|(part, _, _)| part)
        .collect();
    out.sort();
    out
}

/// Why `plugin` stays whole, or `None` when it can be split into parts (ADR-008). Any plugin can
/// be split except a remote-only entry and one with monitors, MCP servers or LSP servers.
pub(crate) fn kept_whole(walker: &mut Walker, plugin: &Artifact) -> Option<WholeReason> {
    if plugin.kind != ArtifactKind::Plugin {
        return None;
    }
    if is_remote_only(plugin) {
        return Some(WholeReason::RemoteOnly);
    }
    let kept = kept_whole_parts(walker, plugin);
    (!kept.is_empty()).then_some(WholeReason::WholeOnlyParts { parts: kept })
}

#[cfg(test)]
mod tests;
