use std::fs;

use super::diff::*;
use super::tests::{agent, write, TestResult};

#[test]
fn lists_added_removed_and_changed_files_with_hunks() -> TestResult {
    let (old, new) = (tempfile::tempdir()?, tempfile::tempdir()?);
    let body = |changed: &str| {
        (1..=10)
            .map(|n| {
                if n == 5 {
                    changed.to_owned()
                } else {
                    format!("line {n}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    write(old.path(), "SKILL.md", &body("old five"))?;
    write(new.path(), "SKILL.md", &body("new five"))?;
    write(old.path(), "gone.md", "bye\n")?;
    write(new.path(), "sub/added.md", "hi\nthere\n")?;
    write(old.path(), "same.md", "same\n")?;
    write(new.path(), "same.md", "same\n")?;
    let diffs = diff_dirs(old.path(), new.path())?;
    let paths: Vec<(&str, DiffStatus)> =
        diffs.iter().map(|d| (d.path.as_str(), d.status)).collect();
    assert_eq!(
        paths,
        [
            ("SKILL.md", DiffStatus::Changed),
            ("gone.md", DiffStatus::Removed),
            ("sub/added.md", DiffStatus::Added)
        ]
    );
    let hunk = &diffs[0].hunks[0];
    assert_eq!(
        (hunk.old_start, hunk.new_start),
        (2, 2),
        "three lines of context"
    );
    let changed: Vec<(LineKind, &str)> = hunk
        .lines
        .iter()
        .filter(|l| l.kind != LineKind::Context)
        .map(|l| (l.kind, l.text.as_str()))
        .collect();
    assert_eq!(
        changed,
        [
            (LineKind::Removed, "old five"),
            (LineKind::Added, "new five")
        ]
    );
    assert_eq!(diffs[2].hunks[0].lines.len(), 2);
    assert!(diffs[2].hunks[0]
        .lines
        .iter()
        .all(|l| l.kind == LineKind::Added));
    Ok(())
}

#[test]
fn a_binary_file_changes_without_hunks() -> TestResult {
    let (old, new) = (tempfile::tempdir()?, tempfile::tempdir()?);
    fs::write(old.path().join("x.bin"), [0u8, 159, 146, 150])?;
    fs::write(new.path().join("x.bin"), [1u8, 159, 146, 150])?;
    let diffs = diff_dirs(old.path(), new.path())?;
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].hunks.is_empty());
    Ok(())
}

#[test]
fn the_update_job_diffs_the_staged_copy_against_the_library() -> TestResult {
    let mut f = super::tests::fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    write(f.repo.path(), "agents/b.md", &agent("b", "two"))?;
    super::tests::commit(f.repo.path(), "b two")?;
    f.library.check_updates(&super::CheckScope::All)?;
    let prepared = f.library.prepare_update(&b)?.run()?;
    let diffs = prepared.diff()?;
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].status, DiffStatus::Changed);
    assert!(diffs[0].hunks[0]
        .lines
        .iter()
        .any(|l| l.kind == LineKind::Added && l.text == "two"));
    Ok(())
}
