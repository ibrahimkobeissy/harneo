//! Which folder shapes a scan recognises, in one place: project, `~/.claude`-like and pack
//! (skills under `skills/` and `.claude/skills/`, handled by the kinds themselves), plugin repo,
//! marketplace repo and loose skills.
//!
//! A plugin is kept whole: nothing inside a plugin folder is reported as another kind. This runs
//! after every kind's own scan, so it can drop whatever they found inside a plugin.

use std::path::{Path, PathBuf};

use super::kinds::{plugin, skill};
use crate::artifact::walk::{Walker, MANAGED_DIRS};
use crate::artifact::{rel_string, ArtifactKind, Diagnostic, Origin, ScanOptions, ScanReport};

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

/// Counts what a skipped managed folder holds: folders with a `SKILL.md` or a plugin manifest,
/// found by listing directories and checking names only (nothing is opened).
fn count_units(walker: &Walker, dir: &Path, depth: u32) -> usize {
    walker
        .list_dirs(dir)
        .iter()
        .map(|sub| {
            if walker.exists(&sub.join("SKILL.md")) || walker.exists(&plugin::manifest_path(sub)) {
                1
            } else if depth < 4 {
                count_units(walker, sub, depth + 1)
            } else {
                0
            }
        })
        .sum()
}

/// One `scan.managed-skipped` info per managed folder that exists and was not entered.
fn report_managed(walker: &Walker, report: &mut ScanReport) {
    for dir in MANAGED_DIRS {
        let dir = Path::new(dir);
        if !walker.is_dir(dir) {
            continue;
        }
        let n = count_units(walker, dir, 0);
        let what = if dir.ends_with("dev-mods") {
            "mod"
        } else {
            "skill"
        };
        let plural = if n == 1 { "" } else { "s" };
        report.diagnostics.push(Diagnostic::info(
            "scan.managed-skipped",
            dir,
            &format!(
                "Claude-managed folder skipped: {n} {what} folder{plural} not scanned (include managed folders to scan them)"
            ),
        ));
    }
}

/// Adds plugins, marketplace entries and loose skills, then removes everything else that lies
/// inside a plugin folder. Call after the other kinds' scans.
pub(crate) fn scan(walker: &mut Walker, options: &ScanOptions, report: &mut ScanReport) {
    if !options.include_managed {
        report_managed(walker, report);
    }
    let mut found = Found::default();
    discover(
        walker,
        Path::new(""),
        0,
        options.max_loose_skill_depth,
        &mut found,
    );

    super::installed::scan(walker, report);

    let mut claimed = Vec::new();
    for folder in &found.marketplaces {
        plugin::scan_marketplace(
            walker,
            folder,
            options.repo.as_deref(),
            &mut claimed,
            report,
        );
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

/// Marks what lies in a Claude-managed folder (only present when those were included).
pub(crate) fn mark_managed_origins(report: &mut ScanReport) {
    for a in &mut report.artifacts {
        let path = Path::new(&a.path);
        if path.starts_with("skills/synced") || path.starts_with(".claude/skills/synced") {
            a.origin = Origin::ClaudeSynced;
        } else if path.starts_with("dev-mods") || path.starts_with(".claude/dev-mods") {
            a.origin = Origin::DevMod;
        }
    }
}
