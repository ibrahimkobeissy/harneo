//! Regression tests for the install review (#143): file modes, Windows-hostile path parts,
//! check-to-write races and the cap on whole-file reads.

use std::fs;
use std::io;

use super::apply::ApplyHooks;
use super::fixture::{fx, plan, put, write, TestResult};
use super::{safe_parts, WriteAction};

/// Runs `act` when the apply is about to write `path`: a change made behind Harneo's back.
struct Meddle<F: Fn() -> io::Result<()>> {
    path: &'static str,
    act: F,
}

impl<F: Fn() -> io::Result<()>> ApplyHooks for Meddle<F> {
    fn before_write(&self, path: &str) -> io::Result<()> {
        if path == self.path {
            (self.act)()?;
        }
        Ok(())
    }
}

struct FailCommit;
impl ApplyHooks for FailCommit {
    fn before_commit(&self) -> io::Result<()> {
        Err(io::Error::other("boom"))
    }
}

#[test]
fn windows_hostile_path_parts_are_refused() {
    for bad in [
        "C:x",
        "a/C:x",
        ".claude/agents/x.md:stream",
        "CLAUDE.md.",
        "dir./x.md",
        "x.md ",
        "CON",
        "con.md",
        ".claude/NUL.txt",
        "aux",
        "PRN.x",
        "COM1",
        "com9.md",
        "LPT1",
        "lpt3.txt",
        "CLAUDE~1.MD",
        ".claude/agents/REVIEW~2.MD",
    ] {
        assert!(safe_parts(bad).is_err(), "{bad} must be refused");
    }
    for good in [
        "CLAUDE.md",
        ".claude/agents/rev.md",
        "a~b.md",
        "console.md",
        "COM10",
        "com0.md",
        ".mcp.json",
        "x.y.z",
    ] {
        assert!(safe_parts(good).is_ok(), "{good} must be allowed");
    }
}

#[test]
fn a_plan_with_a_hostile_path_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let p = plan(
        &f,
        dir.path(),
        vec![write("CLAUDE~1.MD", WriteAction::New, Some("x"))],
    )?;
    assert!(f.library.apply(&p).is_err());
    Ok(())
}

#[test]
fn a_replace_target_changed_after_the_backup_is_never_overwritten() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "a.md", "mine\n")?;
    let file = dir.path().join("a.md");
    let hooks = Meddle {
        path: "a.md",
        act: || fs::write(&file, "edited meanwhile\n"),
    };
    let p = plan(
        &f,
        dir.path(),
        vec![write("a.md", WriteAction::Replace, Some("new\n"))],
    )?;
    assert!(f.library.apply_with(&p, &hooks).is_err());
    assert_eq!(fs::read_to_string(&file)?, "edited meanwhile\n");
    Ok(())
}

#[test]
fn a_delete_target_changed_after_the_backup_is_kept() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "a.md", "mine\n")?;
    let file = dir.path().join("a.md");
    let hooks = Meddle {
        path: "a.md",
        act: || fs::write(&file, "edited meanwhile\n"),
    };
    let p = plan(
        &f,
        dir.path(),
        vec![write("a.md", WriteAction::Delete, None)],
    )?;
    assert!(f.library.apply_with(&p, &hooks).is_err());
    assert_eq!(fs::read_to_string(&file)?, "edited meanwhile\n");
    Ok(())
}

#[test]
fn a_new_file_that_appears_before_the_write_is_never_overwritten_or_removed() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("a.md");
    let hooks = Meddle {
        path: "a.md",
        act: || fs::write(&file, "theirs\n"),
    };
    let p = plan(
        &f,
        dir.path(),
        vec![write("a.md", WriteAction::New, Some("new\n"))],
    )?;
    assert!(f.library.apply_with(&p, &hooks).is_err());
    assert_eq!(fs::read_to_string(&file)?, "theirs\n");
    Ok(())
}

#[test]
fn a_file_over_the_read_cap_is_refused_with_a_clear_error() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("big.bin");
    fs::File::create(&file)?.set_len(64 * 1024 * 1024 + 1)?;
    let p = plan(
        &f,
        dir.path(),
        vec![write("big.bin", WriteAction::Replace, Some("x"))],
    )?;
    let err = f
        .library
        .apply(&p)
        .err()
        .ok_or("expected an error")?
        .to_string();
    assert!(err.contains("too large"), "{err}");
    assert_eq!(fs::metadata(&file)?.len(), 64 * 1024 * 1024 + 1);
    Ok(())
}

#[cfg(unix)]
mod modes {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::install::PlanKind;

    fn mode(path: &std::path::Path) -> io::Result<u32> {
        Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
    }

    fn chmod(path: &std::path::Path, m: u32) -> io::Result<()> {
        fs::set_permissions(path, fs::Permissions::from_mode(m))
    }

    /// What `File::create` gives here: 0666 minus the umask.
    fn default_mode() -> io::Result<u32> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("probe");
        fs::File::create(&file)?;
        mode(&file)
    }

    #[test]
    fn new_files_get_the_default_mode_not_0600() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        let p = f.library.plan_scaffold(dir.path(), f.profile, None)?;
        f.library.apply(&p)?;
        assert_eq!(mode(&dir.path().join("CLAUDE.md"))?, default_mode()?);
        Ok(())
    }

    #[test]
    fn a_rolled_back_delete_brings_the_mode_back() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "run.sh", "x\n")?;
        chmod(&dir.path().join("run.sh"), 0o755)?;
        let p = plan(
            &f,
            dir.path(),
            vec![write("run.sh", WriteAction::Delete, None)],
        )?;
        assert!(f.library.apply_with(&p, &FailCommit).is_err());
        assert_eq!(mode(&dir.path().join("run.sh"))?, 0o755);
        Ok(())
    }

    #[test]
    fn a_rolled_back_replace_brings_the_mode_back() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "a.md", "x\n")?;
        chmod(&dir.path().join("a.md"), 0o640)?;
        let p = plan(
            &f,
            dir.path(),
            vec![write("a.md", WriteAction::Replace, Some("y\n"))],
        )?;
        assert!(f.library.apply_with(&p, &FailCommit).is_err());
        assert_eq!(mode(&dir.path().join("a.md"))?, 0o640);
        Ok(())
    }

    #[test]
    fn restore_backup_puts_the_recorded_mode_back() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "run.sh", "x\n")?;
        chmod(&dir.path().join("run.sh"), 0o750)?;
        let p = plan(
            &f,
            dir.path(),
            vec![write("run.sh", WriteAction::Replace, Some("y\n"))],
        )?;
        let r = f.library.apply(&p)?;
        chmod(&dir.path().join("run.sh"), 0o600)?;
        f.library.restore_backup(r.backup_id.ok_or("no backup")?)?;
        assert_eq!(fs::read_to_string(dir.path().join("run.sh"))?, "x\n");
        assert_eq!(mode(&dir.path().join("run.sh"))?, 0o750);
        Ok(())
    }

    #[test]
    fn restore_backup_of_a_deleted_executable_keeps_it_executable() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "run.sh", "x\n")?;
        chmod(&dir.path().join("run.sh"), 0o755)?;
        let p = plan(
            &f,
            dir.path(),
            vec![write("run.sh", WriteAction::Delete, None)],
        )?;
        let r = f.library.apply(&p)?;
        assert!(!dir.path().join("run.sh").exists());
        f.library.restore_backup(r.backup_id.ok_or("no backup")?)?;
        assert_eq!(mode(&dir.path().join("run.sh"))?, 0o755);
        Ok(())
    }

    #[test]
    fn removal_restores_originals_with_their_mode() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "run.sh", "x\n")?;
        put(dir.path(), "gone.sh", "g\n")?;
        chmod(&dir.path().join("run.sh"), 0o750)?;
        chmod(&dir.path().join("gone.sh"), 0o755)?;
        let mut p = plan(
            &f,
            dir.path(),
            vec![
                write("run.sh", WriteAction::Replace, Some("y\n")),
                write("gone.sh", WriteAction::Delete, None),
                write("new.md", WriteAction::New, Some("n\n")),
            ],
        )?;
        p.kind = PlanKind::Refit;
        let r = f.library.apply(&p)?;
        chmod(&dir.path().join("run.sh"), 0o600)?;
        let removal = f.library.plan_removal(r.project_id)?;
        f.library.apply(&removal)?;
        assert_eq!(fs::read_to_string(dir.path().join("run.sh"))?, "x\n");
        assert_eq!(mode(&dir.path().join("run.sh"))?, 0o750);
        assert_eq!(mode(&dir.path().join("gone.sh"))?, 0o755);
        Ok(())
    }

    #[test]
    fn old_backup_sets_without_a_mode_still_restore() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        put(dir.path(), "a.md", "x\n")?;
        let p = plan(
            &f,
            dir.path(),
            vec![write("a.md", WriteAction::Replace, Some("y\n"))],
        )?;
        let r = f.library.apply(&p)?;
        let folder = f.data.path().join(r.backup_folder.ok_or("no folder")?);
        let manifest = folder.join("backup.json");
        let text = fs::read_to_string(&manifest)?;
        let mut json: serde_json::Value = serde_json::from_str(&text)?;
        for e in json["entries"].as_array_mut().ok_or("entries")? {
            e.as_object_mut().ok_or("entry")?.remove("mode");
        }
        fs::write(&manifest, serde_json::to_vec(&json)?)?;
        f.library.restore_backup(r.backup_id.ok_or("no backup")?)?;
        assert_eq!(fs::read_to_string(dir.path().join("a.md"))?, "x\n");
        Ok(())
    }
}
