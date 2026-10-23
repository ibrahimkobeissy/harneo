use std::fs;

use super::fixture::{fx, put, TestResult};
use super::fixture_lifecycle::{install_over, read, without_agent};
use super::PlanKind;
use crate::error::Error;

const AGENT: &str = ".claude/agents/rev.md";

#[test]
fn restoring_a_set_backs_up_the_current_versions_first() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let original_set = f.library.backups(report.project_id)?[0].clone();
    assert_eq!(original_set.file_count, 1);
    fs::write(dir.path().join("CLAUDE.md"), "# edited since\n")?;

    let done = f.library.restore_backup(original_set.id)?;
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# my own\n");
    assert_eq!((done.written, done.backed_up), (1, 1));
    let backups = f.library.backups(report.project_id)?;
    assert_eq!(backups.len(), 2);
    let newest = &backups[0];
    assert_eq!(Some(newest.id), done.backup_id);
    assert_eq!(newest.reason, format!("Restore: {}", original_set.reason));
    assert_eq!(
        read(&f.data.path().join(&newest.folder), "CLAUDE.md")?,
        b"# edited since\n"
    );
    // The file is the project's own again: it left the manifest, and the project is unchanged.
    let manifest = f.library.installed_files(report.project_id)?;
    assert!(!manifest.iter().any(|m| m.path == "CLAUDE.md"));
    let project = f.library.project(report.project_id)?;
    assert_eq!(project.applied_at, f.library.projects()?[0].applied_at);
    Ok(())
}

#[test]
fn restoring_harnesss_own_version_keeps_it_in_the_manifest_with_the_restored_digest() -> TestResult
{
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let old_harness = read(dir.path(), "HARNESS.md")?;
    without_agent(&f)?;
    let switch = f.library.plan_switch(report.project_id, f.profile, None)?;
    let applied = f.library.apply(&switch)?;
    let set = applied.backup_id.ok_or("set")?;
    assert_ne!(read(dir.path(), "HARNESS.md")?, old_harness);

    f.library.restore_backup(set)?;
    assert_eq!(read(dir.path(), "HARNESS.md")?, old_harness);
    assert!(!read(dir.path(), AGENT)?.is_empty());
    let manifest = f.library.installed_files(report.project_id)?;
    let harness = manifest
        .iter()
        .find(|m| m.path == "HARNESS.md")
        .ok_or("row")?;
    assert_eq!(harness.digest, super::plan::digest(&old_harness));
    // The restored agent was removed from the manifest by the switch, so it is untracked now.
    assert!(!manifest.iter().any(|m| m.path == AGENT));
    // A removal afterwards still deletes the managed file and leaves the untracked one.
    let removal = f.library.plan_removal(report.project_id)?;
    f.library.apply(&removal)?;
    assert!(!dir.path().join("HARNESS.md").exists());
    assert!(dir.path().join(AGENT).exists());
    Ok(())
}

#[test]
fn restoring_recreates_a_file_that_is_gone() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    fs::remove_file(dir.path().join("CLAUDE.md"))?;
    let set = f.library.backups(report.project_id)?[0].id;
    let done = f.library.restore_backup(set)?;
    assert_eq!(done.backed_up, 0);
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# my own\n");
    Ok(())
}

#[test]
fn a_changed_backup_file_is_never_restored() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let set = f.library.backups(report.project_id)?[0].clone();
    fs::write(
        f.data.path().join(&set.folder).join("CLAUDE.md"),
        "tampered\n",
    )?;
    let error = f.library.restore_backup(set.id).err().ok_or("refused")?;
    assert!(error.to_string().contains("does not match"), "{error}");
    assert_eq!(f.library.backups(report.project_id)?.len(), 1);
    // Removal refuses too rather than writing a wrong "original".
    assert!(f.library.plan_removal(report.project_id).is_err());
    Ok(())
}

#[test]
fn unknown_missing_and_escaping_backups_are_refused() -> TestResult {
    let f = fx()?;
    assert!(matches!(
        f.library.restore_backup(4),
        Err(Error::BackupNotFound { id: 4 })
    ));
    assert!(matches!(
        f.library.delete_backup(4),
        Err(Error::BackupNotFound { id: 4 })
    ));
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let set = f.library.backups(report.project_id)?[0].clone();
    fs::remove_dir_all(f.data.path().join(&set.folder))?;
    assert!(matches!(
        f.library.restore_backup(set.id),
        Err(Error::Install { .. })
    ));
    // A stored folder that leaves the backups folder is never followed.
    f.library.store.conn.execute(
        "UPDATE backups SET folder = '../outside' WHERE id = ?1",
        [set.id],
    )?;
    assert!(matches!(
        f.library.restore_backup(set.id),
        Err(Error::Install { .. })
    ));
    assert!(matches!(
        f.library.delete_backup(set.id),
        Err(Error::Install { .. })
    ));
    Ok(())
}

#[test]
fn deleting_a_set_removes_its_folder_and_row_only() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let set = f.library.backups(report.project_id)?[0].clone();
    let folder = f.data.path().join(&set.folder);
    assert!(folder.exists());
    f.library.delete_backup(set.id)?;
    assert!(!folder.exists());
    assert!(f.library.backups(report.project_id)?.is_empty());
    assert!(dir.path().join("CLAUDE.md").exists());
    // Deleting twice says so; a set whose folder is already gone is still dropped.
    assert!(f.library.delete_backup(set.id).is_err());
    Ok(())
}

#[test]
fn a_set_with_no_files_cannot_be_restored() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let set = f.library.backups(report.project_id)?[0].clone();
    let manifest = f.data.path().join(&set.folder).join("backup.json");
    fs::write(&manifest, r#"{"entries":[]}"#)?;
    assert!(matches!(
        f.library.restore_backup(set.id),
        Err(Error::Install { .. })
    ));
    // A restore plan applied to a project that is not registered is refused.
    assert_eq!(PlanKind::default(), PlanKind::Scaffold);
    Ok(())
}
