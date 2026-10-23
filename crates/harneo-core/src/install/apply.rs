//! Applying a plan: lock, back up, write atomically, and undo everything on a failure.

use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::backup::{self, write_atomic, BackupSet};
use super::plan::{check_plan, resolve_inside, Plan, PlanKind, WriteAction};
use super::registry;
use crate::error::{Error, Result};
use crate::library::{make_executable, Library};

const LOCK_FILE: &str = "install.lock";
/// How long an apply waits for another Harneo to finish.
pub(super) const LOCK_WAIT: Duration = Duration::from_secs(10);
const LOCK_POLL: Duration = Duration::from_millis(50);

/// What an apply did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ApplyReport {
    pub project_id: i64,
    /// Files created or replaced.
    pub written: u32,
    pub deleted: u32,
    /// Files copied into the backup set first.
    pub backed_up: u32,
    /// The backup set, when one was made.
    pub backup_id: Option<i64>,
    /// The backup set's folder, relative to the data folder. A removal unregisters its project,
    /// so its set has no row: the folder is where the files are.
    pub backup_folder: Option<String>,
}

/// Failure injection for tests of the rollback. The real library uses [`NoHooks`].
pub(crate) trait ApplyHooks {
    /// Called before each path is written or deleted.
    fn before_write(&self, _path: &str) -> io::Result<()> {
        Ok(())
    }
    /// Called after every file is in place and before the registry transaction commits.
    fn before_commit(&self) -> io::Result<()> {
        Ok(())
    }
    /// How long to wait for the data-dir lock.
    fn lock_wait(&self) -> Duration {
        LOCK_WAIT
    }
}

/// The production hooks: never fail.
pub(crate) struct NoHooks;
impl ApplyHooks for NoHooks {}

/// The data-dir lock, shared by the app and the CLI. Released when dropped.
pub(super) struct DataLock {
    _file: File,
}

impl DataLock {
    pub(super) fn acquire(data_dir: &Path, wait: Duration) -> Result<Self> {
        fs::create_dir_all(data_dir).map_err(|source| io_error(data_dir, source))?;
        let path = data_dir.join(LOCK_FILE);
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|source| io_error(&path, source))?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) if start.elapsed() < wait => sleep(LOCK_POLL),
                Err(TryLockError::WouldBlock) => return Err(Error::Busy),
                Err(TryLockError::Error(source)) => return Err(io_error(&path, source)),
            }
        }
    }
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// What this apply has done to the project so far, so it can be undone.
#[derive(Default)]
struct Journal {
    /// Paths of `Replace` and `Delete` rows that were started.
    changed: Vec<String>,
    /// Paths of `New` rows that were started (the file did not exist).
    created: Vec<PathBuf>,
    /// Folders this apply made, in creation order.
    folders: Vec<PathBuf>,
}

impl Journal {
    /// Makes `folder` and its missing parents, remembering the ones made.
    fn make_folders(&mut self, folder: &Path, target: &Path) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut at = folder;
        while at != target && !at.exists() {
            missing.push(at.to_path_buf());
            match at.parent() {
                Some(parent) => at = parent,
                None => break,
            }
        }
        for dir in missing.into_iter().rev() {
            fs::create_dir(&dir)?;
            self.folders.push(dir);
        }
        Ok(())
    }
}

/// Refuses a plan whose files changed since it was made: a `New` file that now exists, or a
/// file to replace or delete that is no regular file.
fn check_state(plan: &Plan, target: &Path) -> Result<()> {
    for write in &plan.writes {
        let path = resolve_inside(target, &write.path)?;
        let meta = fs::symlink_metadata(&path).ok();
        match (write.action, meta) {
            (WriteAction::New, Some(_)) => {
                return Err(Error::Install {
                    message: format!("{} appeared after the preview; preview again", write.path),
                })
            }
            (WriteAction::Replace | WriteAction::Delete, Some(m)) if !m.is_file() => {
                return Err(Error::Install {
                    message: format!("{} is not a regular file", write.path),
                })
            }
            _ => {}
        }
    }
    Ok(())
}

fn write_all(
    plan: &Plan,
    target: &Path,
    hooks: &dyn ApplyHooks,
    journal: &mut Journal,
) -> Result<()> {
    for write in &plan.writes {
        let path = resolve_inside(target, &write.path)?;
        let step = |e: io::Error| io_error(&path, e);
        match (write.action, &write.bytes) {
            (WriteAction::New | WriteAction::Replace, Some(bytes)) => {
                hooks.before_write(&write.path).map_err(step)?;
                if write.action == WriteAction::New {
                    journal.created.push(path.clone());
                } else {
                    journal.changed.push(write.path.clone());
                }
                if let Some(folder) = path.parent() {
                    journal.make_folders(folder, target).map_err(step)?;
                }
                write_atomic(&path, bytes).map_err(step)?;
                if write.executable {
                    make_executable(&path).map_err(step)?;
                }
            }
            (WriteAction::Delete, _) => {
                hooks.before_write(&write.path).map_err(step)?;
                journal.changed.push(write.path.clone());
                match fs::remove_file(&path) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(step(e)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if matches!(plan.kind, PlanKind::Switch | PlanKind::Removal) {
        prune_empty_folders(plan, target);
    }
    Ok(())
}

/// Removes the folders a switch or removal emptied, from the deleted files up to the project
/// root. A rollback puts the folders back with the files.
fn prune_empty_folders(plan: &Plan, target: &Path) {
    for write in plan
        .writes
        .iter()
        .filter(|w| w.action == WriteAction::Delete)
    {
        let mut at = resolve_inside(target, &write.path)
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        while let Some(dir) = at {
            if dir == target || fs::remove_dir(&dir).is_err() {
                break;
            }
            at = dir.parent().map(Path::to_path_buf);
        }
    }
}

/// Puts the project back as it was. Returns what could not be undone.
fn undo(set: Option<&BackupSet>, target: &Path, journal: &Journal) -> Vec<String> {
    let mut left = Vec::new();
    for rel in &journal.changed {
        if let Some(set) = set {
            if let Err(e) = set.restore(target, rel) {
                left.push(format!("{rel}: {e}"));
            }
        }
    }
    for path in &journal.created {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                left.push(format!("{}: {e}", path.display()));
            }
            _ => {}
        }
    }
    for dir in journal.folders.iter().rev() {
        let _ = fs::remove_dir(dir);
    }
    left
}

impl Library {
    /// The data folder (the parent of `library/`).
    pub(crate) fn data_dir(&self) -> Result<PathBuf> {
        self.store
            .root()
            .parent()
            .map(Path::to_path_buf)
            .ok_or(Error::NoDataDir)
    }

    /// Writes `plan` into its project. Every file that is replaced or deleted is backed up
    /// first; a failure part-way puts the files back, removes what this apply created and
    /// returns the error. The project and its manifest are recorded only when all of it worked.
    pub fn apply(&self, plan: &Plan) -> Result<ApplyReport> {
        self.apply_with(plan, &NoHooks)
    }

    pub(crate) fn apply_with(&self, plan: &Plan, hooks: &dyn ApplyHooks) -> Result<ApplyReport> {
        let data_dir = self.data_dir()?;
        let target = check_plan(plan)?;
        let _lock = DataLock::acquire(&data_dir, hooks.lock_wait())?;
        check_state(plan, &target)?;
        let now = crate::library::records::now();
        let tx = self.store.conn.unchecked_transaction()?;
        let project_id = registry::project_for(&tx, plan, now)?;
        let managed = registry::manifest_paths(&tx, project_id)?;
        let set = backup::create(&data_dir, project_id, plan, &target, now, &managed)?;
        let mut journal = Journal::default();
        let outcome = write_all(plan, &target, hooks, &mut journal).and_then(|()| {
            hooks
                .before_commit()
                .map_err(|e| io_error(&target, e))
                .and_then(|()| registry::record(&tx, project_id, plan, set.as_ref(), now))
        });
        match outcome {
            Ok(backup_id) => {
                if let Err(e) = tx.commit() {
                    return Err(rolled_back(
                        e.into(),
                        undo(set.as_ref(), &target, &journal),
                        set.as_ref(),
                    ));
                }
                Ok(report(plan, project_id, set.as_ref(), backup_id))
            }
            Err(e) => {
                drop(tx);
                Err(rolled_back(
                    e,
                    undo(set.as_ref(), &target, &journal),
                    set.as_ref(),
                ))
            }
        }
    }
}

fn report(
    plan: &Plan,
    project_id: i64,
    set: Option<&BackupSet>,
    backup_id: Option<i64>,
) -> ApplyReport {
    let count = |f: fn(WriteAction) -> bool| {
        u32::try_from(plan.writes.iter().filter(|w| f(w.action)).count()).unwrap_or(u32::MAX)
    };
    ApplyReport {
        project_id,
        written: count(|a| matches!(a, WriteAction::New | WriteAction::Replace)),
        deleted: count(|a| a == WriteAction::Delete),
        backed_up: set.map_or(0, |s| s.file_count),
        backup_id,
        backup_folder: set.map(|s| s.folder.clone()),
    }
}

/// The error to return after an undo: the cause, and whether the project is back as it was.
/// The backup set is kept only when something could not be put back.
fn rolled_back(cause: Error, left: Vec<String>, set: Option<&BackupSet>) -> Error {
    if left.is_empty() {
        if let Some(set) = set {
            set.discard();
        }
        return Error::InstallFailed {
            message: format!("{cause}. Rolled back: nothing was changed."),
        };
    }
    let folder = set.map_or(String::new(), |s| {
        format!(" The originals are in the backup {}.", s.folder)
    });
    Error::InstallFailed {
        message: format!(
            "{cause}. Rolling back failed for: {}.{folder}",
            left.join("; ")
        ),
    }
}
