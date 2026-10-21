//! Which folder shapes a scan recognises, in one place: project, `~/.claude`-like and pack
//! (skills under `skills/` and `.claude/skills/`, handled by the kinds themselves), plugin repo,
//! marketplace repo and loose skills.
//!
//! A plugin is kept whole: nothing inside a plugin folder is reported as another kind. This runs
//! after every kind's own scan, so it can drop whatever they found inside a plugin.

use std::path::{Path, PathBuf};

use super::kinds::{plugin, skill};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, ArtifactKind, ScanOptions, ScanReport};

#[derive(Default)]
struct Found {
    marketplaces: Vec<PathBuf>,
    plugins: Vec<PathBuf>,
    skills: Vec<PathBuf>,
}

/// Looks `max_depth` levels down from `dir` for plugin folders, marketplaces and skill folders.
/// A plugin or a skill folder is never entered.
fn discover(walker: &Walker, dir: &Path, depth: u32, max_depth: u32, found: &mut Found) {
    if walker.exists(&plugin::marketplace_path(dir)) {
        found.marketplaces.push(dir.to_path_buf());
    }
    if walker.exists(&plugin::manifest_path(dir)) {
        found.plugins.push(dir.to_path_buf());
        return;
    }
    if walker.exists(&dir.join("SKILL.md")) {
        found.skills.push(dir.to_path_buf());
        return;
    }
    if depth < max_depth {
        for sub in walker.list_dirs(dir) {
            discover(walker, &sub, depth + 1, max_depth, found);
        }
    }
}

/// Adds plugins, marketplace entries and loose skills, then removes everything else that lies
/// inside a plugin folder. Call after the other kinds' scans.
pub(crate) fn scan(walker: &mut Walker, options: &ScanOptions, report: &mut ScanReport) {
    let mut found = Found::default();
    discover(
        walker,
        Path::new(""),
        0,
        options.max_loose_skill_depth,
        &mut found,
    );

    let mut claimed = Vec::new();
    for folder in &found.marketplaces {
        plugin::scan_marketplace(walker, folder, &mut claimed, report);
    }
    for folder in found.plugins.iter().filter(|f| !claimed.contains(f)) {
        plugin::scan_plugin(walker, folder, report);
    }
    for folder in &found.skills {
        let path = rel_string(folder);
        let known = report
            .artifacts
            .iter()
            .any(|a| a.kind == ArtifactKind::Skill && a.path == path);
        if !known {
            skill::read_skill(walker, folder, report);
        }
    }

    let plugin_paths: Vec<PathBuf> = report
        .artifacts
        .iter()
        .filter(|a| a.kind == ArtifactKind::Plugin && !a.path.contains('#'))
        .map(|a| PathBuf::from(&a.path))
        .collect();
    report.artifacts.retain(|a| {
        a.kind == ArtifactKind::Plugin
            || !plugin_paths
                .iter()
                .any(|p| Path::new(&a.path).starts_with(p))
    });
}
