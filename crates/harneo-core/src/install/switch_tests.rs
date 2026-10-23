use std::fs;
use std::io;

use super::apply::ApplyHooks;
use super::fixture::{fx, put, TestResult};
use super::fixture_lifecycle::{
    files_under, install_over, mini_profile, read, with_agent, without_agent,
};
use super::{PlanKind, WriteAction};
use crate::error::Error;

const AGENT: &str = ".claude/agents/rev.md";

fn actions(plan: &super::Plan) -> Vec<(&str, WriteAction)> {
    plan.writes
        .iter()
        .map(|w| (w.path.as_str(), w.action))
        .collect()
}

#[test]
fn a_switch_plans_and_applies_only_the_difference() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let skill_before = read(dir.path(), ".claude/skills/s/SKILL.md")?;
    let v2 = without_agent(&f)?;

    let plan = f.library.plan_switch(report.project_id, f.profile, None)?;
    assert_eq!(plan.kind, PlanKind::Switch);
    assert_eq!(plan.profile_version, v2);
    assert_eq!(plan.reason, format!("Upgrade web to v{v2}"));
    assert_eq!(
        actions(&plan),
        [
            (".claude/agents/rev.md", WriteAction::Delete),
            ("HARNESS.md", WriteAction::Replace)
        ]
    );
    assert!(plan.edited_in_project.is_empty() && plan.restored.is_empty());
    assert_eq!(plan.backup_count, 2);

    let applied = f.library.apply(&plan)?;
    assert_eq!(applied.project_id, report.project_id);
    assert!(!dir.path().join(AGENT).exists());
    assert!(
        !dir.path().join(".claude/agents").exists(),
        "emptied folder"
    );
    assert_eq!(read(dir.path(), ".claude/skills/s/SKILL.md")?, skill_before);
    let project = f.library.project(report.project_id)?;
    assert_eq!(project.profile_version, v2);
    let paths: Vec<String> = f
        .library
        .installed_files(report.project_id)?
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert!(!paths.contains(&AGENT.to_owned()));
    assert!(!String::from_utf8(read(dir.path(), "HARNESS.md")?)?.contains("rev"));
    // Switching again has nothing left to do but the generated file's own date.
    let again = f.library.plan_switch(report.project_id, f.profile, None)?;
    assert!(again.writes.is_empty(), "{:?}", actions(&again));
    Ok(())
}

#[test]
fn a_removed_path_that_replaced_an_original_gets_it_back() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), AGENT, "---\nname: rev\n---\nmine\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    assert_ne!(read(dir.path(), AGENT)?, b"---\nname: rev\n---\nmine\n");
    without_agent(&f)?;

    let plan = f.library.plan_switch(report.project_id, f.profile, None)?;
    let agent = plan
        .writes
        .iter()
        .find(|w| w.path == AGENT)
        .ok_or("agent")?;
    assert_eq!(agent.action, WriteAction::Replace);
    assert_eq!(plan.restored, [AGENT]);
    f.library.apply(&plan)?;
    assert_eq!(read(dir.path(), AGENT)?, b"---\nname: rev\n---\nmine\n");
    // The file is the project's again: Harneo no longer lists it.
    assert!(!f
        .library
        .installed_files(report.project_id)?
        .iter()
        .any(|m| m.path == AGENT));
    Ok(())
}

#[test]
fn an_edited_file_is_flagged_and_still_backed_up() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    without_agent(&f)?;
    fs::write(dir.path().join(AGENT), "my edit\n")?;
    fs::write(dir.path().join("HARNESS.md"), "my notes\n")?;

    let plan = f.library.plan_switch(report.project_id, f.profile, None)?;
    assert_eq!(plan.edited_in_project, [AGENT, "HARNESS.md"]);
    let applied = f.library.apply(&plan)?;
    let backups = f.library.backups(report.project_id)?;
    assert_eq!(backups.len(), 1);
    assert_eq!(Some(backups[0].id), applied.backup_id);
    let set = f.data.path().join(&backups[0].folder);
    assert_eq!(read(&set, AGENT)?, b"my edit\n");
    assert_eq!(read(&set, "HARNESS.md")?, b"my notes\n");
    Ok(())
}

#[test]
fn a_new_version_adds_files_and_flags_a_project_file_in_the_way() -> TestResult {
    let f = fx()?;
    let mini = mini_profile(&f)?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), mini)?;
    assert!(!dir.path().join(AGENT).exists());
    let v2 = with_agent(&f, mini)?;
    put(dir.path(), AGENT, "the project's own agent\n")?;

    let plan = f.library.plan_switch(report.project_id, mini, Some(v2))?;
    assert_eq!(plan.reason, format!("Upgrade mini to v{v2}"));
    let agent = plan
        .writes
        .iter()
        .find(|w| w.path == AGENT)
        .ok_or("agent")?;
    assert_eq!(agent.action, WriteAction::Replace);
    assert!(agent.artifact.is_some());
    assert_eq!(plan.edited_in_project, [AGENT]);
    f.library.apply(&plan)?;
    assert!(f
        .library
        .installed_files(report.project_id)?
        .iter()
        .any(|m| m.path == AGENT));
    // Switching to another profile names it in the reason and changes the project's profile.
    let web = f.library.plan_switch(report.project_id, f.profile, None)?;
    assert_eq!(web.reason, "Switch to web v1");
    f.library.apply(&web)?;
    assert_eq!(f.library.project(report.project_id)?.profile_id, f.profile);
    Ok(())
}

#[test]
fn files_missing_from_the_project_are_created_or_dropped_quietly() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    without_agent(&f)?;
    fs::remove_file(dir.path().join("HARNESS.md"))?;
    fs::remove_file(dir.path().join(AGENT))?;
    let plan = f.library.plan_switch(report.project_id, f.profile, None)?;
    assert_eq!(
        actions(&plan),
        [
            (AGENT, WriteAction::Delete),
            ("HARNESS.md", WriteAction::New)
        ]
    );
    assert!(plan.edited_in_project.is_empty());
    f.library.apply(&plan)?;
    assert!(dir.path().join("HARNESS.md").exists());
    Ok(())
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

#[test]
fn a_failed_switch_changes_nothing() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let before = files_under(dir.path());
    let harness = read(dir.path(), "HARNESS.md")?;
    without_agent(&f)?;
    let plan = f.library.plan_switch(report.project_id, f.profile, None)?;
    let error = f
        .library
        .apply_with(&plan, &FailAt("HARNESS.md"))
        .err()
        .ok_or("must fail")?;
    assert!(error.to_string().contains("Rolled back"), "{error}");
    assert_eq!(files_under(dir.path()), before);
    assert_eq!(read(dir.path(), "HARNESS.md")?, harness);
    assert_eq!(f.library.project(report.project_id)?.profile_version, 1);
    assert!(f.library.backups(report.project_id)?.is_empty());
    Ok(())
}

#[test]
fn unknown_projects_and_vanished_folders_are_refused() -> TestResult {
    let f = fx()?;
    assert!(matches!(
        f.library.plan_switch(9, f.profile, None),
        Err(Error::ProjectNotFound { id: 9 })
    ));
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    drop(dir);
    assert!(matches!(
        f.library.plan_switch(report.project_id, f.profile, None),
        Err(Error::Install { .. })
    ));
    Ok(())
}

#[test]
fn a_managed_path_that_is_a_folder_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let report = install_over(&f, dir.path(), f.profile)?;
    without_agent(&f)?;
    fs::remove_file(dir.path().join(AGENT))?;
    fs::create_dir(dir.path().join(AGENT))?;
    assert!(matches!(
        f.library.plan_switch(report.project_id, f.profile, None),
        Err(Error::Install { .. })
    ));
    Ok(())
}
