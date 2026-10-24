//! Reset: empties Harneo's data folder and recreates an empty library. Only ever deletes
//! inside the data folder; project folders and `~/.claude` are never touched.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::apply::{DataLock, LOCK_FILE, LOCK_WAIT};
use crate::error::{Error, Result};
use crate::library::Library;
use crate::usage::UsageIndex;

const BACKUPS_DIR: &str = "backups";

/// Refuses a data folder that is a root, the home folder or a git checkout.
fn check_safe(data_dir: &Path) -> Result<()> {
    let refuse = |why: &str| {
        Err(Error::Reset {
            message: format!(
                "refusing to reset {}: {why}. Nothing was deleted.",
                data_dir.display()
            ),
        })
    };
    if data_dir.parent().is_none() || data_dir.components().count() < 2 {
        return refuse("it is a root folder");
    }
    let canonical = fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    if canonical.parent().is_none() {
        return refuse("it is a root folder");
    }
    let home = dirs::home_dir().and_then(|h| fs::canonicalize(h).ok());
    if home.as_deref() == Some(canonical.as_path()) {
        return refuse("it is the home folder");
    }
    if canonical.join(".git").exists() {
        return refuse("it contains a .git folder, so it looks like a project");
    }
    // Only Harneo's own folder: it holds the library database, or nothing but the lock file.
    let has_files = fs::read_dir(&canonical)
        .map(|mut entries| entries.any(|e| e.is_ok_and(|e| e.file_name() != LOCK_FILE)))
        .unwrap_or(false);
    if has_files && !canonical.join("library").join("library.db").is_file() {
        return refuse("it holds no Harneo library, so it is not Harneo's data folder");
    }
    Ok(())
}

/// Deletes everything Harneo keeps in `data_dir` (library, profiles, project registry and
/// drift data, usage index, settings) and recreates an empty library and usage index. Backups
/// are kept unless `delete_backups`. Holds the data-dir lock throughout. A failure part-way
/// names what was and was not deleted. Callers must close their open library first.
pub fn reset_data(data_dir: &Path, delete_backups: bool) -> Result<()> {
    check_safe(data_dir)?;
    if !data_dir.exists() {
        Library::open(data_dir)?;
        UsageIndex::open(data_dir)?;
        return Ok(());
    }
    let _lock = DataLock::acquire(data_dir, LOCK_WAIT)?;
    let entries = fs::read_dir(data_dir).map_err(|source| Error::Io {
        path: data_dir.display().to_string(),
        source,
    })?;
    let mut deleted = Vec::new();
    let mut failed = Vec::new();
    let mut kept = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: data_dir.display().to_string(),
            source,
        })?;
        let name = entry.file_name();
        let label = name.to_string_lossy().into_owned();
        if name == OsStr::new(LOCK_FILE) {
            continue;
        }
        if name == OsStr::new(BACKUPS_DIR) && !delete_backups {
            kept.push(label);
            continue;
        }
        let path = entry.path();
        let is_dir = fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir());
        let result = if is_dir {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        match result {
            Ok(()) => deleted.push(label),
            Err(err) => failed.push(format!("{label} ({err})")),
        }
    }
    if !failed.is_empty() {
        return Err(Error::Reset {
            message: format!(
                "reset stopped part-way. Deleted: {}. Could not delete: {}. Kept: {}.",
                list(&deleted),
                failed.join(", "),
                list(&kept),
            ),
        });
    }
    Library::open(data_dir)?;
    UsageIndex::open(data_dir)?;
    Ok(())
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "nothing".to_string()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::fixture::{fx, put, TestResult};
    use crate::install::fixture_lifecycle::install_over;

    #[test]
    fn reset_leaves_an_empty_library_keeps_backups_and_spares_projects() -> TestResult {
        let f = fx()?;
        let project = tempfile::tempdir()?;
        put(project.path(), "CLAUDE.md", "# my own\n")?;
        install_over(&f, project.path(), f.profile)?;
        let after_install = fs::read(project.path().join("CLAUDE.md"))?;
        let data = f.data.path().to_path_buf();
        assert!(data.join("backups").exists());
        drop(f.library);

        reset_data(&data, false)?;
        let library = Library::open(&data)?;
        assert!(library.profiles()?.is_empty());
        assert!(library.projects()?.is_empty());
        assert_eq!(library.list(&Default::default())?.total, 0);
        assert!(data.join("backups").exists());
        assert_eq!(fs::read(project.path().join("CLAUDE.md"))?, after_install);
        assert!(data.join("usage").exists());
        Ok(())
    }

    #[test]
    fn also_deleting_backups_removes_them() -> TestResult {
        let f = fx()?;
        let project = tempfile::tempdir()?;
        put(project.path(), "CLAUDE.md", "# my own\n")?;
        install_over(&f, project.path(), f.profile)?;
        let data = f.data.path().to_path_buf();
        drop(f.library);
        reset_data(&data, true)?;
        assert!(!data.join("backups").exists());
        assert!(project.path().join("CLAUDE.md").exists());
        Ok(())
    }

    #[test]
    fn refuses_a_root_a_git_checkout_and_a_busy_data_dir() -> TestResult {
        assert!(matches!(
            reset_data(Path::new("/"), true),
            Err(Error::Reset { .. })
        ));
        let repo = tempfile::tempdir()?;
        fs::create_dir(repo.path().join(".git"))?;
        put(repo.path(), "keep.txt", "x")?;
        assert!(matches!(
            reset_data(repo.path(), true),
            Err(Error::Reset { .. })
        ));
        assert!(repo.path().join("keep.txt").exists());

        // A folder with files but no Harneo library is someone else's: left alone.
        let other = tempfile::tempdir()?;
        put(other.path(), "notes/todo.md", "x")?;
        assert!(matches!(
            reset_data(other.path(), true),
            Err(Error::Reset { .. })
        ));
        assert!(other.path().join("notes/todo.md").exists());
        Ok(())
    }

    #[test]
    fn a_missing_data_dir_becomes_an_empty_library() -> TestResult {
        let base = tempfile::tempdir()?;
        let data = base.path().join("fresh");
        reset_data(&data, false)?;
        assert!(Library::open(&data)?.profiles()?.is_empty());
        Ok(())
    }

    #[test]
    fn a_failure_names_what_was_and_was_not_deleted() -> TestResult {
        let data = tempfile::tempdir()?;
        let base = data.path().join("d");
        drop(Library::open(&base)?);
        put(&base, "settings.txt", "x")?;
        // A folder that cannot be listed for removal (unix only): read-only parent is the
        // folder itself, so make a child dir we cannot enter.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let locked = base.join("locked");
            fs::create_dir(&locked)?;
            put(&locked, "inner", "x")?;
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))?;
            let err = reset_data(&base, false);
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))?;
            // Running as root can delete anything; otherwise the failure is reported.
            if let Err(Error::Reset { message }) = err {
                assert!(message.contains("locked") && message.contains("settings.txt"));
            }
        }
        Ok(())
    }
}
