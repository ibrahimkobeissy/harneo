//! The files of a plugin taken whole (ADR-005, the hybrid rule).
//!
//! - **A plugin in its own folder** (`plugins/security-guardrails/`) keeps its whole folder, as
//!   Claude Code installs it: scripts reach their data files through paths built at run time
//!   (`hooks/rules.md`, `templates/settings.json`), so no allowlist can see them.
//! - **A plugin at the scan root** (a marketplace entry `"source": "./"`, `mattpocock/skills`)
//!   would take the repo's plumbing that way (docs, CI, changesets). It keeps the component scope
//!   ([`super::plugin_scope`]), with Claude Code's default component folders kept whole, plus the
//!   top-level entries its own code files name as a path segment (`"templates"`).
//!
//! Left out in both cases: folders that are plugins of their own (a `.claude-plugin/plugin.json`
//! or `marketplace.json` below the plugin's folder, or a local source its own marketplace lists),
//! and the folders the scanner skips everywhere (`ScanOptions::skip_dirs`, honoured by the walk).
//! Large files, symlinks and secrets keep the walk's rules: listed, reported, never read.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::plugin::{manifest_path, marketplace_path, parse_marketplace};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::Diagnostic;

/// Claude Code's default component folders: kept whole for a plugin at the scan root.
const COMPONENT_DIRS: [&str; 10] = [
    ".claude-plugin",
    "skills",
    "commands",
    "agents",
    "hooks",
    "output-styles",
    "workflows",
    "themes",
    "monitors",
    "bin",
];

/// Extensions of the code files searched for the names of top-level entries.
const CODE_EXTENSIONS: [&str; 10] = [
    "py", "sh", "bash", "js", "mjs", "cjs", "ts", "rb", "json", "toml",
];

/// The files of the plugin in `folder`. `scoped` is its component scope; it is all that a plugin
/// at the scan root keeps, widened as the module doc says. Nested plugins are always left out.
/// The walk's findings about the kept files are added to `diagnostics`.
pub(super) fn folder_files(
    walker: &mut Walker,
    folder: &Path,
    scoped: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let mut walk = Vec::new();
    let all = walker.list_files(folder, &mut walk);
    let nested = nested_plugins(walker, folder, &all);
    let at_root = folder.as_os_str().is_empty();
    let wanted = if at_root {
        root_entries(walker, folder, scoped, &all)
    } else {
        BTreeSet::new()
    };
    let kept: Vec<String> = all
        .into_iter()
        .filter(|f| !nested.iter().any(|n| Path::new(f).starts_with(n)))
        .filter(|f| {
            !at_root
                || scoped.contains(f)
                || top_entry(folder, f).is_some_and(|e| wanted.contains(&e))
        })
        .collect();
    for d in walk {
        if kept.contains(&d.file) && !diagnostics.contains(&d) {
            diagnostics.push(d);
        }
    }
    kept
}

/// The folders below `folder` that are plugins of their own.
fn nested_plugins(walker: &mut Walker, folder: &Path, all: &[String]) -> BTreeSet<PathBuf> {
    let mut nested = BTreeSet::new();
    for file in all {
        let path = Path::new(file);
        let in_claude_dir =
            path.parent().and_then(Path::file_name) == Some(".claude-plugin".as_ref());
        let manifest = path
            .file_name()
            .is_some_and(|n| n == "plugin.json" || n == "marketplace.json");
        if let (true, true, Some(root)) = (
            in_claude_dir,
            manifest,
            path.parent().and_then(Path::parent),
        ) {
            if root != folder {
                nested.insert(root.to_path_buf());
            }
        }
    }
    let market = marketplace_path(folder);
    if all.iter().any(|f| Path::new(f) == market) {
        if let Some(text) = walker.read_text(&market, &mut Vec::new()) {
            let entries = parse_marketplace(&market, &text).map(|(_, e)| e);
            for entry in entries.unwrap_or_default() {
                let Some(source) = entry.get("source").and_then(|s| s.as_str()) else {
                    continue;
                };
                if let Ok(target) = resolve_reference(folder, source) {
                    if target != folder && target.starts_with(folder) {
                        nested.insert(target);
                    }
                }
            }
        }
    }
    nested.remove(&manifest_path(folder));
    nested
}

/// The first component of `file` below `folder`, as a string.
fn top_entry(folder: &Path, file: &str) -> Option<String> {
    Path::new(file)
        .strip_prefix(folder)
        .ok()?
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
}

/// The top-level entries a plugin at the scan root keeps whole: the default component folders
/// its scope reaches, and every entry its scoped code files name as a path segment (between
/// quotes or slashes: `"templates"`, `ROOT/templates/x`).
fn root_entries(
    walker: &mut Walker,
    folder: &Path,
    scoped: &[String],
    all: &[String],
) -> BTreeSet<String> {
    let mut wanted: BTreeSet<String> = scoped
        .iter()
        .filter_map(|f| top_entry(folder, f))
        .filter(|e| COMPONENT_DIRS.contains(&e.as_str()))
        .collect();
    let entries: BTreeSet<String> = all.iter().filter_map(|f| top_entry(folder, f)).collect();
    let code = scoped.iter().filter(|f| {
        Path::new(f)
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| CODE_EXTENSIONS.contains(&x))
    });
    for file in code {
        let Some(text) = walker.read_text(Path::new(file), &mut Vec::new()) else {
            continue;
        };
        for entry in &entries {
            if !wanted.contains(entry) && names_segment(&text, entry) {
                wanted.insert(entry.clone());
            }
        }
    }
    wanted
}

/// Whether `text` holds `name` as a whole path segment: between quotes or slashes.
fn names_segment(text: &str, name: &str) -> bool {
    let edge = |c: Option<char>| matches!(c, Some('"' | '\'' | '/' | '\\' | '`'));
    text.match_indices(name).any(|(at, _)| {
        edge(text[..at].chars().next_back()) && edge(text[at + name.len()..].chars().next())
    })
}
