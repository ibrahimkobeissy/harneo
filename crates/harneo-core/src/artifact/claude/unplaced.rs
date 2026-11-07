//! Tier 2's hand-off to tier 3: lists the files that might be artifacts but lie outside every
//! path the earlier tiers claimed. Paths only; nothing here places or imports anything.

use std::collections::HashSet;
use std::path::Path;

use crate::artifact::frontmatter::{self, Frontmatter};
use crate::artifact::walk::{Walker, MANAGED_DIRS};
use crate::artifact::{ArtifactKind, ScanOptions, ScanReport, UNPLACED_CAP};

/// Folder names that mark Markdown inside as a likely artifact.
const KIND_DIRS: [&str; 10] = [
    "prompts",
    "prompt",
    "agents",
    "commands",
    "rules",
    "skills",
    "output-styles",
    "outputstyles",
    "workflows",
    "instructions",
];

/// Folders whose contents never count: repo plumbing, documentation and tests.
const IGNORED_DIRS: [&str; 7] = [
    "docs",
    "test",
    "tests",
    "__tests__",
    "__fixtures__",
    "fixtures",
    "e2e",
];

/// File-name prefixes (upper-cased stems) that never count.
const IGNORED_NAMES: [&str; 4] = ["README", "CHANGELOG", "LICENSE", "CONTRIBUTING"];

fn is_markdown(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// True when `parts` (folders then file name) is never counted.
fn is_ignored(parts: &[&str]) -> bool {
    let Some((name, dirs)) = parts.split_last() else {
        return true;
    };
    let upper = name.to_ascii_uppercase();
    // Hidden folders (`.github`, `.changeset`, ...) are never walked, `.agents/skills` aside,
    // which the skill scan has already claimed.
    IGNORED_NAMES.iter().any(|p| upper.starts_with(p))
        || dirs
            .iter()
            .any(|d| d.starts_with('.') || IGNORED_DIRS.contains(&d.to_ascii_lowercase().as_str()))
}

fn in_kind_dir(parts: &[&str]) -> bool {
    parts.split_last().is_some_and(|(_, dirs)| {
        dirs.iter()
            .any(|d| KIND_DIRS.contains(&d.to_ascii_lowercase().as_str()))
    })
}

fn under_skipped_root(walker: &Walker, options: &ScanOptions, path: &str) -> bool {
    (!options.include_managed && MANAGED_DIRS.iter().any(|m| Path::new(path).starts_with(m)))
        || (walker.installed_mode() && path.starts_with("plugins/"))
}

/// Fills `report.unplaced` and `report.unplaced_overflow`.
pub(crate) fn scan(walker: &mut Walker, options: &ScanOptions, report: &mut ScanReport) {
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut folders: Vec<&Path> = Vec::new();
    for a in &report.artifacts {
        claimed.insert(a.path.as_str());
        claimed.extend(a.files.iter().map(String::as_str));
        // A plugin (or a skill) is claimed whole, not just its listed files.
        if let Some(folder) = crate::plugin::local_folder(a) {
            folders.push(folder);
        } else if a.kind == ArtifactKind::Skill && !crate::plugin::is_remote_path(&a.path) {
            folders.push(Path::new(&a.path));
        }
        // A plugin that was unpacked is still one folder: what is not a part is not unplaced.
        // Its folder comes from the parts, so an `unpack` entry naming no plugin claims nothing.
        if let Some(folder) = a
            .metadata
            .get("fromPlugin")
            .and_then(|p| p.get("folder"))
            .and_then(serde_json::Value::as_str)
        {
            folders.push(Path::new(folder));
        }
    }
    let nothing_found = report.artifacts.is_empty();

    let mut all = walker.list_files(Path::new(""), &mut Vec::new());
    all.retain(|p| {
        let parts: Vec<&str> = p.split('/').collect();
        parts.last().is_some_and(|n| is_markdown(n))
            && !is_ignored(&parts)
            && !claimed.contains(p.as_str())
            && !folders.iter().any(|f| Path::new(p).starts_with(f))
            && !under_skipped_root(walker, options, p)
    });
    let mut unplaced = Vec::new();
    for p in all {
        let parts: Vec<&str> = p.split('/').collect();
        if nothing_found || in_kind_dir(&parts) || has_identity(walker, &p) {
            unplaced.push(p);
        }
    }
    unplaced.sort();
    report.unplaced_overflow =
        u32::try_from(unplaced.len().saturating_sub(UNPLACED_CAP)).unwrap_or(u32::MAX);
    unplaced.truncate(UNPLACED_CAP);
    report.unplaced = unplaced;
}

/// True when the file's frontmatter holds a `name` or a `description`.
fn has_identity(walker: &mut Walker, path: &str) -> bool {
    let Some(text) = walker.read_text(Path::new(path), &mut Vec::new()) else {
        return false;
    };
    matches!(frontmatter::parse(&text), Frontmatter::Parsed { data, .. }
        if data.contains_key("name") || data.contains_key("description"))
}
