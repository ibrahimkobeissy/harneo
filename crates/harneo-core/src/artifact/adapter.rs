use std::path::{Path, PathBuf};

use super::{Artifact, ScanOptions, ScanReport};
use crate::Result;

/// One coding agent's knowledge of where its artifacts live and what they look like.
/// The rest of Harneo only sees `Artifact`s, so another agent is a new adapter, not a rewrite.
pub trait AgentAdapter {
    fn id(&self) -> &'static str;
    /// Reads a folder without running anything in it. Problems become diagnostics, not errors;
    /// `Err` means the scan could not run at all (the folder is missing).
    fn scan(&self, root: &Path, options: &ScanOptions) -> Result<ScanReport>;
    /// Where this artifact goes in a project, relative to the project root.
    fn install_path(&self, artifact: &Artifact) -> PathBuf;
}
