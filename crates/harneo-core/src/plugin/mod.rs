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

/// True when the plugin is a marketplace entry pointing to another repo.
fn is_remote_only(plugin: &Artifact) -> bool {
    plugin.path.contains('#')
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
fn whole_only_parts(walker: &Walker, plugin: &Artifact) -> Vec<PluginPart> {
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
    // Mods are outside the plugin's scoped files, so their folder is looked for on disk.
    if declared(plugin, "mods") || walker.is_dir(&Path::new(&plugin.path).join("mods")) {
        out.push(PluginPart::Mods);
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
