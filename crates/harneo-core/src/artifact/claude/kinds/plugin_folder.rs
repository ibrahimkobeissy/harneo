//! The files of a plugin taken whole: its whole folder, as Claude Code installs it (ADR-005).
//!
//! Scripts reach their data files through paths built at run time, so no allowlist can see them.
//! Left out: folders that are plugins of their own (a `.claude-plugin/plugin.json` or
//! `marketplace.json` below the plugin's folder, or a local source its own marketplace lists),
//! and the folders the scanner skips everywhere (`ScanOptions::skip_dirs`, honoured by the walk).
//! Large files, symlinks and secrets keep the walk's rules: listed, reported, never read.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::plugin::{manifest_path, marketplace_path, parse_marketplace};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::Diagnostic;

/// Every listed file of `folder` except the files of nested plugins. The walk's findings about
/// the kept files are added to `diagnostics`.
pub(super) fn folder_files(
    walker: &mut Walker,
    folder: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let mut walk = Vec::new();
    let all = walker.list_files(folder, &mut walk);
    let nested = nested_plugins(walker, folder, &all);
    let kept: Vec<String> = all
        .into_iter()
        .filter(|f| !nested.iter().any(|n| Path::new(f).starts_with(n)))
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
