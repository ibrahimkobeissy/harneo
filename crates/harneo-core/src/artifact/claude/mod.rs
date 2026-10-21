//! Claude Code's side of the artifact model: where its artifacts live and how to read them.
//! Each artifact type is one file in `kinds/` and one line in `scan`.

mod kinds;
mod layout;

use std::path::{Path, PathBuf};

use super::walk::Walker;
use super::{AgentAdapter, Artifact, ArtifactKind, ScanOptions, ScanReport};
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
        super::digest::fill(&mut walker, &mut report);
        layout::scan(&mut walker, options, &mut report);
        report.diagnostics.extend(walker.finish());
        report.artifacts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(report)
    }

    fn install_path(&self, artifact: &Artifact) -> PathBuf {
        match artifact.kind {
            ArtifactKind::Skill => kinds::skill::install_path(artifact),
            // The other types are completed with the install-path ticket; until then they
            // stay where they were found.
            _ => PathBuf::from(&artifact.path),
        }
    }
}
