//! Backup sets: the files an apply replaces or deletes, copied aside before anything changes.
//!
//! ```text
//! <data dir>/backups/<project id>/<UTC timestamp>/<project path>   the original bytes
//! <data dir>/backups/<project id>/<UTC timestamp>/backup.json      what the set holds
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::plan::{digest, resolve_inside, utc_stamp, Plan, WriteAction};
use crate::error::{Error, Result};

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
) -> Result<Option<BackupSet>> {
    let mut entries = Vec::new();
    let mut originals = Vec::new();
    for write in &plan.writes {
        let (reason, existing) = match write.action {
            WriteAction::Replace => ("replaced", true),
            WriteAction::Delete => ("deleted", true),
            WriteAction::New => ("created", false),
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
