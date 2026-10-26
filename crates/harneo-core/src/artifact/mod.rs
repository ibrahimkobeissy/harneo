//! What an AI Artifact is, agent-neutral. An `AgentAdapter` turns a folder into these types;
//! nothing outside `artifact/claude/` knows Claude Code's folder names or file formats.

pub mod adapter;
pub mod claude;
pub mod digest;
pub mod frontmatter;
pub mod literal_secret;
pub mod walk;

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use adapter::AgentAdapter;
pub use claude::ClaudeAdapter;
pub(crate) use claude::{check_path, github_repo, parse_proposed};

/// The 10 artifact types of `CONTEXT.md` §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ArtifactKind {
    ClaudeMd,
    Rule,
    Skill,
    Command,
    Agent,
    Settings,
    Mcp,
    OutputStyle,
    Workflow,
    Plugin,
}

/// How serious a finding is. Nothing fails a scan; the severity is for the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    /// Worth knowing, nothing to fix (for example folders a scan chose not to enter).
    Info,
}

/// One finding. The `code` (`<kind>.<kebab-problem>`) is stable once released, because the audit
/// and the UI key on it; the message is for people and may change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    /// Relative to the scan root, with forward slashes.
    pub file: String,
}

impl Diagnostic {
    fn new(severity: Severity, code: &str, file: &Path, message: &str) -> Self {
        Self {
            severity,
            code: code.to_owned(),
            message: message.to_owned(),
            file: rel_string(file),
        }
    }

    /// A problem that makes the artifact unusable or wrong.
    pub fn error(code: &str, file: &Path, message: &str) -> Self {
        Self::new(Severity::Error, code, file, message)
    }

    /// A note that asks for no action.
    pub fn info(code: &str, file: &Path, message: &str) -> Self {
        Self::new(Severity::Info, code, file, message)
    }

    /// A problem worth fixing that does not stop the artifact from working.
    pub fn warning(code: &str, file: &Path, message: &str) -> Self {
        Self::new(Severity::Warning, code, file, message)
    }
}

/// A dependency an artifact declares in a structured field (never free text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Reference {
    /// The type of the thing referred to.
    pub kind: ArtifactKind,
    /// The name or relative path as written in the artifact.
    pub target: String,
    /// True when the target was found in the same scan.
    pub resolved: bool,
}

/// One artifact found in a folder, ready for the library, the audit and bundles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub name: String,
    pub description: Option<String>,
    /// The artifact's own file or folder, relative to the scan root, with forward slashes.
    pub path: String,
    /// Everything that travels with the artifact, relative to the scan root, sorted.
    pub files: Vec<String>,
    /// The kind-specific fields (frontmatter or JSON keys) other than `name` and `description`.
    #[ts(type = "Record<string, unknown>")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub references: Vec<Reference>,
    pub diagnostics: Vec<Diagnostic>,
    /// SHA-256 over the artifact's files; filled in by the digest step, `None` until then.
    pub digest: Option<String>,
    /// Where the artifact came from; `own` for plain files.
    #[serde(default)]
    #[ts(optional = false)]
    pub origin: Origin,
}

/// Where an artifact came from, so the library can tell your own files from ones Claude Code
/// or a marketplace put there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Origin {
    /// Plain files you or your project own.
    #[default]
    Own,
    /// A skill Claude synced from your account (`skills/synced/`).
    ClaudeSynced,
    /// A plugin installed from a marketplace. `official` is true for repos under `anthropics/`.
    Marketplace {
        name: String,
        repo: Option<String>,
        official: bool,
    },
    /// A mod in development (`dev-mods/`).
    DevMod,
}

/// Everything one scan found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScanReport {
    pub artifacts: Vec<Artifact>,
    /// Findings that belong to no single artifact (unreadable folders, oversized files...).
    pub diagnostics: Vec<Diagnostic>,
    /// Paths of files that might be artifacts but lie outside every path a scan claimed,
    /// sorted and capped at [`UNPLACED_CAP`]; the rest is counted in `unplaced_overflow`.
    #[serde(default)]
    #[ts(optional = false)]
    pub unplaced: Vec<String>,
    /// How many more unplaced paths there were beyond the cap.
    #[serde(default)]
    #[ts(optional = false)]
    pub unplaced_overflow: u32,
}

/// The most unplaced paths one report lists.
pub const UNPLACED_CAP: usize = 200;

/// The limits that keep a scan of an untrusted folder cheap and safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScanOptions {
    /// Larger files are counted but not parsed.
    pub max_file_bytes: usize,
    /// Total bytes read in one scan.
    pub max_total_bytes: usize,
    /// How deep loose skills are looked for.
    pub max_loose_skill_depth: u32,
    /// Folder names never entered.
    pub skip_dirs: Vec<String>,
    /// Also enter Claude-managed folders (`skills/synced/`, `dev-mods/`), skipped by default.
    pub include_managed: bool,
    /// The GitHub `owner/repo` being scanned, when known (a git import). A marketplace entry
    /// whose source names this repo is the plugin folder itself, not a separate remote plugin.
    #[serde(default)]
    pub repo: Option<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_file_bytes: 1024 * 1024,
            max_total_bytes: 100 * 1024 * 1024,
            max_loose_skill_depth: 4,
            skip_dirs: vec![".git".into(), "node_modules".into(), "target".into()],
            include_managed: false,
            repo: None,
        }
    }
}

/// A path as it appears in reports: forward slashes on every platform, so output is stable.
pub fn rel_string(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let o = ScanOptions::default();
        assert_eq!(o.max_file_bytes, 1_048_576);
        assert_eq!(o.max_total_bytes, 104_857_600);
        assert_eq!(o.max_loose_skill_depth, 4);
        assert_eq!(o.skip_dirs, [".git", "node_modules", "target"]);
        assert!(!o.include_managed);
    }

    #[test]
    fn rel_string_uses_forward_slashes() {
        let p: PathBuf = ["skills", "a", "SKILL.md"].iter().collect();
        assert_eq!(rel_string(&p), "skills/a/SKILL.md");
    }

    #[test]
    fn diagnostics_carry_severity_and_file() {
        let e = Diagnostic::error("skill.x", Path::new("a/b"), "m");
        let w = Diagnostic::warning("skill.y", Path::new("a/b"), "m");
        assert_eq!((e.severity, e.file.as_str()), (Severity::Error, "a/b"));
        assert_eq!(w.severity, Severity::Warning);
    }

    #[test]
    fn serialises_camel_case() -> Result<(), serde_json::Error> {
        let v = serde_json::to_value(ArtifactKind::OutputStyle)?;
        assert_eq!(v, "outputStyle");
        let r = serde_json::to_value(ScanReport::default())?;
        assert_eq!(r["artifacts"].as_array().map(Vec::len), Some(0));
        Ok(())
    }
}
