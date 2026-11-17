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

pub(crate) fn load(conn: &Connection, id: &str) -> Result<Option<Artifact>> {
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

/// True when `path` (relative to the project, forward slashes) is somewhere a harness may
/// write: under `.claude/`, the `scripts/` folder, or one of the files Harneo knows by name.
/// Everything else (`.envrc`, `package.json`, `.husky/`, `.github/workflows/`, `.vscode/`,
/// `Makefile`...) runs or configures tooling outside Claude Code, so an artifact never gets
/// to write it.
pub(crate) fn in_harness(path: &str) -> bool {
    let normal = |p: &str| {
        !p.split('/')
            .any(|c| c.is_empty() || c == "." || c == ".." || c.contains('\\'))
    };
    normal(path)
        && (matches!(path, "CLAUDE.md" | ".mcp.json" | "HARNESS.md")
            || path.starts_with(".claude/")
            || path.starts_with("scripts/"))
}

/// True for a stored path that holds a linked file (ADR-007).
pub(crate) fn is_linked(stored: &str) -> bool {
    stored.starts_with(concat!("_linked", "/"))
}

/// Every file of one artifact as `(path in its library copy, project path)`. A file that would
/// land outside the harness (see [`in_harness`]) refuses the whole artifact.
pub(crate) fn placed(artifact: &Artifact) -> Result<Vec<(String, String)>> {
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
        let project = match artifact.kind {
            _ if is_linked(stored) => crate::artifact::claude::linked::project_path(
                artifact, source,
            )
            .ok_or_else(|| crate::error::Error::UnsafePath {
                path: source.to_owned(),
            })?,
            ArtifactKind::Skill => rel_string(&install.join(stored)),
            _ if source == artifact.path => rel_string(&install),
            // Scripts a Settings file or an MCP server brings keep their project-relative path.
            ArtifactKind::Settings | ArtifactKind::Mcp => source.to_owned(),
            _ => remap(source, own_dir, install_dir),
        };
        if !in_harness(&project) {
            return Err(crate::error::Error::UnsafePath { path: project });
        }
        out.push((stored.clone(), project));
    }
    Ok(out)
}

pub(super) fn layout(
    conn: &Connection,
    artifacts_dir: &Path,
    spec: &ProfileSpec,
) -> Result<Layout> {
    let resolved = super::resolve::resolve(conn, spec)?;
    let mut claims: BTreeMap<String, Vec<PathFrom>> = BTreeMap::new();
    // The bytes of each linked file a path is claimed with; claimants of one path that all carry
    // the same bytes share it (ADR-007) instead of conflicting.
    let mut linked: BTreeMap<String, Vec<Option<Vec<u8>>>> = BTreeMap::new();
    let mut claim = |path: String, from: PathFrom, bytes: Option<Vec<u8>>| {
        claims.entry(path.clone()).or_default().push(from);
        linked.entry(path).or_default().push(bytes);
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
        for (stored, path) in placed(&artifact)? {
            let bytes = if is_linked(&stored) {
                let file = crate::library::safe_join(&artifacts_dir.join(&item.id), &stored)?;
                Some(std::fs::read(&file).map_err(|e| crate::error::Error::Io {
                    path: file.display().to_string(),
                    source: e,
                })?)
            } else {
                None
            };
            // The shared `.mcp.json` is generated, never a server's own file.
            if item.kind == ArtifactKind::Mcp && path == MCP_JSON {
                continue;
            }
            claim(
                path,
                PathFrom::Artifact {
                    artifact: item.id.clone(),
                },
                bytes,
            );
        }
    }
    if plugins && template.is_none() {
        claim(
            SETTINGS.to_owned(),
            PathFrom::Generated {
                generated: Generated::Settings,
            },
            None,
        );
    }
    if servers {
        claim(
            MCP_JSON.to_owned(),
            PathFrom::Generated {
                generated: Generated::McpJson,
            },
            None,
        );
    }
    claim(
        HARNESS_MD.to_owned(),
        PathFrom::Generated {
            generated: Generated::HarnessMd,
        },
        None,
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
        let shared = linked.get(&path).is_some_and(|all| {
            all.iter().all(Option::is_some) && all.windows(2).all(|w| w[0] == w[1])
        });
        if owners.len() > 1 && !shared {
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
        layout(&self.store.conn, &self.store.artifacts_dir(), spec)
    }
}
