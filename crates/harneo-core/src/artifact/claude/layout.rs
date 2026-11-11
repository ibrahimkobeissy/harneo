//! Which folder shapes a scan recognises, in one place: project, `~/.claude`-like and pack
//! (skills under `skills/` and `.claude/skills/`, handled by the kinds themselves), plugin repo,
//! marketplace repo and loose skills.
//!
//! A plugin is kept whole: nothing inside a plugin folder is reported as another kind. This runs
//! after every kind's own scan, so it can drop whatever they found inside a plugin.

use std::path::{Path, PathBuf};

use super::kinds::{plugin, plugin_parts, skill};
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
            if dir.as_os_str().is_empty() && is_other_agent_dir(&sub) {
                continue;
            }
            discover(walker, &sub, depth + 1, max_depth, found);
        }
    }
}

/// True for a dot-folder at the scan root other than `.claude` and `.agents`: another agent's
/// layout (`.opencode`, `.cursor`, `.gemini`, ...), which waits for a second adapter (ADR-001).
fn is_other_agent_dir(rel: &Path) -> bool {
    let name = rel.to_string_lossy();
    name.starts_with('.') && !matches!(name.as_ref(), ".claude" | ".agents")
}

/// One `scan.other-agent-folder` info per skipped dot-folder that holds a skill or a plugin.
fn report_other_agents(walker: &Walker, options: &ScanOptions, report: &mut ScanReport) {
    for dir in walker.list_dirs(Path::new("")) {
        if !is_other_agent_dir(&dir) {
            continue;
        }
        let mut found = Found::default();
        discover(walker, &dir, 1, options.max_loose_skill_depth, &mut found);
        if found.skills.is_empty() && found.plugins.is_empty() {
            continue;
        }
        report.diagnostics.push(Diagnostic::info(
            "scan.other-agent-folder",
            &dir,
            "not read: only .claude and .agents are (another agent's folder, or a hidden one)",
        ));
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
    report_other_agents(walker, options, report);
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
        if !known && !skill::is_superseded(walker, folder, report) {
            skill::read_skill(walker, folder, report);
        }
    }

    let plugin_paths: Vec<PathBuf> = report
        .artifacts
        .iter()
        .filter_map(|a| crate::plugin::local_folder(a))
        .map(Path::to_path_buf)
        .collect();
    report.artifacts.retain(|a| {
        a.kind == ArtifactKind::Plugin
            || !plugin_paths
                .iter()
                .any(|p| Path::new(&a.path).starts_with(p))
    });
    if !options.unpack.is_empty() {
        unpack_plugins(walker, options, report);
    }
}

/// A path as `ScanOptions.unpack` may spell it: no `./` prefix, no trailing slash, the scan
/// root itself as an empty string.
fn normal(path: &str) -> &str {
    let path = path.trim().trim_end_matches('/');
    let path = path.strip_prefix("./").unwrap_or(path);
    if path == "." {
        ""
    } else {
        path
    }
}

/// The warning on a plugin asked to unpack that has no pickable parts.
pub(crate) const NO_PARTS: &str = "plugin.no-parts";

/// Replaces each Plugin named in `options.unpack` with its parts. A plugin that must stay whole
/// (ADR-006), or that is only a remote marketplace entry with no files to unpack, stays whole,
/// with a note.
fn unpack_plugins(walker: &mut Walker, options: &ScanOptions, report: &mut ScanReport) {
    let wanted: Vec<&str> = options.unpack.iter().map(|u| normal(u)).collect();
    let mut kept = Vec::new();
    let mut parts = Vec::new();
    for mut a in std::mem::take(&mut report.artifacts) {
        if a.kind != ArtifactKind::Plugin || !wanted.contains(&normal(&a.path)) {
            kept.push(a);
        } else if crate::plugin::is_remote_only(&a) {
            let file = crate::plugin::remote_file(&a.path).to_owned();
            a.diagnostics.push(Diagnostic::warning(
                "plugin.unpack-remote-only",
                Path::new(&file),
                "this marketplace entry points to another repo that is never fetched, so it can only be taken whole",
            ));
            kept.push(a);
        } else if let Some(reason) = crate::plugin::must_stay_whole(walker, &a) {
            // ADR-006: a plugin is unpacked only when every part works as a copy.
            a.diagnostics.push(Diagnostic::warning(
                "plugin.unpack-refused",
                &plugin::manifest_path(Path::new(&a.path)),
                &format!("this plugin can only be taken whole. {}", reason.text()),
            ));
            kept.push(a);
        } else {
            let found = plugin_parts::unpack(walker, &a, &mut report.diagnostics);
            if found.is_empty() {
                // Nothing to pick: the plugin stays whole, so it can still be imported.
                a.diagnostics.push(Diagnostic::warning(
                    NO_PARTS,
                    &plugin::manifest_path(Path::new(&a.path)),
                    "this plugin has no skills, agents, commands, output styles or MCP servers to pick, so it can only be taken whole",
                ));
                kept.push(a);
            } else {
                parts.extend(found);
            }
        }
    }
    kept.extend(parts);
    report.artifacts = kept;
}

/// A `plugin.json` in a marketplace's own folder, named like an entry that marketplace publishes
/// from another folder, is the same plugin listed twice (e.g. the author's working copy at the
/// repo root, the entry pointing at `./plugin`). Only the entry, what Claude Code installs, is
/// kept. Call after `unplaced::scan`: the folder still counts as a plugin folder until then,
/// so its own files are neither listed as artifacts nor reported as unplaced.
pub(crate) fn drop_duplicate_marketplace_roots(walker: &mut Walker, report: &mut ScanReport) {
    let roots: Vec<String> = report
        .artifacts
        .iter()
        .filter(|a| a.kind == ArtifactKind::Plugin && !crate::plugin::is_marketplace_entry(a))
        .map(|a| a.path.clone())
        .filter(|p| walker.exists(&plugin::marketplace_path(Path::new(p))))
        .collect();
    let published: Vec<(String, String)> = report
        .artifacts
        .iter()
        .filter(|a| {
            a.kind == ArtifactKind::Plugin
                && crate::plugin::is_marketplace_entry(a)
                && !crate::plugin::is_remote_only(a)
        })
        .map(|a| (a.name.clone(), a.path.clone()))
        .collect();
    report.artifacts.retain(|a| {
        let duplicate = a.kind == ArtifactKind::Plugin
            && !crate::plugin::is_marketplace_entry(a)
            && roots.contains(&a.path)
            && published
                .iter()
                .any(|(name, path)| name == &a.name && path != &a.path);
        !duplicate
    });
}

/// Records which tier placed each artifact in `metadata.layoutBy`: plugins come from a manifest
/// (plugin.json, marketplace.json or installed_plugins.json), everything else from a known
/// folder convention.
pub(crate) fn mark_layout_by(report: &mut ScanReport) {
    for a in &mut report.artifacts {
        if a.metadata.contains_key("fromPlugin") {
            continue; // an unpacked plugin part, already marked `plugin.json`
        }
        let tier = if a.kind == ArtifactKind::Plugin {
            "manifest"
        } else {
            "convention"
        };
        a.metadata.insert("layoutBy".into(), tier.into());
        if a.kind == ArtifactKind::Plugin {
            // Which manifest named it: a marketplace entry, else the plugin's own plugin.json
            // (an installed plugin is read from its folder's plugin.json).
            let manifest = crate::plugin::manifest_name(a);
            a.metadata.insert("layoutManifest".into(), manifest.into());
        }
    }
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
