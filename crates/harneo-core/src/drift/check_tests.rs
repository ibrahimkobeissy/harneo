use std::fs;

use super::fixture::{fx, project, put, read, TestResult, AGENT, SKILL};
use super::{DriftChange, DriftFile, DriftStatus};
use crate::install::WriteAction;

fn change<'a>(files: &'a [DriftFile], path: &str) -> Option<&'a DriftChange> {
    files.iter().find(|f| f.path == path).map(|f| &f.change)
}

#[test]
fn a_fresh_project_is_in_sync() -> TestResult {
    let f = fx()?;
    let (_dir, id) = project(&f)?;
    let report = f.library.check_project(id)?;
    assert_eq!(report.status, DriftStatus::InSync, "{:?}", report.files);
    assert!(report.files.is_empty() && report.library_newer.is_empty());
    assert_eq!(report.profile_newer, None);
    Ok(())
}

#[test]
fn edited_deleted_and_added_files_are_found_with_line_counts() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    // Three lines become two plus a new one: +2 -2 against the profile's render.
    put(
        dir.path(),
        AGENT,
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nChanged\nNew\n",
    )?;
    fs::remove_file(dir.path().join(SKILL))?;
    put(
        dir.path(),
        ".claude/commands/extra.md",
        "---\ndescription: Extra\n---\nGo\n",
    )?;
    put(dir.path(), "CLAUDE.local.md", "mine\n")?;
    put(dir.path(), ".claude/settings.local.json", "{}\n")?;

    let report = f.library.check_project(id)?;
    assert_eq!(report.status, DriftStatus::Drift);
    assert_eq!(
        change(&report.files, AGENT),
        Some(&DriftChange::Edited { plus: 2, minus: 2 })
    );
    assert_eq!(change(&report.files, SKILL), Some(&DriftChange::Deleted));
    assert_eq!(
        change(&report.files, ".claude/commands/extra.md"),
        Some(&DriftChange::Added)
    );
    assert_eq!(report.files.len(), 3, "{:?}", report.files);
    let added = report.files.last().ok_or("no files")?;
    assert_eq!(added.change, DriftChange::Added, "added files come last");
    assert!(added.artifact.is_none());
    let edited = report
        .files
        .iter()
        .find(|x| x.path == AGENT)
        .ok_or("agent")?;
    assert!(edited.artifact.is_some());
    Ok(())
}

#[test]
fn a_missing_project_folder_is_reported_as_missing() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    let path = dir.path().to_path_buf();
    fs::remove_dir_all(&path)?;
    let report = f.library.check_project(id)?;
    assert_eq!(report.status, DriftStatus::Missing);
    assert!(report.files.is_empty());
    assert!(f.library.plan_reapply(id, None).is_err());
    fs::create_dir(&path)?;
    Ok(())
}

#[test]
fn an_unknown_project_is_an_error() -> TestResult {
    let f = fx()?;
    assert!(f.library.check_project(99).is_err());
    assert!(f.library.drift_diff(99, AGENT).is_err());
    Ok(())
}

#[test]
fn a_newer_profile_version_is_reported_without_drift() -> TestResult {
    let f = fx()?;
    let (_dir, id) = project(&f)?;
    let view = f.library.profile(f.profile, None)?;
    let mut spec = view.spec;
    spec.items.truncate(2);
    let v2 = f.library.save_profile(f.profile, &spec)?;
    let report = f.library.check_project(id)?;
    assert_eq!(report.profile_newer, Some(v2));
    assert_eq!(report.status, DriftStatus::InSync);
    Ok(())
}

#[test]
fn check_all_summarises_every_project() -> TestResult {
    let f = fx()?;
    let (a, a_id) = project(&f)?;
    let (_b, b_id) = project(&f)?;
    put(a.path(), AGENT, "changed\n")?;
    let all = f.library.check_all()?;
    assert_eq!(all.len(), 2);
    let of = |id| all.iter().find(|p| p.project_id == id).map(|p| &p.summary);
    assert_eq!(
        of(a_id).map(|s| (s.status, s.drifted)),
        Some((DriftStatus::Drift, 1))
    );
    assert_eq!(
        of(b_id).map(|s| (s.status, s.drifted)),
        Some((DriftStatus::InSync, 0))
    );
    Ok(())
}

#[test]
fn the_diff_runs_from_the_profile_version_to_the_project_file() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    assert!(f.library.drift_diff(id, AGENT)?.is_empty());
    put(
        dir.path(),
        AGENT,
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nChanged\nThree\n",
    )?;
    let hunks = f.library.drift_diff(id, AGENT)?;
    let text: Vec<String> = hunks
        .iter()
        .flat_map(|h| h.lines.iter().map(|l| format!("{:?}:{}", l.kind, l.text)))
        .collect();
    assert!(text.contains(&"Removed:Two".to_owned()), "{text:?}");
    assert!(text.contains(&"Added:Changed".to_owned()), "{text:?}");
    // A deleted file shows as all lines removed.
    fs::remove_file(dir.path().join(SKILL))?;
    let gone = f.library.drift_diff(id, SKILL)?;
    assert!(gone
        .iter()
        .flat_map(|h| &h.lines)
        .all(|l| l.kind == crate::updates::LineKind::Removed));
    Ok(())
}

#[test]
fn reapply_restores_sync_and_backs_up_the_project_version() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    let original = read(dir.path(), AGENT)?;
    put(dir.path(), AGENT, "mine\n")?;
    fs::remove_file(dir.path().join(SKILL))?;
    put(
        dir.path(),
        ".claude/commands/extra.md",
        "---\ndescription: E\n---\n",
    )?;

    let plan = f.library.plan_reapply(id, None)?;
    let actions: Vec<_> = plan
        .writes
        .iter()
        .map(|w| (w.path.as_str(), w.action))
        .collect();
    assert_eq!(
        actions,
        [(AGENT, WriteAction::Replace), (SKILL, WriteAction::New)]
    );
    assert_eq!(plan.edited_in_project, [AGENT]);
    assert_eq!(plan.backup_count, 1);
    assert!(plan.writes[0].diff.as_ref().is_some_and(|d| !d.is_empty()));

    let applied = f.library.apply(&plan)?;
    assert_eq!(applied.backed_up, 1);
    let backup = f.data.path().join(applied.backup_folder.ok_or("backup")?);
    assert!(
        backup.join(AGENT).is_file(),
        "the edited file is in the backup"
    );
    assert_eq!(fs::read_to_string(backup.join(AGENT))?, "mine\n");
    assert_eq!(read(dir.path(), AGENT)?, original);
    let report = f.library.check_project(id)?;
    // Only the added artifact is left; the project's own files are never touched.
    assert_eq!(report.files.len(), 1, "{:?}", report.files);
    assert_eq!(report.files[0].change, DriftChange::Added);
    assert!(read(dir.path(), ".claude/commands/extra.md").is_ok());
    Ok(())
}

#[test]
fn reapply_can_take_a_subset_and_refuses_anything_else() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    put(dir.path(), AGENT, "mine\n")?;
    put(dir.path(), SKILL, "mine too\n")?;
    let plan = f.library.plan_reapply(id, Some(vec![AGENT.to_owned()]))?;
    let paths: Vec<&str> = plan.writes.iter().map(|w| w.path.as_str()).collect();
    assert!(paths.contains(&AGENT) && !paths.contains(&SKILL));
    f.library.apply(&plan)?;
    assert_eq!(read(dir.path(), SKILL)?, "mine too\n");
    let err = f
        .library
        .plan_reapply(id, Some(vec!["README.md".to_owned()]));
    assert!(err.is_err());
    // Nothing to do after the first one: the skill is still drifted, the agent is not.
    assert!(f
        .library
        .plan_reapply(id, Some(vec![AGENT.to_owned()]))
        .is_err());
    Ok(())
}
