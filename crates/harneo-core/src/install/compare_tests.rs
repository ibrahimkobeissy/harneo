use std::collections::BTreeMap;
use std::fs;

use super::compare::MAX_LOSS_LINES;
use super::fixture::{fx, put, Fx, TestResult};
use super::{
    Comparison, InProfile, InProject, LossUnit, RefitAction as A, Row, RowKind, WriteAction,
};
use crate::artifact::ArtifactKind;
use crate::library::{Source, TagChoices};

fn row<'a>(c: &'a Comparison, item: &str) -> Result<&'a Row, String> {
    c.rows.iter().find(|r| r.item == item).ok_or_else(|| {
        format!(
            "no row {item}: {:?}",
            c.rows.iter().map(|r| &r.item).collect::<Vec<_>>()
        )
    })
}

/// The fixture profile plus a settings template with one allow rule and one hook.
fn with_settings(f: &mut Fx) -> TestResult {
    let src = tempfile::tempdir()?;
    put(
        src.path(),
        ".claude/settings.json",
        r#"{"permissions":{"allow":["Bash(ls:*)"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo profile"}]}]}}"#,
    )?;
    let preview = f.library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    let id = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Settings)
        .map(|c| c.id.clone())
        .ok_or("no settings")?;
    let mut spec = f.library.profile(f.profile, None)?.spec;
    spec.settings = Some(id);
    f.library.save_profile(f.profile, &spec)?;
    Ok(())
}

/// A project with its own CLAUDE.md, a differing agent, an identical skill file set, an extra
/// command and a local settings file.
fn project(f: &Fx) -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    put(
        r,
        "CLAUDE.md",
        "# Mine\n## Deploy\nmake deploy\nNever on Fridays.\n",
    )?;
    put(
        r,
        ".claude/agents/rev.md",
        "---\nname: rev\ndescription: Reviews code\n---\nOld body\n",
    )?;
    put(
        r,
        ".claude/commands/deploy.md",
        "---\ndescription: Deploy it\n---\nRun make deploy\nthen ping\n",
    )?;
    put(r, ".claude/settings.local.json", "{}")?;
    let skill = f.library.render(
        &f.library.profile(f.profile, None)?.spec,
        "web",
        1,
        "2026-01-01",
    )?;
    for file in skill
        .files
        .iter()
        .filter(|x| x.path.starts_with(".claude/skills/"))
    {
        fs::create_dir_all(r.join(&file.path).parent().ok_or("no parent")?)?;
        fs::write(r.join(&file.path), &file.bytes)?;
    }
    Ok(dir)
}

#[test]
fn rows_get_their_kind_default_and_options() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    assert!(!c.nothing_to_change);

    let both = row(&c, "CLAUDE.md")?;
    assert_eq!(both.kind, RowKind::Both);
    assert_eq!(
        (both.in_project, both.in_profile),
        (InProject::Exists, InProfile::Differs)
    );
    assert_eq!(both.default_action, A::Replace);
    assert_eq!(both.options, vec![A::Keep, A::Replace]);

    let same = row(&c, ".claude/skills/s")?;
    assert_eq!(same.kind, RowKind::Same);
    assert_eq!(
        (same.in_project, same.in_profile),
        (InProject::Same, InProfile::Same)
    );
    assert_eq!(same.default_action, A::Keep);
    assert_eq!(same.paths.len(), 2, "a skill folder is one row");
    assert!(same.loss.is_none());

    let only = row(&c, ".claude/commands/deploy.md")?;
    assert_eq!(only.kind, RowKind::ProjectOnly);
    assert_eq!(
        (only.in_project, only.in_profile),
        (InProject::Exists, InProfile::Absent)
    );
    assert_eq!(only.default_action, A::Keep);
    assert_eq!(only.options, vec![A::Keep, A::Delete]);
    assert_eq!(only.loss.as_ref().map(|l| l.count), Some(5));

    let new = row(&c, "HARNESS.md")?;
    assert_eq!(new.kind, RowKind::ProfileOnly);
    assert_eq!(
        (new.in_project, new.in_profile),
        (InProject::Absent, InProfile::New)
    );
    assert_eq!(new.default_action, A::Add);
    assert_eq!(new.options, vec![A::Add, A::Skip]);
    assert!(new.loss.is_none());

    assert_eq!(row(&c, ".claude/agents/rev.md")?.kind, RowKind::Both);
    assert!(c
        .rows
        .iter()
        .all(|r| !r.paths.iter().any(|p| p.contains("local"))));
    Ok(())
}

#[test]
fn claude_md_loss_lists_the_lines_the_profile_lacks() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let loss = row(&c, "CLAUDE.md")?.loss.clone().ok_or("no loss")?;
    assert_eq!(loss.unit, LossUnit::Lines);
    assert_eq!(
        loss.lines,
        ["# Mine", "## Deploy", "make deploy", "Never on Fridays."]
    );
    assert_eq!(loss.count, 4);
    Ok(())
}

#[test]
fn loss_lines_are_capped_but_counted() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let many: String = (0..300).map(|i| format!("line {i}\n")).collect();
    put(dir.path(), "CLAUDE.md", &many)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let loss = row(&c, "CLAUDE.md")?.loss.clone().ok_or("no loss")?;
    assert_eq!((loss.lines.len(), loss.count), (MAX_LOSS_LINES, 300));
    Ok(())
}

#[test]
fn settings_loss_counts_permission_rules_and_hooks() -> TestResult {
    let mut f = fx()?;
    with_settings(&mut f)?;
    let dir = project(&f)?;
    put(
        dir.path(),
        ".claude/settings.json",
        r#"{"permissions":{"allow":["Bash(ls:*)","Bash(make:*)"],"deny":["Read(./x/**)"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo mine"},{"type":"command","command":"echo profile"}]}]}}"#,
    )?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let settings = row(&c, ".claude/settings.json")?;
    assert_eq!(settings.kind, RowKind::Both);
    let loss = settings.loss.clone().ok_or("no loss")?;
    assert_eq!(loss.unit, LossUnit::PermissionRules);
    assert_eq!(
        loss.lines,
        [
            "\"Bash(make:*)\" (allow)",
            "\"Read(./x/**)\" (deny)",
            "hook PreToolUse: echo mine"
        ]
    );
    assert_eq!(loss.count, 3);
    Ok(())
}

#[test]
fn unparsable_settings_fall_back_to_lines_and_a_project_only_settings_loses_every_rule(
) -> TestResult {
    let mut f = fx()?;
    let dir = project(&f)?;
    put(
        dir.path(),
        ".claude/settings.json",
        r#"{"permissions":{"allow":["A","B"]},"hooks":{"Stop":[{"hooks":[{"command":"c"}]}]}}"#,
    )?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let loss = row(&c, ".claude/settings.json")?
        .loss
        .clone()
        .ok_or("no loss")?;
    assert_eq!((loss.unit, loss.count), (LossUnit::PermissionRules, 3));

    with_settings(&mut f)?;
    put(dir.path(), ".claude/settings.json", "{ not json\nsecond\n")?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let loss = row(&c, ".claude/settings.json")?
        .loss
        .clone()
        .ok_or("no loss")?;
    assert_eq!((loss.unit, loss.count), (LossUnit::Lines, 2));
    Ok(())
}

#[test]
fn choices_change_the_plan_and_replace_carries_a_diff() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;

    let plan = f.library.plan_refit(&c, &BTreeMap::new())?;
    let act = |p: &str| plan.writes.iter().find(|w| w.path == p).map(|w| w.action);
    assert_eq!(act("CLAUDE.md"), Some(WriteAction::Replace));
    assert_eq!(act(".claude/agents/rev.md"), Some(WriteAction::Replace));
    assert_eq!(act("HARNESS.md"), Some(WriteAction::New));
    assert_eq!(act(".claude/commands/deploy.md"), Some(WriteAction::Keep));
    assert_eq!(act(".claude/skills/s/SKILL.md"), Some(WriteAction::Keep));
    assert_eq!(plan.backup_count, 2);
    assert_eq!(plan.reason, "Refit web v1");
    let replaced = plan
        .writes
        .iter()
        .find(|w| w.path == "CLAUDE.md")
        .ok_or("none")?;
    assert!(replaced.bytes.is_some());
    assert!(replaced.diff.as_ref().is_some_and(|d| !d.is_empty()));
    // Identical files are tracked through a Keep that carries their bytes; differing ones are not.
    let tracked = plan
        .writes
        .iter()
        .find(|w| w.path == ".claude/skills/s/SKILL.md")
        .ok_or("none")?;
    assert!(tracked.bytes.is_some());
    let kept = plan
        .writes
        .iter()
        .find(|w| w.path == ".claude/commands/deploy.md")
        .ok_or("none")?;
    assert!(kept.bytes.is_none());

    let choices = BTreeMap::from([
        ("CLAUDE.md".to_owned(), A::Keep),
        (".claude/commands/deploy.md".to_owned(), A::Delete),
        ("HARNESS.md".to_owned(), A::Skip),
    ]);
    let plan = f.library.plan_refit(&c, &choices)?;
    let act = |p: &str| plan.writes.iter().find(|w| w.path == p).map(|w| w.action);
    assert_eq!(act("CLAUDE.md"), Some(WriteAction::Keep));
    assert_eq!(act(".claude/commands/deploy.md"), Some(WriteAction::Delete));
    assert_eq!(act("HARNESS.md"), Some(WriteAction::Skip));
    assert_eq!(plan.backup_count, 2);
    assert!(plan.writes.iter().all(|w| !w.path.contains("local")));
    Ok(())
}

#[test]
fn bad_choices_are_refused() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let unknown = BTreeMap::from([("nope".to_owned(), A::Keep)]);
    assert!(f.library.plan_refit(&c, &unknown).is_err());
    let wrong = BTreeMap::from([("CLAUDE.md".to_owned(), A::Delete)]);
    assert!(f.library.plan_refit(&c, &wrong).is_err());
    assert!(f
        .library
        .compare(&dir.path().join("missing"), f.profile, None)
        .is_err());
    Ok(())
}

#[test]
fn applying_the_default_plan_then_comparing_again_changes_nothing() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    let plan = f.library.plan_refit(&c, &BTreeMap::new())?;
    f.library.apply(&plan)?;
    // The project's own extra command is still there, and the originals were backed up.
    assert!(dir.path().join(".claude/commands/deploy.md").exists());
    let again = f.library.compare(dir.path(), f.profile, None)?;
    assert!(again.nothing_to_change, "{:?}", again.rows);
    assert!(again
        .rows
        .iter()
        .all(|r| matches!(r.kind, RowKind::Same | RowKind::ProjectOnly)));
    // The skill files that were identical are in the manifest.
    let id = f.library.projects()?[0].id;
    let manifest = f.library.installed_files(id)?;
    assert!(manifest
        .iter()
        .any(|m| m.path == ".claude/skills/s/SKILL.md"));
    assert!(!manifest
        .iter()
        .any(|m| m.path == ".claude/commands/deploy.md"));

    // An edit after the apply is a difference again.
    put(dir.path(), "HARNESS.md", "edited\n")?;
    assert!(
        !f.library
            .compare(dir.path(), f.profile, None)?
            .nothing_to_change
    );
    Ok(())
}

#[test]
fn a_different_profile_version_is_never_nothing_to_change() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    let plan = f.library.plan_refit(
        &f.library.compare(dir.path(), f.profile, None)?,
        &BTreeMap::new(),
    )?;
    f.library.apply(&plan)?;
    let mut spec = f.library.profile(f.profile, None)?.spec;
    spec.items.truncate(2);
    f.library.save_profile(f.profile, &spec)?;
    assert!(
        !f.library
            .compare(dir.path(), f.profile, None)?
            .nothing_to_change
    );
    Ok(())
}

#[test]
fn replace_of_a_folder_adds_missing_files_and_deletes_extras() -> TestResult {
    let f = fx()?;
    let dir = project(&f)?;
    fs::remove_file(dir.path().join(".claude/skills/s/scripts/run.sh"))?;
    put(dir.path(), ".claude/skills/s/extra.md", "mine\n")?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    assert_eq!(row(&c, ".claude/skills/s")?.kind, RowKind::Both);
    let plan = f.library.plan_refit(&c, &BTreeMap::new())?;
    let act = |p: &str| plan.writes.iter().find(|w| w.path == p).map(|w| w.action);
    assert_eq!(
        act(".claude/skills/s/scripts/run.sh"),
        Some(WriteAction::New)
    );
    assert_eq!(act(".claude/skills/s/extra.md"), Some(WriteAction::Delete));
    let keep = BTreeMap::from([(".claude/skills/s".to_owned(), A::Keep)]);
    let plan = f.library.plan_refit(&c, &keep)?;
    let act = |p: &str| plan.writes.iter().find(|w| w.path == p).map(|w| w.action);
    assert_eq!(
        act(".claude/skills/s/scripts/run.sh"),
        Some(WriteAction::Skip)
    );
    assert_eq!(act(".claude/skills/s/extra.md"), Some(WriteAction::Keep));
    Ok(())
}

#[test]
fn an_empty_folder_has_only_profile_rows() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let c = f.library.compare(dir.path(), f.profile, None)?;
    assert!(c.rows.iter().all(|r| r.kind == RowKind::ProfileOnly));
    assert!(!c.nothing_to_change);
    let plan = f.library.plan_refit(&c, &BTreeMap::new())?;
    assert!(plan.writes.iter().all(|w| w.action == WriteAction::New));
    Ok(())
}
