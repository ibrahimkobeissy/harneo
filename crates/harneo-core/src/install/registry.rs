//! The install registry: which projects Harneo installed profiles into, what it wrote there
//! (the manifest) and the backup sets it made.

use std::collections::HashSet;
use std::io::Read;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::backup::BackupSet;
use super::plan::{digest, project_folder, resolve_inside, Plan, PlanKind, WriteAction};
use crate::error::{Error, Result};
use crate::library::{ArtifactId, Library};
use crate::profiles::ProfileId;

/// The most of a managed file `project_file_content` returns.
pub const MAX_FILE_BYTES: u64 = 512 * 1024;

/// A project Harneo installed a profile into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    /// The project folder, canonical.
    pub path: String,
    pub name: String,
    pub profile_id: ProfileId,
    pub profile_name: String,
    pub profile_version: u32,
    /// Unix seconds of the last apply.
    pub applied_at: i64,
    /// Files in the manifest.
    pub file_count: u32,
    /// Unix seconds when the profile was removed; `None` for a live project. A removed project
    /// has no manifest and `profile_id` 0; its backup sets stay listed and restorable.
    pub removed_at: Option<i64>,
}

/// One file Harneo wrote into a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct InstalledFile {
    /// Relative to the project root.
    pub path: String,
    /// SHA-256 of what Harneo wrote.
    pub digest: String,
    /// The library artifact it came from; `None` for generated files.
    pub artifact_id: Option<ArtifactId>,
}

/// One backup set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Backup {
    pub id: i64,
    pub project_id: i64,
    /// Unix seconds.
    pub created_at: i64,
    pub reason: String,
    pub file_count: u32,
    /// Relative to Harneo's data folder.
    pub folder: String,
}

/// A managed file read back from a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProjectFile {
    pub path: String,
    /// The text (invalid UTF-8 replaced).
    pub content: String,
    /// The file is longer than [`MAX_FILE_BYTES`] and was cut.
    pub truncated: bool,
}

/// How many projects use a profile.
pub(crate) fn project_count(conn: &Connection, profile: ProfileId) -> Result<u32> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE profile_id = ?1 AND removed_at IS NULL",
        params![profile],
        |r| r.get(0),
    )?)
}

/// The names of the projects using a profile.
pub(crate) fn project_names(conn: &Connection, profile: ProfileId) -> Result<Vec<String>> {
    Ok(conn
        .prepare(
            "SELECT name FROM projects WHERE profile_id = ?1 AND removed_at IS NULL
             ORDER BY name COLLATE NOCASE",
        )?
        .query_map(params![profile], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?)
}

/// Registers the plan's project (or updates the one at that path) and returns its id.
pub(super) fn upsert_project(conn: &Connection, plan: &Plan, now: i64) -> Result<i64> {
    let name = std::path::Path::new(&plan.target)
        .file_name()
        .map_or_else(|| plan.target.clone(), |n| n.to_string_lossy().into_owned());
    Ok(conn.query_row(
        "INSERT INTO projects (path, name, profile_id, profile_version, applied_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) WHERE removed_at IS NULL DO UPDATE SET
             name = excluded.name, profile_id = excluded.profile_id,
             profile_version = excluded.profile_version, applied_at = excluded.applied_at
         RETURNING id",
        params![
            plan.target,
            name,
            plan.profile_id,
            plan.profile_version,
            now
        ],
        |r| r.get(0),
    )?)
}

/// The project leaves the live list but stays registered as removed, so its backup sets stay
/// listed and restorable. Its manifest goes.
pub(super) fn mark_removed(conn: &Connection, project_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM installed_files WHERE project_id = ?1",
        params![project_id],
    )?;
    conn.execute(
        "UPDATE projects SET removed_at = ?2, profile_name = (SELECT name FROM profiles
             WHERE id = projects.profile_id), profile_id = NULL WHERE id = ?1",
        params![project_id, now],
    )?;
    Ok(())
}

/// Records the manifest and the backup row of a finished apply; returns the backup's id.
pub(super) fn record(
    conn: &Connection,
    project_id: i64,
    plan: &Plan,
    set: Option<&BackupSet>,
    now: i64,
) -> Result<Option<i64>> {
    for write in &plan.writes {
        match (write.action, &write.bytes) {
            (WriteAction::New | WriteAction::Replace, _) if plan.restored.contains(&write.path) => {
                conn.execute(
                    "DELETE FROM installed_files WHERE project_id = ?1 AND path = ?2",
                    params![project_id, write.path],
                )?;
            }
            (WriteAction::New | WriteAction::Replace | WriteAction::Keep, Some(bytes)) => {
                conn.execute(
                    "INSERT OR REPLACE INTO installed_files (project_id, path, digest, artifact_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![project_id, write.path, digest(bytes), write.artifact],
                )?;
            }
            (WriteAction::Delete, _) => {
                conn.execute(
                    "DELETE FROM installed_files WHERE project_id = ?1 AND path = ?2",
                    params![project_id, write.path],
                )?;
            }
            _ => {}
        }
    }
    if plan.kind == PlanKind::Removal {
        mark_removed(conn, project_id, now)?;
    }
    let Some(set) = set else {
        return Ok(None);
    };
    conn.execute(
        "INSERT INTO backups (project_id, created_at, reason, file_count, folder)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![project_id, now, plan.reason, set.file_count, set.folder],
    )?;
    Ok(Some(conn.last_insert_rowid()))
}

const PROJECT_SELECT: &str = "SELECT p.id, p.path, p.name, COALESCE(p.profile_id, 0),
        COALESCE(f.name, p.profile_name, ''), p.profile_version,
        p.applied_at, (SELECT COUNT(*) FROM installed_files i WHERE i.project_id = p.id),
        p.removed_at
     FROM projects p LEFT JOIN profiles f ON f.id = p.profile_id";

fn project_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        path: r.get(1)?,
        name: r.get(2)?,
        profile_id: r.get(3)?,
        profile_name: r.get(4)?,
        profile_version: r.get(5)?,
        applied_at: r.get(6)?,
        file_count: r.get(7)?,
        removed_at: r.get(8)?,
    })
}

impl Library {
    /// Every live project, by name.
    pub fn projects(&self) -> Result<Vec<Project>> {
        Ok(self
            .store
            .conn
            .prepare(&format!(
                "{PROJECT_SELECT} WHERE p.removed_at IS NULL ORDER BY p.name COLLATE NOCASE, p.id"
            ))?
            .query_map([], project_row)?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// Projects whose profile was removed, newest removal first. Their backup sets stay.
    pub fn removed_projects(&self) -> Result<Vec<Project>> {
        Ok(self
            .store
            .conn
            .prepare(&format!(
                "{PROJECT_SELECT} WHERE p.removed_at IS NOT NULL
                 ORDER BY p.removed_at DESC, p.id DESC"
            ))?
            .query_map([], project_row)?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// A project that is not removed; the one to install into, switch or remove.
    pub(super) fn live_project(&self, id: i64) -> Result<Project> {
        let project = self.project(id)?;
        if project.removed_at.is_some() {
            return Err(Error::Install {
                message: format!("{} was removed from Harneo", project.name),
            });
        }
        Ok(project)
    }

    /// One project, live or removed.
    pub fn project(&self, id: i64) -> Result<Project> {
        self.store
            .conn
            .query_row(
                &format!("{PROJECT_SELECT} WHERE p.id = ?1"),
                params![id],
                project_row,
            )
            .optional()?
            .ok_or(Error::ProjectNotFound { id })
    }

    /// The manifest of a project: every file Harneo wrote, by path.
    pub fn installed_files(&self, id: i64) -> Result<Vec<InstalledFile>> {
        self.project(id)?;
        Ok(self
            .store
            .conn
            .prepare(
                "SELECT path, digest, artifact_id FROM installed_files
                 WHERE project_id = ?1 ORDER BY path",
            )?
            .query_map(params![id], |r| {
                Ok(InstalledFile {
                    path: r.get(0)?,
                    digest: r.get(1)?,
                    artifact_id: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// The backup sets of a project, newest first.
    pub fn backups(&self, id: i64) -> Result<Vec<Backup>> {
        self.project(id)?;
        Ok(self
            .store
            .conn
            .prepare(
                "SELECT id, project_id, created_at, reason, file_count, folder FROM backups
                 WHERE project_id = ?1 ORDER BY created_at DESC, id DESC",
            )?
            .query_map(params![id], |r| {
                Ok(Backup {
                    id: r.get(0)?,
                    project_id: r.get(1)?,
                    created_at: r.get(2)?,
                    reason: r.get(3)?,
                    file_count: r.get(4)?,
                    folder: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// The current content of one managed file of a project, cut at [`MAX_FILE_BYTES`]. Only
    /// files in the manifest can be read, and never through a symlink.
    pub fn project_file_content(&self, id: i64, path: &str) -> Result<ProjectFile> {
        let project = self.project(id)?;
        if !self.installed_files(id)?.iter().any(|f| f.path == path) {
            return Err(Error::Install {
                message: format!("{path} is not a file Harneo installed in {}", project.name),
            });
        }
        let target = project_folder(&project.path)?;
        let file = resolve_inside(&target, path)?;
        let io = |source| Error::Io {
            path: file.display().to_string(),
            source,
        };
        let mut bytes = Vec::new();
        std::fs::File::open(&file)
            .map_err(io)?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        let truncated = bytes.len() as u64 > MAX_FILE_BYTES;
        if truncated {
            bytes.truncate(usize::try_from(MAX_FILE_BYTES).unwrap_or(usize::MAX));
        }
        Ok(ProjectFile {
            path: path.to_owned(),
            content: String::from_utf8_lossy(&bytes).into_owned(),
            truncated,
        })
    }
}

// ---- Switch, removal and restore ---------------------------------------------------------------

/// The project row an apply works on: registered or updated for an install or a switch, looked
/// up (and required) for a removal or a restore.
pub(super) fn project_for(
    conn: &Connection,
    plan: &Plan,
    now: i64,
    only: Option<i64>,
) -> Result<i64> {
    if let Some(id) = only {
        // A restore names its project: it may be a removed one.
        return conn
            .query_row("SELECT id FROM projects WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or(Error::ProjectNotFound { id });
    }
    match plan.kind {
        PlanKind::Scaffold | PlanKind::Refit | PlanKind::Switch => upsert_project(conn, plan, now),
        PlanKind::Removal | PlanKind::Restore => conn
            .query_row(
                "SELECT id FROM projects WHERE path = ?1 AND removed_at IS NULL",
                params![plan.target],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::Install {
                message: format!("{} is not a project Harneo installed into", plan.target),
            }),
    }
}

/// The paths of a project's manifest.
pub(super) fn manifest_paths(conn: &Connection, project_id: i64) -> Result<HashSet<String>> {
    Ok(conn
        .prepare("SELECT path FROM installed_files WHERE project_id = ?1")?
        .query_map(params![project_id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?)
}

pub(super) fn delete_backup_row(conn: &Connection, backup_id: i64) -> Result<()> {
    conn.execute("DELETE FROM backups WHERE id = ?1", params![backup_id])?;
    Ok(())
}

impl Library {
    /// One backup set.
    pub fn backup(&self, backup_id: i64) -> Result<Backup> {
        self.store
            .conn
            .query_row(
                "SELECT id, project_id, created_at, reason, file_count, folder FROM backups
                 WHERE id = ?1",
                params![backup_id],
                |r| {
                    Ok(Backup {
                        id: r.get(0)?,
                        project_id: r.get(1)?,
                        created_at: r.get(2)?,
                        reason: r.get(3)?,
                        file_count: r.get(4)?,
                        folder: r.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or(Error::BackupNotFound { id: backup_id })
    }

    /// The folders of a project's backup sets, newest first.
    pub(super) fn backup_folders_newest_first(&self, project_id: i64) -> Result<Vec<String>> {
        Ok(self
            .store
            .conn
            .prepare(
                "SELECT folder FROM backups WHERE project_id = ?1
                 ORDER BY created_at DESC, id DESC",
            )?
            .query_map(params![project_id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?)
    }
}
