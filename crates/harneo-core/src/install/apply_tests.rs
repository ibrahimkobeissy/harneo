use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use super::apply::ApplyHooks;
use super::fixture::{fx, plan, put, write, Fx, TestResult};
use super::WriteAction::{Delete, New, Replace};
use crate::error::Error;

fn read(root: &Path, rel: &str) -> io::Result<String> {
    fs::read_to_string(root.join(rel))
}

fn files_under(root: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

struct FailAt(&'static str);
impl ApplyHooks for FailAt {
    fn before_write(&self, path: &str) -> io::Result<()> {
        if path == self.0 {
            return Err(io::Error::other("disk full"));
        }
        Ok(())
    }
}

struct FailCommit;
impl ApplyHooks for FailCommit {
    fn before_commit(&self) -> io::Result<()> {
        Err(io::Error::other("no commit"))
    }
}

struct ShortWait;
impl ApplyHooks for ShortWait {
    fn lock_wait(&self) -> Duration {
        Duration::from_millis(150)
    }
}

/// A project with two files of its own, and a plan that replaces one, deletes one and adds one
/// in a new folder tree.
fn mixed(
    f: &Fx,
) -> std::result::Result<(tempfile::TempDir, super::Plan), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    put(dir.path(), "a.txt", "original a\n")?;
    put(dir.path(), "b.txt", "original b\n")?;
    put(dir.path(), "CLAUDE.local.md", "my private notes\n")?;
    let p = plan(
        f,
        dir.path(),
        vec![
            write("a.txt", Replace, Some("new a\n")),
            write("b.txt", Delete, None),
            write("deep/er/c.txt", New, Some("new c\n")),
        ],
    )?;
    Ok((dir, p))
}

#[test]
fn scaffold_writes_exactly_the_plan_and_registers_the_project() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.local.md", "mine\n")?;
    put(dir.path(), "src/main.rs", "fn main() {}\n")?;
    let p = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    let report = f.library.apply(&p)?;
    let mut expected: Vec<String> = p.writes.iter().map(|w| w.path.clone()).collect();
    expected.extend(["CLAUDE.local.md".to_owned(), "src/main.rs".to_owned()]);
    expected.sort();
    assert_eq!(files_under(dir.path()), expected);
    for w in &p.writes {
        assert_eq!(
            fs::read(dir.path().join(&w.path))?,
            w.bytes.clone().unwrap_or_default()
        );
    }
    assert_eq!(read(dir.path(), "CLAUDE.local.md")?, "mine\n");
    assert_eq!(
        (report.deleted, report.backed_up, report.backup_id),
        (0, 0, None)
    );
    assert_eq!(report.written as usize, p.writes.len());

    let project = f.library.project(report.project_id)?;
    assert_eq!(project.path, p.target);
    assert_eq!(
        (project.profile_id, project.profile_version),
        (f.profile, 1)
    );
    assert_eq!(project.file_count as usize, p.writes.len());
    assert_eq!(f.library.projects()?, vec![project]);
    let manifest = f.library.installed_files(report.project_id)?;
    assert_eq!(manifest.len(), p.writes.len());
    let skill = manifest
        .iter()
        .find(|m| m.path == ".claude/skills/s/SKILL.md")
        .ok_or("skill in manifest")?;
    assert!(skill.artifact_id.is_some());
    assert_eq!(skill.digest.len(), 64);
    assert!(manifest
        .iter()
        .any(|m| m.path == "HARNESS.md" && m.artifact_id.is_none()));
    assert!(f.library.backups(report.project_id)?.is_empty());
    assert_eq!(f.library.profiles()?[0].project_count, 1);
    Ok(())
}

#[test]
fn replaced_and_deleted_files_are_backed_up_before_the_write() -> TestResult {
    let f = fx()?;
    let (dir, p) = mixed(&f)?;
    let report = f.library.apply(&p)?;
    assert_eq!(
        (report.written, report.deleted, report.backed_up),
        (2, 1, 2)
    );
    assert_eq!(read(dir.path(), "a.txt")?, "new a\n");
    assert!(!dir.path().join("b.txt").exists());
    assert_eq!(read(dir.path(), "deep/er/c.txt")?, "new c\n");

    let backups = f.library.backups(report.project_id)?;
    assert_eq!(backups.len(), 1);
    assert_eq!(Some(backups[0].id), report.backup_id);
    assert_eq!(
        (backups[0].file_count, backups[0].reason.as_str()),
        (2, "test")
    );
    let set = f.data.path().join(&backups[0].folder);
    assert_eq!(read(&set, "a.txt")?, "original a\n");
    assert_eq!(read(&set, "b.txt")?, "original b\n");
    let json: serde_json::Value = serde_json::from_str(&read(&set, "backup.json")?)?;
    let entries = json["entries"].as_array().ok_or("entries")?;
    assert_eq!(entries.len(), 3);
    let a = entries.iter().find(|e| e["path"] == "a.txt").ok_or("a")?;
    assert_eq!(a["existedBefore"], true);
    assert_eq!(a["reason"], "replaced");
    assert_eq!(a["digest"].as_str().map(str::len), Some(64));
    let c = entries
        .iter()
        .find(|e| e["path"] == "deep/er/c.txt")
        .ok_or("c")?;
    assert_eq!(c["existedBefore"], false);
    // The manifest keeps the new files only.
    let paths: Vec<String> = f
        .library
        .installed_files(report.project_id)?
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(paths, ["a.txt", "deep/er/c.txt"]);
    Ok(())
}

#[test]
fn an_injected_write_failure_restores_the_original_bytes() -> TestResult {
    let f = fx()?;
    let (dir, p) = mixed(&f)?;
    let error = f
        .library
        .apply_with(&p, &FailAt("deep/er/c.txt"))
        .err()
        .ok_or("must fail")?;
    assert!(matches!(&error, Error::InstallFailed { .. }), "{error:?}");
    assert!(error.to_string().contains("Rolled back"));
    assert_eq!(read(dir.path(), "a.txt")?, "original a\n");
    assert_eq!(read(dir.path(), "b.txt")?, "original b\n");
    assert_eq!(
        files_under(dir.path()),
        ["CLAUDE.local.md", "a.txt", "b.txt"]
    );
    assert!(
        !dir.path().join("deep").exists(),
        "made folders are removed"
    );
    assert!(f.library.projects()?.is_empty());
    let backups = f.data.path().join("backups");
    assert!(!backups.exists() || files_under(&backups).is_empty());
    Ok(())
}

#[test]
fn a_failure_on_the_first_and_on_the_replace_step_restores_too() -> TestResult {
    let f = fx()?;
    for failing in ["a.txt", "b.txt"] {
        let (dir, p) = mixed(&f)?;
        assert!(f.library.apply_with(&p, &FailAt(failing)).is_err());
        assert_eq!(read(dir.path(), "a.txt")?, "original a\n");
        assert_eq!(read(dir.path(), "b.txt")?, "original b\n");
        assert!(!dir.path().join("deep").exists());
    }
    Ok(())
}

#[test]
fn a_failure_at_commit_undoes_the_files_and_the_registry() -> TestResult {
    let f = fx()?;
    let (dir, p) = mixed(&f)?;
    assert!(f.library.apply_with(&p, &FailCommit).is_err());
    assert_eq!(read(dir.path(), "a.txt")?, "original a\n");
    assert_eq!(read(dir.path(), "b.txt")?, "original b\n");
    assert!(!dir.path().join("deep").exists());
    assert!(f.library.projects()?.is_empty());
    // The same plan still applies afterwards.
    f.library.apply(&p)?;
    assert_eq!(read(dir.path(), "a.txt")?, "new a\n");
    Ok(())
}

#[test]
fn a_failed_backup_writes_nothing() -> TestResult {
    let f = fx()?;
    let (dir, p) = mixed(&f)?;
    fs::write(f.data.path().join("backups"), "in the way")?;
    let error = f.library.apply(&p).err().ok_or("must fail")?;
    assert!(matches!(error, Error::Io { .. }), "{error:?}");
    assert_eq!(
        files_under(dir.path()),
        ["CLAUDE.local.md", "a.txt", "b.txt"]
    );
    assert_eq!(read(dir.path(), "a.txt")?, "original a\n");
    assert!(f.library.projects()?.is_empty());
    Ok(())
}

#[test]
fn a_symlinked_folder_in_the_path_refuses_the_whole_plan() -> TestResult {
    let f = fx()?;
    let outside = tempfile::tempdir()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "a.txt", "keep\n")?;
    symlink(outside.path(), dir.path().join("link"))?;
    let p = plan(
        &f,
        dir.path(),
        vec![
            write("a.txt", Replace, Some("changed\n")),
            write("link/x.txt", New, Some("escape\n")),
        ],
    )?;
    assert!(matches!(f.library.apply(&p), Err(Error::UnsafePath { .. })));
    assert!(files_under(outside.path()).is_empty());
    assert_eq!(
        read(dir.path(), "a.txt")?,
        "keep\n",
        "refused before any write"
    );
    Ok(())
}

#[test]
fn a_symlinked_file_is_never_written_or_deleted() -> TestResult {
    let f = fx()?;
    let outside = tempfile::tempdir()?;
    put(outside.path(), "real.txt", "outside\n")?;
    let dir = tempfile::tempdir()?;
    symlink(outside.path().join("real.txt"), dir.path().join("a.txt"))?;
    for action in [Replace, Delete] {
        let bytes = (action == Replace).then_some("x\n");
        let p = plan(&f, dir.path(), vec![write("a.txt", action, bytes)])?;
        assert!(matches!(f.library.apply(&p), Err(Error::UnsafePath { .. })));
    }
    assert_eq!(read(outside.path(), "real.txt")?, "outside\n");
    Ok(())
}

#[test]
fn local_files_and_git_are_never_touched() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let local: [(&str, &str); 3] = [
        ("CLAUDE.local.md", "private\n"),
        (".claude/settings.local.json", "{\"a\":1}\n"),
        (".git/config", "[core]\n"),
    ];
    for (rel, text) in local {
        put(dir.path(), rel, text)?;
    }
    for rel in [
        "CLAUDE.local.md",
        ".claude/settings.local.json",
        ".git/config",
        ".git/hooks/x",
    ] {
        for action in [New, Replace, Delete] {
            let bytes = (action != Delete).then_some("overwritten\n");
            let p = plan(&f, dir.path(), vec![write(rel, action, bytes)])?;
            assert!(f.library.apply(&p).is_err(), "{rel} {action:?}");
        }
    }
    for (rel, text) in local {
        assert_eq!(read(dir.path(), rel)?, text);
    }
    assert!(!dir.path().join(".git/hooks").exists());
    Ok(())
}

#[test]
fn escaping_and_duplicate_paths_are_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    for bad in ["../up.txt", "/abs.txt", "a\\b.txt"] {
        let p = plan(&f, dir.path(), vec![write(bad, New, Some("x"))])?;
        assert!(f.library.apply(&p).is_err(), "{bad}");
    }
    let twice = plan(
        &f,
        dir.path(),
        vec![
            write("a.txt", New, Some("1")),
            write("A.txt", New, Some("2")),
        ],
    )?;
    assert!(f.library.apply(&twice).is_err());
    let mismatch = plan(&f, dir.path(), vec![write("a.txt", New, None)])?;
    assert!(f.library.apply(&mismatch).is_err());
    assert!(files_under(dir.path()).is_empty());
    Ok(())
}

#[test]
fn a_target_that_is_not_the_canonical_folder_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let mut p = plan(&f, dir.path(), vec![write("a.txt", New, Some("x"))])?;
    p.target = format!("{}/../{}", p.target, "nowhere");
    assert!(matches!(f.library.apply(&p), Err(Error::Install { .. })));
    Ok(())
}

#[test]
fn a_new_file_that_appeared_after_the_preview_is_not_overwritten() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let p = plan(&f, dir.path(), vec![write("a.txt", New, Some("harneo\n"))])?;
    put(dir.path(), "a.txt", "user wrote this\n")?;
    assert!(matches!(f.library.apply(&p), Err(Error::Install { .. })));
    assert_eq!(read(dir.path(), "a.txt")?, "user wrote this\n");
    let folder = tempfile::tempdir()?;
    fs::create_dir(folder.path().join("d"))?;
    let p = plan(&f, folder.path(), vec![write("d", Replace, Some("x"))])?;
    assert!(f.library.apply(&p).is_err());
    Ok(())
}

#[test]
fn a_busy_lock_is_reported_and_nothing_is_written() -> TestResult {
    let f = fx()?;
    let (dir, p) = mixed(&f)?;
    let lock = fs::File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(f.data.path().join("install.lock"))?;
    lock.lock()?;
    let error = f
        .library
        .apply_with(&p, &ShortWait)
        .err()
        .ok_or("must fail")?;
    assert!(matches!(error, Error::Busy));
    assert!(error.to_string().contains("Harneo is busy"));
    assert_eq!(read(dir.path(), "a.txt")?, "original a\n");
    drop(lock);
    f.library.apply_with(&p, &ShortWait)?;
    assert_eq!(read(dir.path(), "a.txt")?, "new a\n");
    Ok(())
}

#[test]
fn replacing_keeps_the_file_mode() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "run.sh", "old\n")?;
    fs::set_permissions(dir.path().join("run.sh"), fs::Permissions::from_mode(0o755))?;
    let p = plan(
        &f,
        dir.path(),
        vec![write("run.sh", Replace, Some("new\n"))],
    )?;
    f.library.apply(&p)?;
    let mode = fs::metadata(dir.path().join("run.sh"))?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
    Ok(())
}

#[test]
fn applying_again_to_a_registered_project_updates_its_row() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let first = plan(&f, dir.path(), vec![write("a.txt", New, Some("1\n"))])?;
    let id = f.library.apply(&first)?.project_id;
    let second = plan(&f, dir.path(), vec![write("a.txt", Replace, Some("2\n"))])?;
    assert_eq!(f.library.apply(&second)?.project_id, id);
    assert_eq!(f.library.projects()?.len(), 1);
    assert_eq!(f.library.installed_files(id)?.len(), 1);
    assert_eq!(f.library.backups(id)?.len(), 1);
    Ok(())
}
