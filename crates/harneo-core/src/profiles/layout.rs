//! The project paths a profile produces, and who claims each. Content is `install-engine`'s job.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ProfileSpec;
use crate::artifact::{rel_string, AgentAdapter, Artifact, ArtifactKind, ClaudeAdapter};
use crate::error::Result;
use crate::library::{stored_view, ArtifactId, Library};

/// A file Harneo writes itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Generated {
    /// `HARNESS.md`: what is installed and why.
    HarnessMd,
    /// `.mcp.json`, built from the profile's servers.
    McpJson,
    /// `.claude/settings.json` when there is no template: only the plugins' keys.
    Settings,
}

/// Where a planned file comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum PathFrom {
    /// A file of this library artifact (for `.claude/settings.json`, the template, which also
    /// receives the plugins' keys).
    Artifact {
        artifact: ArtifactId,
    },
    Generated {
        generated: Generated,
    },
}

/// One project path the profile produces, relative to the project root with forward slashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PlannedPath {
    pub path: String,
    pub from: PathFrom,
}

/// Two or more artifacts that put a file at the same path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct LayoutConflict {
    pub path: String,
    /// Every claimant, in layout order.
    pub artifacts: Vec<ArtifactId>,
}

/// What a profile writes into a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Layout {
    /// Sorted by path; a conflicted path appears once, under its first claimant.
    pub paths: Vec<PlannedPath>,
    pub conflicts: Vec<LayoutConflict>,
}

const SETTINGS: &str = ".claude/settings.json";
const MCP_JSON: &str = ".mcp.json";
const HARNESS_MD: &str = "HARNESS.md";

fn load(conn: &Connection, id: &str) -> Result<Option<Artifact>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT artifact FROM artifacts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
}

/// `rel` as a project path: `install_dir` (the install path's folder) replaces `source_dir`
/// when the file sits under it, otherwise the source path is kept as it is.
fn remap(rel: &str, source_dir: &str, install_dir: &Path) -> String {
    let under = if source_dir.is_empty() {
        Some(rel)
    } else {
        rel.strip_prefix(source_dir)
            .and_then(|r| r.strip_prefix('/'))
    };
    match under {
        Some(rest) if !install_dir.as_os_str().is_empty() => rel_string(&install_dir.join(rest)),
        _ => rel.to_owned(),
    }
}

/// Every file of one artifact, as project paths.
fn files_of(artifact: &Artifact) -> Result<Vec<String>> {
    let install = ClaudeAdapter.install_path(artifact);
    let (view, back) = stored_view(artifact)?;
    let own_dir = artifact
        .path
        .split('#')
        .next()
        .unwrap_or_default()
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let install_dir = install.parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();
    for stored in &view.files {
        let source = back.get(stored).map_or(stored.as_str(), String::as_str);
        out.push(match artifact.kind {
            ArtifactKind::Skill => rel_string(&install.join(stored)),
            _ if source == artifact.path => rel_string(&install),
            // Scripts a Settings file or an MCP server brings keep their project-relative path.
            ArtifactKind::Settings | ArtifactKind::Mcp => source.to_owned(),
            _ => remap(source, own_dir, install_dir),
        });
    }
    Ok(out)
}

pub(super) fn layout(conn: &Connection, spec: &ProfileSpec) -> Result<Layout> {
    let resolved = super::resolve::resolve(conn, spec)?;
    let mut claims: BTreeMap<String, Vec<PathFrom>> = BTreeMap::new();
    let mut claim = |path: String, from: PathFrom| {
        claims.entry(path).or_default().push(from);
    };
    let mut plugins = false;
    let mut servers = false;
    let mut template = None;
    for item in &resolved.artifacts {
        match item.kind {
            ArtifactKind::Plugin => {
                // A plugin only adds `enabledPlugins` to settings.json.
                plugins = true;
                continue;
            }
            ArtifactKind::Mcp => servers = true,
            ArtifactKind::Settings => template = Some(item.id.clone()),
            _ => {}
        }
        let Some(artifact) = load(conn, &item.id)? else {
            continue;
        };
        for path in files_of(&artifact)? {
            // The shared `.mcp.json` is generated, never a server's own file.
            if item.kind == ArtifactKind::Mcp && path == MCP_JSON {
                continue;
            }
            claim(
                path,
                PathFrom::Artifact {
                    artifact: item.id.clone(),
                },
            );
        }
    }
    if plugins && template.is_none() {
        claim(
            SETTINGS.to_owned(),
            PathFrom::Generated {
                generated: Generated::Settings,
            },
        );
    }
    if servers {
        claim(
            MCP_JSON.to_owned(),
            PathFrom::Generated {
                generated: Generated::McpJson,
            },
        );
    }
    claim(
        HARNESS_MD.to_owned(),
        PathFrom::Generated {
            generated: Generated::HarnessMd,
        },
    );
    let mut out = Layout {
        paths: Vec::new(),
        conflicts: Vec::new(),
    };
    for (path, from) in claims {
        let mut owners: Vec<ArtifactId> = Vec::new();
        for f in &from {
            if let PathFrom::Artifact { artifact } = f {
                if !owners.contains(artifact) {
                    owners.push(artifact.clone());
                }
            }
        }
        if owners.len() > 1 {
            out.conflicts.push(LayoutConflict {
                path: path.clone(),
                artifacts: owners,
            });
        }
        if let Some(first) = from.into_iter().next() {
            out.paths.push(PlannedPath { path, from: first });
        }
    }
    Ok(out)
}

impl Library {
    /// Every project path `spec` produces (bundles included) with where it comes from, and the
    /// paths two artifacts both claim.
    pub fn layout(&self, spec: &ProfileSpec) -> Result<Layout> {
        layout(&self.store.conn, spec)
    }
}
