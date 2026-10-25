//! Switching a project to another profile or version: only the difference between what Harneo
//! wrote (the manifest) and what the new profile renders is planned.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::backup::Original;
use super::plan::{
    digest, project_folder, read_capped, resolve_inside, Plan, PlanKind, Write, WriteAction,
};
use super::registry::InstalledFile;
use crate::error::{Error, Result};
use crate::library::Library;
use crate::profiles::{PathFrom, ProfileId, ProfileView};

/// The bytes of `rel` in the project, `None` when there is no such file.
pub(super) fn current_bytes(target: &Path, rel: &str) -> Result<Option<Vec<u8>>> {
    let path = resolve_inside(target, rel)?;
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => read_capped(&path).map(Some),
        Ok(_) => Err(Error::Install {
            message: format!("{rel} is not a regular file"),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

/// `New` for a file that is gone from the project, `Replace` for one that is there.
fn put_action(present: bool) -> WriteAction {
    if present {
        WriteAction::Replace
    } else {
        WriteAction::New
    }
}

/// Counts the files a plan backs up first.
pub(super) fn backup_count(writes: &[Write]) -> u32 {
    let n = writes
        .iter()
        .filter(|w| matches!(w.action, WriteAction::Replace | WriteAction::Delete))
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The writes that take every manifest path in `gone` out of the project: a path that replaced
/// one of the project's originals gets it back (listed in `restored`), any other is deleted.
/// Paths whose file was edited since Harneo wrote it are listed in `edited`.
pub(super) fn take_out(
    target: &Path,
    manifest: &HashMap<&str, &InstalledFile>,
    gone: &[&str],
    originals: &HashMap<String, Original>,
    edited: &mut Vec<String>,
    restored: &mut Vec<String>,
) -> Result<Vec<Write>> {
    let mut writes = Vec::new();
    for &path in gone {
        let disk = current_bytes(target, path)?;
        if let (Some(bytes), Some(file)) = (&disk, manifest.get(path)) {
            if digest(bytes) != file.digest {
                edited.push(path.to_owned());
            }
        }
        let (action, bytes, mode) = match originals.get(path) {
            Some(original) => {
                restored.push(path.to_owned());
                (
                    put_action(disk.is_some()),
                    Some(original.bytes.clone()),
                    original.mode,
                )
            }
            None => (WriteAction::Delete, None, None),
        };
        writes.push(Write {
            path: path.to_owned(),
            action,
            bytes,
            diff: None,
            artifact: None,
            executable: false,
            mode,
        });
    }
    Ok(writes)
}

impl Library {
    /// The plan to move project `project_id` to profile `profile_id` (at `version`, the current
    /// one when `None`). It holds only the difference from what Harneo wrote: files the profile
    /// adds, files whose content changed (`Replace`) and files it no longer produces (restored
    /// from the project's originals when they replaced one, otherwise deleted). A file edited in
    /// the project since Harneo wrote it, or a file of the project's own that the profile now
    /// adds, is listed in `edited_in_project`; it is still backed up before it is replaced.
    pub fn plan_switch(
        &self,
        project_id: i64,
        profile_id: ProfileId,
        version: Option<u32>,
    ) -> Result<Plan> {
        let view = self.profile(profile_id, version)?;
        self.plan_switch_to(project_id, view)
    }

    /// Like [`Library::plan_switch`] for a profile view that need not be saved: the plan to move
    /// the project to `view.spec`, named as `view.name` at `view.version`.
    pub(crate) fn plan_switch_to(&self, project_id: i64, view: ProfileView) -> Result<Plan> {
        let project = self.live_project(project_id)?;
        let target = project_folder(&project.path)?;
        let profile_id = view.id;
        let rendered = self.render(
            &view.spec,
            &view.name,
            view.version,
            &super::plan::utc_date(crate::library::records::now()),
        )?;
        let installed = self.installed_files(project_id)?;
        let manifest: HashMap<&str, &InstalledFile> =
            installed.iter().map(|f| (f.path.as_str(), f)).collect();
        let produced: HashSet<&str> = rendered.files.iter().map(|f| f.path.as_str()).collect();

        let mut writes = Vec::new();
        let mut edited = Vec::new();
        for file in &rendered.files {
            let disk = current_bytes(&target, &file.path)?;
            let write = |action| Write {
                path: file.path.clone(),
                action,
                bytes: Some(file.bytes.clone()),
                diff: None,
                artifact: match &file.from {
                    PathFrom::Artifact { artifact } => Some(artifact.clone()),
                    PathFrom::Generated { .. } => None,
                },
                executable: file.executable,
                mode: None,
            };
            match manifest.get(file.path.as_str()) {
                Some(known) if known.digest == digest(&file.bytes) => {}
                Some(known) => {
                    if disk.as_ref().is_some_and(|b| digest(b) != known.digest) {
                        edited.push(file.path.clone());
                    }
                    writes.push(write(put_action(disk.is_some())));
                }
                None => {
                    if disk.is_some() {
                        edited.push(file.path.clone());
                    }
                    writes.push(write(put_action(disk.is_some())));
                }
            }
        }

        let gone: Vec<&str> = installed
            .iter()
            .map(|f| f.path.as_str())
            .filter(|p| !produced.contains(p))
            .collect();
        let mut restored = Vec::new();
        let originals = self.originals_of(&project, &gone)?;
        writes.extend(take_out(
            &target,
            &manifest,
            &gone,
            &originals,
            &mut edited,
            &mut restored,
        )?);
        writes.sort_by(|a, b| a.path.cmp(&b.path));
        edited.sort();
        restored.sort();
        let reason = if view.id == project.profile_id {
            format!("Upgrade {} to v{}", view.name, view.version)
        } else {
            format!("Switch to {} v{}", view.name, view.version)
        };
        Ok(Plan {
            warnings: self.plugin_warnings(&view.spec)?,
            target: project.path,
            profile_id,
            reason,
            profile_name: view.name,
            profile_version: view.version,
            backup_count: backup_count(&writes),
            writes,
            kind: PlanKind::Switch,
            edited_in_project: edited,
            restored,
        })
    }
}
