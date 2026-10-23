//! Refit: comparing a project's existing harness with a profile, item by item, and turning the
//! user's choices into a [`Plan`]. Nothing here writes to the project.
//!
//! An *item* is one artifact install unit (a skill folder, an agent file, `settings.json` with
//! its hook scripts, `.mcp.json`, `CLAUDE.md`, `HARNESS.md`), matched by install path.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::plan::{check_plan, digest, resolve_inside, safe_parts, utc_date};
use super::{Plan, PlanKind, Write, WriteAction};
use crate::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, Origin, ScanOptions};
use crate::error::{Error, Result};
use crate::library::{ArtifactId, Library};
use crate::profiles::layout::load;
use crate::profiles::{Generated, PathFrom, ProfileId};
use crate::updates::hunks;

const SETTINGS: &str = ".claude/settings.json";
const MCP_JSON: &str = ".mcp.json";
const CLAUDE_MD: &str = "CLAUDE.md";
/// The most lost lines a [`Loss`] lists; `count` stays exact.
pub const MAX_LOSS_LINES: usize = 200;

/// What the user can do with one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum RefitAction {
    /// Leave the project's files as they are.
    Keep,
    /// Overwrite the project's files with the profile's (backed up first).
    Replace,
    /// Delete the project's files (backed up first).
    Delete,
    /// Write the profile's files, which the project lacks.
    Add,
    /// Do not add the profile's files.
    Skip,
}

/// Where an item stands between the project and the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum RowKind {
    /// In both, with different content.
    Both,
    /// In both, byte for byte.
    Same,
    /// Only in the project.
    ProjectOnly,
    /// Only in the profile.
    ProfileOnly,
}

/// The "In project" column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum InProject {
    Absent,
    /// Identical to the profile's.
    Same,
    Differs,
    /// There, and the profile's is different or absent.
    Exists,
}

/// The "In profile" column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum InProfile {
    Absent,
    Same,
    /// The project has it too, with other content.
    Differs,
    /// The project does not have it.
    New,
}

/// What `Loss::count` counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum LossUnit {
    Lines,
    /// Permission rules and hook commands of a `settings.json`.
    PermissionRules,
}

/// Content the project has that a replace or delete would lose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Loss {
    /// The lost lines (or rules), in file order, at most [`MAX_LOSS_LINES`].
    pub lines: Vec<String>,
    /// How many there are in all.
    pub count: u32,
    pub unit: LossUnit,
}

/// One item of the comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Row {
    /// The item's install path (`.claude/skills/tdd`, `CLAUDE.md`); the key of a choice.
    pub item: String,
    pub kind: RowKind,
    /// Every file of the item, in the project or the profile, sorted.
    pub paths: Vec<String>,
    pub in_project: InProject,
    pub in_profile: InProfile,
    pub default_action: RefitAction,
    pub options: Vec<RefitAction>,
    /// What `Replace` (a `Both` row) or `Delete` (a `ProjectOnly` row) would lose.
    pub loss: Option<Loss>,
}

/// A project compared with a profile version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Comparison {
    /// The project folder, canonical.
    pub target: String,
    pub profile_id: ProfileId,
    pub profile_name: String,
    pub profile_version: u32,
    /// Sorted by item.
    pub rows: Vec<Row>,
    /// A registered project on this profile version whose files still match what Harneo wrote.
    pub nothing_to_change: bool,
}

/// One file of the profile with its content.
struct ProfileFile {
    bytes: Vec<u8>,
    artifact: Option<ArtifactId>,
    executable: bool,
}

/// An item with the content behind its row.
struct Item {
    row: Row,
    profile: BTreeMap<String, ProfileFile>,
    project: BTreeMap<String, Vec<u8>>,
}

fn install(message: impl Into<String>) -> Error {
    Error::Install {
        message: message.into(),
    }
}

/// The lines of `bytes` that are text; `None` for anything else.
fn text(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes).ok()
}

/// Lines of `old` (not blank) that `new` lacks, in order. A file that is not text has none.
fn lost_lines(old: &[u8], new: &[u8]) -> Vec<String> {
    let (Some(old), Some(new)) = (text(old), text(new)) else {
        return Vec::new();
    };
    let kept: HashSet<&str> = new.lines().collect();
    old.lines()
        .filter(|l| !l.trim().is_empty() && !kept.contains(l))
        .map(str::to_owned)
        .collect()
}

/// Permission rules and hook commands of a `settings.json`, as display strings. `None` when it
/// is not a JSON object.
fn settings_entries(bytes: &[u8]) -> Option<Vec<String>> {
    let Value::Object(root) = serde_json::from_slice(bytes).ok()? else {
        return None;
    };
    let mut out = Vec::new();
    if let Some(Value::Object(perms)) = root.get("permissions") {
        for kind in ["allow", "ask", "deny"] {
            if let Some(Value::Array(rules)) = perms.get(kind) {
                for rule in rules.iter().filter_map(Value::as_str) {
                    out.push(format!("\"{rule}\" ({kind})"));
                }
            }
        }
    }
    if let Some(Value::Object(events)) = root.get("hooks") {
        for (event, groups) in events {
            let Value::Array(groups) = groups else {
                continue;
            };
            for group in groups {
                let Some(Value::Array(hooks)) = group.get("hooks") else {
                    continue;
                };
                for command in hooks
                    .iter()
                    .filter_map(|h| h.get("command").and_then(Value::as_str))
                {
                    out.push(format!("hook {event}: {command}"));
                }
            }
        }
    }
    Some(out)
}

fn capped(lines: Vec<String>, unit: LossUnit) -> Option<Loss> {
    if lines.is_empty() {
        return None;
    }
    let count = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    Some(Loss {
        lines: lines.into_iter().take(MAX_LOSS_LINES).collect(),
        count,
        unit,
    })
}

/// What replacing (or, with no profile file, deleting) the project's files loses.
fn loss_of(
    item: &str,
    project: &BTreeMap<String, Vec<u8>>,
    profile: &BTreeMap<String, ProfileFile>,
) -> Option<Loss> {
    if item == SETTINGS {
        if let Some(old) = project.get(SETTINGS).and_then(|b| settings_entries(b)) {
            let new = match profile.get(SETTINGS) {
                Some(f) => settings_entries(&f.bytes),
                None => Some(Vec::new()),
            };
            if let Some(new) = new {
                let kept: HashSet<&String> = new.iter().collect();
                let lost: Vec<String> = old.into_iter().filter(|e| !kept.contains(e)).collect();
                if let Some(loss) = capped(lost, LossUnit::PermissionRules) {
                    return Some(loss);
                }
            }
        }
    }
    let mut lines = Vec::new();
    for (path, old) in project {
        let new = profile.get(path).map_or(&[][..], |f| f.bytes.as_slice());
        lines.extend(lost_lines(old, new));
    }
    capped(lines, LossUnit::Lines)
}

fn is_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(meta.is_file()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })
}

impl Row {
    fn new(item: &str, kind: RowKind, paths: Vec<String>) -> Self {
        let (in_project, in_profile, default_action, options) = match kind {
            RowKind::Both => (
                InProject::Exists,
                InProfile::Differs,
                RefitAction::Replace,
                vec![RefitAction::Keep, RefitAction::Replace],
            ),
            RowKind::Same => (
                InProject::Same,
                InProfile::Same,
                RefitAction::Keep,
                vec![RefitAction::Keep],
            ),
            RowKind::ProjectOnly => (
                InProject::Exists,
                InProfile::Absent,
                RefitAction::Keep,
                vec![RefitAction::Keep, RefitAction::Delete],
            ),
            RowKind::ProfileOnly => (
                InProject::Absent,
                InProfile::New,
                RefitAction::Add,
                vec![RefitAction::Add, RefitAction::Skip],
            ),
        };
        Self {
            item: item.to_owned(),
            kind,
            paths,
            in_project,
            in_profile,
            default_action,
            options,
            loss: None,
        }
    }
}

impl Library {
    /// Compares the harness in `target` with profile `profile_id` at `version` (the current one
    /// when `None`), item by item. Reads the project; writes nothing.
    pub fn compare(
        &self,
        target: &Path,
        profile_id: ProfileId,
        version: Option<u32>,
    ) -> Result<Comparison> {
        Ok(self.inspect(target, profile_id, version)?.0)
    }

    /// The plan for the user's `choices` (item → action; an item without one takes its default)
    /// on a fresh look at `comparison`'s project, so the plan matches the disk as it is now.
    pub fn plan_refit(
        &self,
        comparison: &Comparison,
        choices: &BTreeMap<String, RefitAction>,
    ) -> Result<Plan> {
        self.refit_plan(
            Path::new(&comparison.target),
            comparison.profile_id,
            Some(comparison.profile_version),
            choices,
        )
    }

    /// [`Library::compare`] followed by [`Library::plan_refit`].
    pub fn refit_plan(
        &self,
        target: &Path,
        profile_id: ProfileId,
        version: Option<u32>,
        choices: &BTreeMap<String, RefitAction>,
    ) -> Result<Plan> {
        let (comparison, items) = self.inspect(target, profile_id, version)?;
        for (item, action) in choices {
            let row = comparison
                .rows
                .iter()
                .find(|r| &r.item == item)
                .ok_or_else(|| install(format!("{item} is not part of this comparison")))?;
            if !row.options.contains(action) {
                return Err(install(format!("{item}: {action:?} is not an option")));
            }
        }
        let mut writes = Vec::new();
        for item in &items {
            let action = choices
                .get(&item.row.item)
                .copied()
                .unwrap_or(item.row.default_action);
            writes.extend(item_writes(item, action));
        }
        writes.sort_by(|a, b| a.path.cmp(&b.path));
        let backup_count = writes
            .iter()
            .filter(|w| matches!(w.action, WriteAction::Replace | WriteAction::Delete))
            .count();
        let plan = Plan {
            target: comparison.target,
            profile_id,
            reason: format!(
                "Refit {} v{}",
                comparison.profile_name, comparison.profile_version
            ),
            profile_name: comparison.profile_name,
            profile_version: comparison.profile_version,
            writes,
            backup_count: u32::try_from(backup_count).unwrap_or(u32::MAX),
            kind: PlanKind::Refit,
            edited_in_project: Vec::new(),
            restored: Vec::new(),
        };
        check_plan(&plan)?;
        Ok(plan)
    }

    fn inspect(
        &self,
        target: &Path,
        profile_id: ProfileId,
        version: Option<u32>,
    ) -> Result<(Comparison, Vec<Item>)> {
        let shown = target.display().to_string();
        let root = target
            .canonicalize()
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| install(format!("{shown} is not a folder")))?;
        let view = self.profile(profile_id, version)?;
        let registered = self
            .projects()?
            .into_iter()
            .find(|p| Path::new(&p.path) == root);
        let same_version = registered
            .as_ref()
            .filter(|p| p.profile_id == profile_id && p.profile_version == view.version);
        // HARNESS.md carries the install date: reuse the project's own so it stays comparable.
        let date =
            utc_date(same_version.map_or_else(crate::library::records::now, |p| p.applied_at));
        let rendered = self.render(&view.spec, &view.name, view.version, &date)?;

        let mut groups: BTreeMap<String, (BTreeMap<String, ProfileFile>, BTreeSet<String>)> =
            BTreeMap::new();
        for file in rendered.files {
            let (key, artifact) = match &file.from {
                PathFrom::Generated {
                    generated: Generated::Settings,
                } => (SETTINGS.to_owned(), None),
                PathFrom::Generated {
                    generated: Generated::McpJson,
                } => (MCP_JSON.to_owned(), None),
                PathFrom::Generated {
                    generated: Generated::HarnessMd,
                } => (file.path.clone(), None),
                PathFrom::Artifact { artifact: id } => {
                    let key = match load(&self.store.conn, id)? {
                        Some(a) => match a.kind {
                            ArtifactKind::Settings => SETTINGS.to_owned(),
                            ArtifactKind::Mcp => MCP_JSON.to_owned(),
                            ArtifactKind::ClaudeMd => CLAUDE_MD.to_owned(),
                            _ => ClaudeAdapter
                                .install_path(&a)
                                .to_string_lossy()
                                .replace('\\', "/"),
                        },
                        None => file.path.clone(),
                    };
                    (key, Some(id.clone()))
                }
            };
            groups.entry(key).or_default().0.insert(
                file.path,
                ProfileFile {
                    bytes: file.bytes,
                    artifact,
                    executable: file.executable,
                },
            );
        }

        // The project's own harness, by what a scan finds. Files Harneo does not know are ignored.
        let report = ClaudeAdapter.scan(&root, &ScanOptions::default())?;
        for artifact in report.artifacts {
            if artifact.kind == ArtifactKind::Plugin || artifact.origin != Origin::Own {
                continue;
            }
            let key = match artifact.kind {
                ArtifactKind::Settings => SETTINGS.to_owned(),
                ArtifactKind::Mcp => MCP_JSON.to_owned(),
                _ => artifact
                    .path
                    .split('#')
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
            };
            let entry = groups.entry(key).or_default();
            // CLAUDE.md lists the files it imports; those are items of their own.
            let files = if artifact.kind == ArtifactKind::ClaudeMd {
                vec![artifact.path.clone()]
            } else {
                artifact.files
            };
            for path in files {
                if safe_parts(&path).is_ok() {
                    entry.1.insert(path);
                }
            }
        }
        // Profile files already on disk belong to their item whether or not a scan found them.
        for (profile, project) in groups.values_mut() {
            for path in profile.keys() {
                if is_file(&resolve_inside(&root, path)?)? {
                    project.insert(path.clone());
                }
            }
        }

        let mut items = Vec::new();
        for (key, (profile, paths)) in groups {
            let mut project = BTreeMap::new();
            for path in paths {
                let file = resolve_inside(&root, &path)?;
                if is_file(&file)? {
                    project.insert(path, read(&file)?);
                }
            }
            let kind = match (profile.is_empty(), project.is_empty()) {
                (true, true) => continue,
                (true, false) => RowKind::ProjectOnly,
                (false, true) => RowKind::ProfileOnly,
                (false, false) => {
                    let same = project.len() == profile.len()
                        && project
                            .iter()
                            .all(|(p, b)| profile.get(p).is_some_and(|f| &f.bytes == b));
                    if same {
                        RowKind::Same
                    } else {
                        RowKind::Both
                    }
                }
            };
            let all: BTreeSet<&String> = profile.keys().chain(project.keys()).collect();
            let mut row = Row::new(&key, kind, all.into_iter().cloned().collect());
            if matches!(kind, RowKind::Both | RowKind::ProjectOnly) {
                row.loss = loss_of(&key, &project, &profile);
            }
            items.push(Item {
                row,
                profile,
                project,
            });
        }

        let nothing_to_change = match same_version {
            Some(p) => {
                items
                    .iter()
                    .all(|i| matches!(i.row.kind, RowKind::Same | RowKind::ProjectOnly))
                    && self.manifest_matches(p.id, &root)?
            }
            None => false,
        };
        let comparison = Comparison {
            target: root.display().to_string(),
            profile_id,
            profile_name: view.name,
            profile_version: view.version,
            rows: items.iter().map(|i| i.row.clone()).collect(),
            nothing_to_change,
        };
        Ok((comparison, items))
    }

    /// Every file Harneo recorded for the project is on disk with the recorded content.
    fn manifest_matches(&self, project_id: i64, root: &Path) -> Result<bool> {
        for file in self.installed_files(project_id)? {
            let path = resolve_inside(root, &file.path)?;
            if !is_file(&path)? || digest(&read(&path)?) != file.digest {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// The writes of one item under `action`.
fn item_writes(item: &Item, action: RefitAction) -> Vec<Write> {
    let write = |path: &str, action, bytes: Option<Vec<u8>>, diff, artifact| Write {
        path: path.to_owned(),
        action,
        bytes,
        diff,
        artifact,
        executable: false,
    };
    let exec = |mut w: Write, f: &ProfileFile| {
        w.executable = f.executable;
        w
    };
    let mut out = Vec::new();
    for path in &item.row.paths {
        let (mine, theirs) = (item.project.get(path), item.profile.get(path));
        let artifact = theirs.and_then(|f| f.artifact.clone());
        out.push(match (action, mine, theirs) {
            (RefitAction::Replace | RefitAction::Add, None, Some(f)) => exec(
                write(
                    path,
                    WriteAction::New,
                    Some(f.bytes.clone()),
                    None,
                    artifact,
                ),
                f,
            ),
            (RefitAction::Replace, Some(old), Some(f)) if *old != f.bytes => exec(
                write(
                    path,
                    WriteAction::Replace,
                    Some(f.bytes.clone()),
                    Some(hunks(old, &f.bytes)),
                    artifact,
                ),
                f,
            ),
            // Already what the profile has: nothing to write, and the manifest tracks it.
            (RefitAction::Replace | RefitAction::Keep, Some(old), Some(f)) if *old == f.bytes => {
                write(path, WriteAction::Keep, Some(old.clone()), None, artifact)
            }
            (RefitAction::Replace | RefitAction::Delete, Some(_), None)
            | (RefitAction::Delete, Some(_), Some(_)) => {
                write(path, WriteAction::Delete, None, None, None)
            }
            (_, Some(_), _) => write(path, WriteAction::Keep, None, None, None),
            (_, None, _) => write(path, WriteAction::Skip, None, None, None),
        });
    }
    out
}
