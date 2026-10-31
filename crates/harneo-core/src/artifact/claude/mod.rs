//! Claude Code's side of the artifact model: where its artifacts live and how to read them.
//! Each artifact type is one file in `kinds/` and one line in `scan`.

mod checks;
mod installed;
mod kinds;

pub(crate) use kinds::plugin::github_repo;
mod layout;
mod proposed;
mod unplaced;

pub(crate) use proposed::{check_path, parse_proposed};

use std::path::{Path, PathBuf};

use super::walk::Walker;
use super::{AgentAdapter, Artifact, ArtifactKind, ScanOptions, ScanReport};
use crate::audit::Audit;
use crate::{Error, Result};

/// The adapter for Claude Code.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn scan(&self, root: &Path, options: &ScanOptions) -> Result<ScanReport> {
        if !root.is_dir() {
            return Err(Error::NotAFolder {
                path: root.display().to_string(),
            });
        }
        let mut report = ScanReport::default();
        let mut walker = Walker::new(root, options);
        kinds::claude_md::scan(&mut walker, &mut report);
        kinds::skill::scan(&mut walker, &mut report);
        kinds::settings::scan(&mut walker, &mut report);
        kinds::rule::scan(&mut walker, &mut report);
        kinds::command::scan(&mut walker, &mut report);
        kinds::agent::scan(&mut walker, &mut report);
        kinds::output_style::scan(&mut walker, &mut report);
        kinds::mcp::scan(&mut walker, &mut report);
        kinds::workflow::scan(&mut walker, &mut report);
        // Keep last: it removes what the kinds found inside a plugin.
        layout::scan(&mut walker, options, &mut report);
        layout::mark_layout_by(&mut report);
        unplaced::scan(&mut walker, options, &mut report);
        layout::drop_duplicate_marketplace_roots(&mut walker, &mut report);
        super::digest::fill(&mut walker, &mut report);
        layout::mark_managed_origins(&mut report);
        checks::run(&mut report);
        report.diagnostics.extend(walker.finish());
        report.artifacts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(report)
    }

    fn audit(&self, artifact: &Artifact, root: &Path, options: &ScanOptions) -> Audit {
        crate::audit::audit(artifact, root, options)
    }

    fn install_path(&self, artifact: &Artifact) -> PathBuf {
        match artifact.kind {
            ArtifactKind::ClaudeMd => kinds::claude_md::install_path(artifact),
            ArtifactKind::Rule => kinds::rule::install_path(artifact),
            ArtifactKind::Skill => kinds::skill::install_path(artifact),
            ArtifactKind::Command => kinds::command::install_path(artifact),
            ArtifactKind::Agent => kinds::agent::install_path(artifact),
            ArtifactKind::Settings => kinds::settings::install_path(artifact),
            ArtifactKind::Mcp => kinds::mcp::install_path(artifact),
            ArtifactKind::OutputStyle => kinds::output_style::install_path(artifact),
            ArtifactKind::Workflow => kinds::workflow::install_path(artifact),
            ArtifactKind::Plugin => kinds::plugin::install_path(artifact),
        }
    }
}
