//! Backup sets: the files an apply replaces or deletes, copied aside before anything changes.
//!
//! ```text
//! <data dir>/backups/<project id>/<UTC timestamp>/<project path>   the original bytes
//! <data dir>/backups/<project id>/<UTC timestamp>/backup.json      what the set holds
//! ```

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::apply::{ApplyReport, DataLock, NoHooks};
use super::plan::{digest, resolve_inside, utc_stamp, Plan, PlanKind, Write, WriteAction};
use super::registry;
use crate::error::{Error, Result};
use crate::library::Library;

const BACKUPS_DIR: &str = "backups";
const MANIFEST: &str = "backup.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    path: String,
    /// SHA-256 of the original; `None` when the file did not exist.
    digest: Option<String>,
    existed_before: bool,
    reason: &'static str,
    /// The file was one Harneo manages (listed in the manifest) when it was backed up, so the
    /// copy is not one of the project's originals.
    managed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    project: &'a str,
    created_at: i64,
    reason: &'a str,
    entries: &'a [Entry],
}

/// A backup set on disk.
#[derive(Debug)]
pub(crate) struct BackupSet {
    dir: PathBuf,
    /// Relative to the data folder, as the `backups` table stores it.
    pub(crate) folder: String,
    /// Files copied (what `backups.file_count` counts).
    pub(crate) file_count: u32,
    /// `(project path, original bytes)` of every file the set holds.
    originals: Vec<(String, Vec<u8>)>,
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Writes `bytes` to `path` through a temp file in the same folder, so a reader never sees a
/// half-written file. Keeps the permissions of a file that is replaced.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a file needs a folder"))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".harneo-tmp-")
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(temp.path(), meta.permissions())?;
    }
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Copies every file `plan` replaces or deletes into a new set. `None` when the plan touches no
/// existing file. On any failure the half-made set is removed and nothing else has happened.
pub(crate) fn create(
    data_dir: &Path,
    project_id: i64,
    plan: &Plan,
    target: &Path,
    now: i64,
    managed: &HashSet<String>,
) -> Result<Option<BackupSet>> {
    let mut entries = Vec::new();
    let mut originals = Vec::new();
    for write in &plan.writes {
        let (reason, existing) = match write.action {
            WriteAction::Replace => ("replaced", true),
            WriteAction::Delete => ("deleted", true),
            WriteAction::New => ("created", false),
            // A file of the project that is already what the profile has: nothing is written,
            // but the manifest will track it, so removal must still find it among the
            // originals. Files the manifest already tracks are Harneo's own, not originals.
            WriteAction::Keep if write.bytes.is_some() && !managed.contains(&write.path) => {
                ("kept", true)
            }
            WriteAction::Keep | WriteAction::Skip => continue,
        };
        let path = resolve_inside(target, &write.path)?;
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => {
                let bytes = fs::read(&path).map_err(|e| io_error(&path, e))?;
                entries.push(Entry {
                    path: write.path.clone(),
                    digest: Some(digest(&bytes)),
                    existed_before: true,
                    reason,
                    managed: managed.contains(&write.path),
                });
                originals.push((write.path.clone(), bytes));
            }
            Ok(_) => {
                return Err(Error::Install {
                    message: format!("{} is not a regular file", write.path),
                })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound && !existing => entries.push(Entry {
                path: write.path.clone(),
                digest: None,
                existed_before: false,
                reason,
                managed: managed.contains(&write.path),
            }),
            // A file to replace or delete that vanished since the preview: nothing to keep.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(&path, e)),
        }
    }
    if originals.is_empty() {
        return Ok(None);
    }
    let parent = data_dir.join(BACKUPS_DIR).join(project_id.to_string());
    fs::create_dir_all(&parent).map_err(|e| io_error(&parent, e))?;
    let stamp = utc_stamp(now);
    let (dir, name) = fresh_dir(&parent, &stamp)?;
    let made = (|| -> Result<()> {
        for (rel, bytes) in &originals {
            let dest = rel.split('/').fold(dir.clone(), |acc, part| acc.join(part));
            if let Some(folder) = dest.parent() {
                fs::create_dir_all(folder).map_err(|e| io_error(folder, e))?;
            }
            write_atomic(&dest, bytes).map_err(|e| io_error(&dest, e))?;
            let copied = fs::read(&dest).map_err(|e| io_error(&dest, e))?;
            if copied != *bytes {
                return Err(Error::Install {
                    message: format!("the backup of {rel} did not match the original"),
                });
            }
        }
        let manifest = Manifest {
            project: &plan.target,
            created_at: now,
            reason: &plan.reason,
            entries: &entries,
        };
        let json = serde_json::to_vec_pretty(&manifest)?;
        let file = dir.join(MANIFEST);
        write_atomic(&file, &json).map_err(|e| io_error(&file, e))
    })();
    if let Err(e) = made {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(Some(BackupSet {
        dir,
        folder: format!("{BACKUPS_DIR}/{project_id}/{name}"),
        file_count: u32::try_from(originals.len()).unwrap_or(u32::MAX),
        originals,
    }))
}

/// A new empty folder named after `stamp` under `parent` (a counter is appended when two
/// applies land in the same second).
fn fresh_dir(parent: &Path, stamp: &str) -> Result<(PathBuf, String)> {
    for n in 1..1000 {
        let name = if n == 1 {
            stamp.to_owned()
        } else {
            format!("{stamp}-{n}")
        };
        let dir = parent.join(&name);
        match fs::create_dir(&dir) {
            Ok(()) => return Ok((dir, name)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_error(&dir, e)),
        }
    }
    Err(Error::Install {
        message: "could not make a backup folder".to_owned(),
    })
}

impl BackupSet {
    /// Puts the original bytes of `rel` back (the file is restored from memory, which was
    /// read and checked when the set was made).
    pub(crate) fn restore(&self, target: &Path, rel: &str) -> io::Result<()> {
        let Some((_, bytes)) = self.originals.iter().find(|(p, _)| p == rel) else {
            return Ok(());
        };
        let path = resolve_inside(target, rel).map_err(io::Error::other)?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        write_atomic(&path, bytes)
    }

    /// Deletes the set from disk (used when the apply it was made for was fully undone).
    pub(crate) fn discard(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

// ---- Reading sets back: originals, restore, delete -------------------------------------------

/// One entry of a `backup.json` as read back.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEntry {
    path: String,
    digest: Option<String>,
    existed_before: bool,
    #[serde(default)]
    managed: bool,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Deserialize)]
struct StoredManifest {
    entries: Vec<StoredEntry>,
}

/// A file a backup set holds.
#[derive(Debug)]
pub(crate) struct StoredFile {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
    /// Harneo's own file at the time, not one of the project's originals.
    pub(crate) managed: bool,
}

/// The folder of a set on disk, refusing a stored folder that leaves `<data dir>/backups`.
fn set_dir(data_dir: &Path, folder: &str) -> Result<PathBuf> {
    let parts: Vec<&str> = folder.split('/').collect();
    let plain = parts
        .iter()
        .all(|p| !p.is_empty() && *p != "." && *p != ".." && !p.contains('\\'));
    if parts.first() != Some(&BACKUPS_DIR) || parts.len() < 2 || !plain {
        return Err(Error::Install {
            message: format!("{folder} is not a backup folder"),
        });
    }
    Ok(parts.iter().fold(data_dir.to_path_buf(), |a, p| a.join(p)))
}

/// The entries of the set in `dir`; `None` when the folder or its `backup.json` is gone.
fn read_manifest(dir: &Path) -> Result<Option<Vec<StoredEntry>>> {
    let file = dir.join(MANIFEST);
    match fs::read(&file) {
        Ok(json) => Ok(Some(
            serde_json::from_slice::<StoredManifest>(&json)?.entries,
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(&file, e)),
    }
}

/// The bytes of `entry` from the set in `dir`, checked against the recorded digest.
fn read_file(dir: &Path, entry: &StoredEntry) -> Result<Vec<u8>> {
    let path = entry
        .path
        .split('/')
        .fold(dir.to_path_buf(), |a, p| a.join(p));
    let bytes = fs::read(&path).map_err(|e| io_error(&path, e))?;
    if entry.digest.as_deref() != Some(digest(&bytes).as_str()) {
        return Err(Error::Install {
            message: format!(
                "the backup of {} does not match its record; it was changed since it was made",
                entry.path
            ),
        });
    }
    Ok(bytes)
}

/// Every file the set in `folder` holds (the ones that existed before), verified.
pub(crate) fn read_set(data_dir: &Path, folder: &str) -> Result<Vec<StoredFile>> {
    let dir = set_dir(data_dir, folder)?;
    let entries = read_manifest(&dir)?.ok_or_else(|| Error::Install {
        message: format!("the backup {folder} is no longer on disk"),
    })?;
    let mut files = Vec::new();
    for entry in entries.iter().filter(|e| e.existed_before) {
        files.push(StoredFile {
            path: entry.path.clone(),
            bytes: read_file(&dir, entry)?,
            managed: entry.managed,
        });
    }
    Ok(files)
}

/// The project's originals among `wanted`: for each path, the bytes in the first set (`folders`,
/// newest first) that holds the file as the project had it, not as Harneo wrote it, so the
/// version the project had when Harneo last took the file over comes back. Sets that are gone
/// from disk are skipped.
pub(crate) fn find_originals(
    data_dir: &Path,
    folders: &[String],
    wanted: &HashSet<&str>,
) -> Result<HashMap<String, Vec<u8>>> {
    let mut found = HashMap::new();
    for folder in folders {
        let dir = set_dir(data_dir, folder)?;
        let Some(entries) = read_manifest(&dir)? else {
            continue;
        };
        for entry in entries
            .iter()
            .filter(|e| e.existed_before && !e.managed && wanted.contains(e.path.as_str()))
        {
            if !found.contains_key(&entry.path) {
                found.insert(entry.path.clone(), read_file(&dir, entry)?);
            }
        }
    }
    Ok(found)
}

/// The project's own files a refit deleted: for each path with an unmanaged `deleted` entry
/// (newest set first wins) that `wanted` accepts, the bytes it held.
pub(crate) fn find_deleted_originals(
    data_dir: &Path,
    folders: &[String],
    wanted: impl Fn(&str) -> bool,
) -> Result<HashMap<String, Vec<u8>>> {
    let mut found = HashMap::new();
    for folder in folders {
        let dir = set_dir(data_dir, folder)?;
        let Some(entries) = read_manifest(&dir)? else {
            continue;
        };
        for entry in entries.iter().filter(|e| {
            e.existed_before && !e.managed && e.reason == "deleted" && wanted(e.path.as_str())
        }) {
            if !found.contains_key(&entry.path) {
                found.insert(entry.path.clone(), read_file(&dir, entry)?);
            }
        }
    }
    Ok(found)
}

impl Library {
    /// Puts the files of backup set `backup_id` back into its project. The current versions of
    /// those files go into a new backup set first. A file that was one of Harneo's stays in the
    /// manifest with the restored content; one of the project's originals leaves it.
    pub fn restore_backup(&self, backup_id: i64) -> Result<ApplyReport> {
        let backup = self.backup(backup_id)?;
        let project = self.project(backup.project_id)?;
        let data_dir = self.data_dir()?;
        let target = super::plan::project_folder(&project.path)?;
        let stored = read_set(&data_dir, &backup.folder)?;
        if stored.is_empty() {
            return Err(Error::Install {
                message: "this backup holds no files".to_owned(),
            });
        }
        let manifest: HashMap<String, Option<crate::library::ArtifactId>> = self
            .installed_files(project.id)?
            .into_iter()
            .map(|f| (f.path, f.artifact_id))
            .collect();
        let mut writes = Vec::new();
        let mut restored = Vec::new();
        for file in stored {
            let path = resolve_inside(&target, &file.path)?;
            let action = if fs::symlink_metadata(&path).is_ok() {
                WriteAction::Replace
            } else {
                WriteAction::New
            };
            let artifact = match manifest.get(&file.path) {
                Some(artifact) if file.managed => artifact.clone(),
                _ => {
                    restored.push(file.path.clone());
                    None
                }
            };
            writes.push(Write {
                path: file.path,
                action,
                bytes: Some(file.bytes),
                diff: None,
                artifact,
                executable: false,
            });
        }
        writes.sort_by(|a, b| a.path.cmp(&b.path));
        let backup_count = writes
            .iter()
            .filter(|w| w.action == WriteAction::Replace)
            .count();
        restored.sort();
        let plan = Plan {
            target: project.path,
            profile_id: project.profile_id,
            profile_name: project.profile_name,
            profile_version: project.profile_version,
            reason: format!("Restore: {}", backup.reason),
            writes,
            backup_count: u32::try_from(backup_count).unwrap_or(u32::MAX),
            kind: PlanKind::Restore,
            edited_in_project: Vec::new(),
            restored,
        };
        self.apply_to(&plan, &NoHooks, Some(project.id))
    }

    /// Deletes backup set `backup_id`: its folder and its row. Nothing else changes.
    pub fn delete_backup(&self, backup_id: i64) -> Result<()> {
        let backup = self.backup(backup_id)?;
        let data_dir = self.data_dir()?;
        let dir = set_dir(&data_dir, &backup.folder)?;
        let _lock = DataLock::acquire(&data_dir, super::apply::LOCK_WAIT)?;
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(io_error(&dir, e)),
            _ => {}
        }
        registry::delete_backup_row(&self.store.conn, backup_id)
    }
}
