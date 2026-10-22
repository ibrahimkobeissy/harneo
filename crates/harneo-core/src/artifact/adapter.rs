use std::path::{Path, PathBuf};

use super::{Artifact, ScanOptions, ScanReport};
use crate::audit::Audit;
use crate::Result;

/// One coding agent's knowledge of where its artifacts live and what they look like.
/// The rest of Harneo only sees `Artifact`s, so another agent is a new adapter, not a rewrite.
pub trait AgentAdapter {
    fn id(&self) -> &'static str;
    /// Reads a folder without running anything in it. Problems become diagnostics, not errors;
    /// `Err` means the scan could not run at all (the folder is missing).
    fn scan(&self, root: &Path, options: &ScanOptions) -> Result<ScanReport>;
    /// Audits one artifact of a scan of `root`: reads its files within the limits and runs every
    /// audit rule. Static; nothing is executed.
    fn audit(&self, artifact: &Artifact, root: &Path, options: &ScanOptions) -> Audit;
    /// Where this artifact goes in a project, relative to the project root.
    fn install_path(&self, artifact: &Artifact) -> PathBuf;
}
