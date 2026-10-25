//! Checking a project against its manifest: edited, deleted and added files, library copies
//! newer than what the project holds, and a newer version of its profile. Reads only.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use ts_rs::TS;

use crate::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions};
use crate::error::{Error, Result};
use crate::install::{InstalledFile, Plan, PlanKind, Project, Rendered, Write, WriteAction};
use crate::library::{ArtifactId, Library};
use crate::profiles::PathFrom;
use crate::updates::Hunk;

/// How a project stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DriftStatus {
    InSync,
    /// At least one managed file was edited or deleted, or an artifact was added.
    Drift,
    /// The project folder is gone.
    Missing,
}

/// What happened to one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DriftChange {
    /// Edited in the project; line counts against the profile version's render.
    Edited { plus: u32, minus: u32 },
    /// Deleted in the project.
    Deleted,
    /// An artifact in the project's harness that Harneo did not write.
    Added,
}

/// One drifted file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DriftFile {
    /// Relative to the project root, with forward slashes.
    pub path: String,
    pub change: DriftChange,
    /// The library artifact the file came from; `None` for generated and added files.
    pub artifact: Option<ArtifactId>,
}

/// Everything a check found for one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DriftReport {
    pub status: DriftStatus,
    /// Edited and deleted managed files, then added ones, each sorted by path.
    pub files: Vec<DriftFile>,
    /// Managed files that still match the manifest but whose library copies now render
    /// differently: the library moved on.
    pub library_newer: Vec<String>,
    /// The profile's current version when it is newer than the project's.
    pub profile_newer: Option<u32>,
}

/// The counts a list needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DriftSummary {
    pub status: DriftStatus,
    /// Files in `DriftReport::files`.
    pub drifted: u32,
    pub library_newer: u32,
    pub profile_newer: Option<u32>,
}

/// A project's summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProjectDrift {
    pub project_id: i64,
    pub summary: DriftSummary,
}

fn drift(message: impl Into<String>) -> Error {
    Error::Drift {
        message: message.into(),
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Lines added and removed going from `old` to `new`; zero for either when not UTF-8 text.
fn line_counts(old: &[u8], new: &[u8]) -> (u32, u32) {
    let (Ok(old), Ok(new)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        return (0, 0);
    };
    let (mut plus, mut minus) = (0, 0);
    for change in TextDiff::from_lines(old, new).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => plus += 1,
            ChangeTag::Delete => minus += 1,
            ChangeTag::Equal => {}
        }
    }
    (plus, minus)
}

/// The kinds a project can hold on its own and that import can bring into the library. Settings,
/// MCP servers and plugins are merged into shared files, so they are never "added".
fn addable(kind: ArtifactKind) -> bool {
    !matches!(
        kind,
        ArtifactKind::Settings | ArtifactKind::Mcp | ArtifactKind::Plugin | ArtifactKind::ClaudeMd
    )
}

/// The bytes of `rel` in the project: `None` when absent.
pub(super) fn disk_bytes(root: &Path, rel: &str) -> Result<Option<Vec<u8>>> {
    let path = crate::install::resolve_inside(root, rel)?;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => std::fs::read(&path).map(Some).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        }),
        Ok(_) => Err(drift(format!("{rel} is not a regular file"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

impl Library {
    /// What the project's pinned profile version renders now, with the project's own date.
    pub(super) fn render_pinned(&self, project: &Project, date: &str) -> Result<Rendered> {
        let view = self.profile(project.profile_id, Some(project.profile_version))?;
        self.render(&view.spec, &view.name, view.version, date)
    }

    /// Compares the project's files with the manifest. Writes nothing.
    pub fn check_project(&self, id: i64) -> Result<DriftReport> {
        let project = self.project(id)?;
        let profile_newer = self
            .profile(project.profile_id, None)
            .ok()
            .map(|p| p.current_version)
            .filter(|current| *current > project.profile_version);
        let root = Path::new(&project.path);
        if !root.is_dir() {
            return Ok(DriftReport {
                status: DriftStatus::Missing,
                files: Vec::new(),
                library_newer: Vec::new(),
                profile_newer,
            });
        }
        let installed = self.installed_files(id)?;
        // A render that fails (an artifact left the library) only costs the line counts and the
        // library-newer signal.
        let date = crate::install::utc_date(project.applied_at);
        let rendered = self.render_pinned(&project, &date).ok();
        let by_path: HashMap<&str, &[u8]> = rendered
            .iter()
            .flat_map(|r| &r.files)
            .map(|f| (f.path.as_str(), f.bytes.as_slice()))
            .collect();

        let mut files = Vec::new();
        let mut library_newer = Vec::new();
        for file in &installed {
            // A managed path that is not a regular file (a symlink, a folder) or cannot be read
            // was changed in the project; it must not stop the check.
            let Ok(disk) = disk_bytes(root, &file.path) else {
                files.push(DriftFile {
                    path: file.path.clone(),
                    change: DriftChange::Edited { plus: 0, minus: 0 },
                    artifact: file.artifact_id.clone(),
                });
                continue;
            };
            let change = match &disk {
                None => Some(DriftChange::Deleted),
                Some(bytes) if crate::install::digest(bytes) != file.digest => {
                    let (plus, minus) = line_counts(
                        by_path.get(file.path.as_str()).copied().unwrap_or(bytes),
                        bytes,
                    );
                    Some(DriftChange::Edited { plus, minus })
                }
                Some(_) => {
                    let newer = by_path
                        .get(file.path.as_str())
                        .is_some_and(|b| crate::install::digest(b) != file.digest);
                    if newer {
                        library_newer.push(file.path.clone());
                    }
                    None
                }
            };
            if let Some(change) = change {
                files.push(DriftFile {
                    path: file.path.clone(),
                    change,
                    artifact: file.artifact_id.clone(),
                });
            }
        }
        files.extend(self.added_files(root, &installed)?);
        files.sort_by(|a, b| {
            let rank = |f: &DriftFile| u8::from(f.change == DriftChange::Added);
            rank(a).cmp(&rank(b)).then_with(|| a.path.cmp(&b.path))
        });
        library_newer.sort();
        Ok(DriftReport {
            status: if files.is_empty() {
                DriftStatus::InSync
            } else {
                DriftStatus::Drift
            },
            files,
            library_newer,
            profile_newer,
        })
    }

    /// Artifacts the project's harness holds that Harneo did not write. A skill is listed by its
    /// folder.
    fn added_files(&self, root: &Path, installed: &[InstalledFile]) -> Result<Vec<DriftFile>> {
        let managed: HashSet<&str> = installed.iter().map(|f| f.path.as_str()).collect();
        let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
        let mut out = Vec::new();
        for artifact in report.artifacts {
            if !addable(artifact.kind) || artifact.origin != crate::artifact::Origin::Own {
                continue;
            }
            let path = crate::artifact::rel_string(&ClaudeAdapter.install_path(&artifact));
            let inside = format!("{path}/");
            if managed.contains(path.as_str())
                || managed.iter().any(|m| m.starts_with(&inside))
                || crate::install::safe_parts(&path).is_err()
            {
                continue;
            }
            out.push(DriftFile {
                path,
                change: DriftChange::Added,
                artifact: None,
            });
        }
        Ok(out)
    }

    /// The status of every project, by name. A project whose check fails is listed as drifted
    /// (with nothing to show) instead of hiding the others.
    pub fn check_all(&self) -> Result<Vec<ProjectDrift>> {
        Ok(self
            .projects()?
            .into_iter()
            .map(|project| {
                let summary = match self.check_project(project.id) {
                    Ok(report) => DriftSummary {
                        status: report.status,
                        drifted: count(report.files.len()),
                        library_newer: count(report.library_newer.len()),
                        profile_newer: report.profile_newer,
                    },
                    Err(_) => DriftSummary {
                        status: DriftStatus::Drift,
                        drifted: 0,
                        library_newer: 0,
                        profile_newer: None,
                    },
                };
                ProjectDrift {
                    project_id: project.id,
                    summary,
                }
            })
            .collect())
    }

    /// The line changes from what the project's profile version renders to the project's file
    /// (a deleted file is shown as all lines removed; a file the profile does not produce, such
    /// as an added artifact, as all lines added).
    pub fn drift_diff(&self, id: i64, path: &str) -> Result<Vec<Hunk>> {
        let project = self.project(id)?;
        let root = Path::new(&project.path);
        let date = crate::install::utc_date(project.applied_at);
        let rendered = self.render_pinned(&project, &date)?;
        let profile = rendered
            .files
            .iter()
            .find(|f| f.path == path)
            .map_or(&[][..], |f| f.bytes.as_slice());
        let disk = disk_bytes(root, path)?.unwrap_or_default();
        Ok(crate::updates::diff::hunks(profile, &disk))
    }

    /// The plan to put the profile's version back: edited and deleted files and library-newer
    /// files (`paths` narrows it, `None` takes all) are written from what the pinned version
    /// renders now. Added files are never touched. Edited files are listed in
    /// `edited_in_project` and backed up by `apply` like every replaced file. `HARNESS.md` is refreshed too if it is untouched and out of date, since an
    /// apply stamps the project with today's date.
    pub fn plan_reapply(&self, id: i64, paths: Option<Vec<String>>) -> Result<Plan> {
        let project = self.project(id)?;
        let report = self.check_project(id)?;
        if report.status == DriftStatus::Missing {
            return Err(drift(format!("{} is not on disk any more", project.path)));
        }
        let root = crate::install::project_folder(&project.path)?;
        let rendered = self.render_pinned(
            &project,
            &crate::install::utc_date(crate::library::records::now()),
        )?;
        let candidates: HashSet<&str> = report
            .files
            .iter()
            .filter(|f| f.change != DriftChange::Added)
            .map(|f| f.path.as_str())
            .chain(report.library_newer.iter().map(String::as_str))
            .collect();
        let mut wanted: Vec<String> = match &paths {
            None => candidates.iter().map(|p| (*p).to_owned()).collect(),
            Some(list) => {
                for path in list {
                    if !candidates.contains(path.as_str()) {
                        return Err(drift(format!("{path} has nothing to re-apply")));
                    }
                }
                list.clone()
            }
        };
        let manifest: HashMap<String, InstalledFile> = self
            .installed_files(id)?
            .into_iter()
            .map(|f| (f.path.clone(), f))
            .collect();
        if !wanted.iter().any(|p| p == "HARNESS.md") {
            let fresh = rendered.files.iter().find(|f| f.path == "HARNESS.md");
            let stale = manifest.get("HARNESS.md").is_some_and(|m| {
                matches!(
                    (disk_bytes(&root, "HARNESS.md"), fresh),
                    (Ok(Some(b)), Some(fresh))
                        if crate::install::digest(&b) == m.digest && b != fresh.bytes
                )
            });
            if stale {
                wanted.push("HARNESS.md".to_owned());
            }
        }
        wanted.sort();
        wanted.dedup();

        let mut writes = Vec::new();
        let mut edited = Vec::new();
        for path in wanted {
            let file = rendered
                .files
                .iter()
                .find(|f| f.path == path)
                .ok_or_else(|| drift(format!("{path} is no longer part of the profile")))?;
            let disk = disk_bytes(&root, &path)?;
            if let (Some(bytes), Some(known)) = (&disk, manifest.get(&path)) {
                if crate::install::digest(bytes) != known.digest {
                    edited.push(path.clone());
                }
            }
            writes.push(Write {
                action: if disk.is_some() {
                    WriteAction::Replace
                } else {
                    WriteAction::New
                },
                diff: disk
                    .as_ref()
                    .map(|b| crate::updates::diff::hunks(b, &file.bytes)),
                bytes: Some(file.bytes.clone()),
                artifact: match &file.from {
                    PathFrom::Artifact { artifact } => Some(artifact.clone()),
                    PathFrom::Generated { .. } => None,
                },
                executable: file.executable,
                mode: None,
                path,
            });
        }
        let backups = writes
            .iter()
            .filter(|w| w.action == WriteAction::Replace)
            .count();
        Ok(Plan {
            warnings: Vec::new(),
            target: project.path,
            profile_id: project.profile_id,
            reason: format!(
                "Re-apply {} v{}",
                project.profile_name, project.profile_version
            ),
            profile_name: project.profile_name,
            profile_version: project.profile_version,
            backup_count: count(backups),
            writes,
            kind: PlanKind::Switch,
            edited_in_project: edited,
            restored: Vec::new(),
        })
    }
}
