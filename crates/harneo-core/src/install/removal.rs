//! Clean removal: Harneo's files go, the project's originals come back, and the project leaves
//! the registry when the plan is applied.

use std::collections::{HashMap, HashSet};

use super::backup::{find_deleted_originals, find_originals, Original};
use super::plan::{project_folder, Plan, PlanKind, Write, WriteAction};
use super::registry::{InstalledFile, Project};
use super::switch::{backup_count, current_bytes, take_out};
use crate::error::Result;
use crate::library::Library;

impl Library {
    /// The project's originals among `paths`, from its backup sets (see [`find_originals`]).
    pub(super) fn originals_of(
        &self,
        project: &Project,
        paths: &[&str],
    ) -> Result<HashMap<String, Original>> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }
        let folders = self.backup_folders_newest_first(project.id)?;
        let wanted: HashSet<&str> = paths.iter().copied().collect();
        find_originals(&self.data_dir()?, &folders, &wanted)
    }

    /// The plan to remove a project's profile: every file Harneo wrote is deleted (`HARNESS.md`
    /// included) or, when it replaced one of the project's files, replaced by that original.
    /// Files edited since Harneo wrote them are listed in `edited_in_project`; every changed file
    /// is backed up first. Files a refit deleted are put back too. Applying the plan marks the
    /// project removed; its backup sets stay.
    pub fn plan_removal(&self, project_id: i64) -> Result<Plan> {
        let project = self.live_project(project_id)?;
        let target = project_folder(&project.path)?;
        let installed = self.installed_files(project_id)?;
        let manifest: HashMap<&str, &InstalledFile> =
            installed.iter().map(|f| (f.path.as_str(), f)).collect();
        let paths: Vec<&str> = installed.iter().map(|f| f.path.as_str()).collect();
        let originals = self.originals_of(&project, &paths)?;
        let (mut edited, mut restored) = (Vec::new(), Vec::new());
        let mut writes = take_out(
            &target,
            &manifest,
            &paths,
            &originals,
            &mut edited,
            &mut restored,
        )?;
        // Files a refit deleted from the project come back when nothing is at their path now.
        let folders = self.backup_folders_newest_first(project.id)?;
        let deleted =
            find_deleted_originals(&self.data_dir()?, &folders, |p| !manifest.contains_key(p))?;
        for (path, original) in deleted {
            if current_bytes(&target, &path)?.is_none() {
                restored.push(path.clone());
                writes.push(Write {
                    path,
                    action: WriteAction::New,
                    bytes: Some(original.bytes),
                    diff: None,
                    artifact: None,
                    executable: false,
                    mode: original.mode,
                });
            }
        }
        writes.sort_by(|a, b| a.path.cmp(&b.path));
        edited.sort();
        restored.sort();
        Ok(Plan {
            warnings: Vec::new(),
            target: project.path,
            profile_id: project.profile_id,
            reason: format!(
                "Remove {} v{}",
                project.profile_name, project.profile_version
            ),
            profile_name: project.profile_name,
            profile_version: project.profile_version,
            backup_count: backup_count(&writes),
            writes,
            kind: PlanKind::Removal,
            edited_in_project: edited,
            restored,
        })
    }
}
