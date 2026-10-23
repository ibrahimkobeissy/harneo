use std::fs;
use std::os::unix::fs::symlink;

use super::fixture::{fx, plan, write, TestResult};
use super::registry::MAX_FILE_BYTES;
use super::WriteAction::New;
use crate::error::Error;

#[test]
fn a_profile_in_use_cannot_be_deleted_and_its_projects_are_named() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    assert_eq!(f.library.profiles()?[0].project_count, 0);
    let p = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    f.library.apply(&p)?;
    let name = dir
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or("name")?;
    let error = f.library.delete_profile(f.profile).err().ok_or("refused")?;
    assert!(matches!(error, Error::ProfileInUse { .. }));
    assert!(error.to_string().contains(&name), "{error}");
    assert_eq!(f.library.profiles()?.len(), 1);
    Ok(())
}

#[test]
fn unknown_projects_are_refused() -> TestResult {
    let f = fx()?;
    assert!(matches!(
        f.library.project(7),
        Err(Error::ProjectNotFound { id: 7 })
    ));
    assert!(f.library.installed_files(7).is_err());
    assert!(f.library.backups(7).is_err());
    assert!(f.library.project_file_content(7, "a").is_err());
    Ok(())
}

#[test]
fn a_managed_file_is_read_back_and_others_are_not() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("mine.txt"), "not managed")?;
    let p = plan(&f, dir.path(), vec![write("a.txt", New, Some("héllo\n"))])?;
    let id = f.library.apply(&p)?.project_id;
    let file = f.library.project_file_content(id, "a.txt")?;
    assert_eq!((file.content.as_str(), file.truncated), ("héllo\n", false));
    assert!(f.library.project_file_content(id, "mine.txt").is_err());
    assert!(f.library.project_file_content(id, "../etc/passwd").is_err());
    Ok(())
}

#[test]
fn a_long_file_is_cut_at_the_cap() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let big = "x".repeat(usize::try_from(MAX_FILE_BYTES)? + 10);
    let p = plan(&f, dir.path(), vec![write("big.txt", New, Some(&big))])?;
    let id = f.library.apply(&p)?.project_id;
    let file = f.library.project_file_content(id, "big.txt")?;
    assert!(file.truncated);
    assert_eq!(file.content.len() as u64, MAX_FILE_BYTES);
    Ok(())
}

#[test]
fn a_managed_file_that_became_a_symlink_is_not_followed() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    fs::write(outside.path().join("secret.txt"), "outside")?;
    let p = plan(&f, dir.path(), vec![write("a.txt", New, Some("x"))])?;
    let id = f.library.apply(&p)?.project_id;
    fs::remove_file(dir.path().join("a.txt"))?;
    symlink(outside.path().join("secret.txt"), dir.path().join("a.txt"))?;
    assert!(matches!(
        f.library.project_file_content(id, "a.txt"),
        Err(Error::UnsafePath { .. })
    ));
    Ok(())
}
