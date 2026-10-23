use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::fixture::{fx, put, TestResult};
use super::plan::{safe_parts, utc_date, utc_stamp};
use super::WriteAction;
use crate::error::Error;

#[test]
fn utc_formats_known_instants() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_stamp(1_700_000_000), "2023-11-14T22-13-20Z");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
}

#[test]
fn scaffold_plans_every_rendered_file_as_new_and_writes_nothing() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "src/main.rs", "fn main() {}\n")?;
    let plan = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    let paths: Vec<&str> = plan.writes.iter().map(|w| w.path.as_str()).collect();
    assert!(paths.contains(&"CLAUDE.md") && paths.contains(&"HARNESS.md"));
    assert!(paths.contains(&".claude/skills/s/SKILL.md"));
    assert!(plan
        .writes
        .iter()
        .all(|w| w.action == WriteAction::New && w.bytes.is_some() && w.diff.is_none()));
    assert_eq!(plan.backup_count, 0);
    assert_eq!(plan.reason, "Scaffold web v1");
    assert_eq!(
        plan.target,
        dir.path().canonicalize()?.display().to_string()
    );
    let left: Vec<_> = fs::read_dir(dir.path())?.collect();
    assert_eq!(left.len(), 1, "only the project's own src folder");
    Ok(())
}

#[test]
fn a_folder_with_a_harness_is_refused() -> TestResult {
    let f = fx()?;
    for marker in [".claude/x", "CLAUDE.md", ".mcp.json"] {
        let dir = tempfile::tempdir()?;
        put(dir.path(), marker, "x")?;
        let error = f.library.plan_scaffold(dir.path(), f.profile, None);
        assert!(
            matches!(&error, Err(Error::HasHarness { .. })),
            "{marker}: {error:?}"
        );
        assert!(error.is_err_and(|e| e.to_string().contains("refit it instead")));
    }
    Ok(())
}

#[test]
fn a_file_or_missing_folder_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "file.txt", "x")?;
    for target in [dir.path().join("file.txt"), dir.path().join("nope")] {
        assert!(matches!(
            f.library.plan_scaffold(&target, f.profile, None),
            Err(Error::Install { .. })
        ));
    }
    Ok(())
}

#[test]
fn an_unwritable_folder_is_refused_and_nothing_is_written() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555))?;
    let result = f.library.plan_scaffold(dir.path(), f.profile, None);
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755))?;
    assert!(matches!(result, Err(Error::Install { .. })), "{result:?}");
    assert_eq!(fs::read_dir(dir.path())?.count(), 0);
    Ok(())
}

#[test]
fn an_unknown_profile_or_version_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    assert!(f.library.plan_scaffold(dir.path(), 999, None).is_err());
    assert!(f
        .library
        .plan_scaffold(dir.path(), f.profile, Some(9))
        .is_err());
    Ok(())
}

#[test]
fn unsafe_paths_are_refused() {
    for bad in [
        "",
        "/etc/passwd",
        "../x",
        "a/../b",
        "a//b",
        "./a",
        "a\\b",
        ".git/config",
        "sub/.GIT/x",
        "CLAUDE.local.md",
        "a/claude.local.md",
        ".claude/settings.local.json",
    ] {
        assert!(safe_parts(bad).is_err(), "{bad:?} should be refused");
    }
    assert!(safe_parts(".claude/skills/s/SKILL.md").is_ok());
    assert!(safe_parts("CLAUDE.md").is_ok());
    assert!(safe_parts(".gitignore").is_ok());
}
