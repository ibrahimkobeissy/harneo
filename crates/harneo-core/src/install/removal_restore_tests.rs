//! Regression tests for the clean-removal review (#139): every original comes back on removal,
//! and a removed project keeps its backups.

use std::collections::BTreeMap;
use std::fs;

use super::fixture::{fx, plan, put, write, TestResult};
use super::fixture_lifecycle::{files_under, read};
use super::{PlanKind, RefitAction, WriteAction};
use crate::error::Error;

const AGENT: &str = ".claude/agents/rev.md";
const AGENT_TEXT: &str = "---\nname: rev\ndescription: Reviews code\n---\nB\n";

/// A project file identical to the profile's is kept by a refit; removal must not delete it.
#[test]
fn a_kept_file_identical_to_the_profiles_survives_removal() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), AGENT, AGENT_TEXT)?;
    put(dir.path(), "CLAUDE.md", "# mine\n")?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let r = f.library.apply(&p)?;
    let removal = f.library.plan_removal(r.project_id)?;
    f.library.apply(&removal)?;
    assert_eq!(read(dir.path(), AGENT)?, AGENT_TEXT.as_bytes());
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# mine\n");
    Ok(())
}

/// The same file stays when a later switch drops it from the profile.
#[test]
fn a_kept_identical_file_survives_a_switch_that_drops_it() -> TestResult {
    use super::fixture_lifecycle::without_agent;
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), AGENT, AGENT_TEXT)?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let r = f.library.apply(&p)?;
    let version = without_agent(&f)?;
    let switch = f
        .library
        .plan_switch(r.project_id, f.profile, Some(version))?;
    f.library.apply(&switch)?;
    assert_eq!(read(dir.path(), AGENT)?, AGENT_TEXT.as_bytes());
    Ok(())
}

/// A project-only file that the refit deleted comes back on removal.
#[test]
fn a_file_a_refit_deleted_is_restored_by_removal() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let mine = ".claude/agents/mine.md";
    put(
        dir.path(),
        mine,
        "---\nname: mine\ndescription: x\n---\nmine\n",
    )?;
    let mut choices = BTreeMap::new();
    choices.insert(mine.to_owned(), RefitAction::Delete);
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &choices)?;
    let r = f.library.apply(&p)?;
    assert!(!dir.path().join(mine).exists());
    let removal = f.library.plan_removal(r.project_id)?;
    assert!(removal.restored.iter().any(|p| p == mine));
    f.library.apply(&removal)?;
    assert_eq!(
        read(dir.path(), mine)?,
        b"---\nname: mine\ndescription: x\n---\nmine\n"
    );
    Ok(())
}

/// A deleted file whose path is taken again (by the user) is left alone.
#[test]
fn a_refit_deleted_file_is_not_restored_over_a_newer_file() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let mine = ".claude/agents/mine.md";
    put(dir.path(), mine, "old\n")?;
    let mut choices = BTreeMap::new();
    choices.insert(mine.to_owned(), RefitAction::Delete);
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &choices)?;
    let r = f.library.apply(&p)?;
    put(dir.path(), mine, "newer\n")?;
    let removal = f.library.plan_removal(r.project_id)?;
    assert!(removal.writes.iter().all(|w| w.path != mine));
    f.library.apply(&removal)?;
    assert_eq!(read(dir.path(), mine)?, b"newer\n");
    Ok(())
}

/// Restore, edit, refit again, remove: the newest original comes back, not the stale one.
#[test]
fn removal_brings_back_the_newest_original() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# v1 mine\n")?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let r = f.library.apply(&p)?;
    let first = f.library.backups(r.project_id)?[0].id;
    f.library.restore_backup(first)?;
    fs::write(dir.path().join("CLAUDE.md"), "# v2 mine, edited\n")?;
    let p2 = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    f.library.apply(&p2)?;
    let removal = f.library.plan_removal(r.project_id)?;
    f.library.apply(&removal)?;
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# v2 mine, edited\n");
    Ok(())
}

/// A removed project leaves the live list but keeps its backups, which can be restored.
#[test]
fn a_removed_project_keeps_its_backups_and_can_restore_them() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# mine\n")?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let r = f.library.apply(&p)?;
    // The user edits Harneo's file; the removal backs that edit up.
    fs::write(dir.path().join(AGENT), "my edit of the agent\n")?;
    let removal = f.library.plan_removal(r.project_id)?;
    let done = f.library.apply(&removal)?;
    assert!(done.backup_id.is_some());
    assert!(f.library.projects()?.is_empty());
    let removed = f.library.removed_projects()?;
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].profile_name, "web");
    assert!(removed[0].removed_at.is_some());
    assert_eq!(removed[0].file_count, 0);
    let backups = f.library.backups(r.project_id)?;
    assert_eq!(backups.len(), 2, "the refit's set and the removal's own");
    assert!(matches!(
        f.library.plan_removal(r.project_id),
        Err(Error::Install { .. })
    ));
    // Restoring the removal's set writes the edited file back, backing the current one up.
    let own = backups
        .iter()
        .find(|b| b.reason.starts_with("Remove"))
        .ok_or("removal set")?;
    f.library.restore_backup(own.id)?;
    assert_eq!(read(dir.path(), AGENT)?, b"my edit of the agent\n");
    assert!(
        f.library.projects()?.is_empty(),
        "restoring does not revive it"
    );
    Ok(())
}

/// Installing again at a removed project's path is a new project with its own history.
#[test]
fn installing_again_after_removal_starts_a_new_project() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# mine\n")?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let first = f.library.apply(&p)?;
    f.library
        .apply(&f.library.plan_removal(first.project_id)?)?;
    fs::write(dir.path().join("CLAUDE.md"), "# newer mine\n")?;
    let p = f
        .library
        .refit_plan(dir.path(), f.profile, None, &BTreeMap::new())?;
    let second = f.library.apply(&p)?;
    assert_ne!(first.project_id, second.project_id);
    assert_eq!(f.library.projects()?.len(), 1);
    assert_eq!(f.library.removed_projects()?.len(), 1);
    f.library
        .apply(&f.library.plan_removal(second.project_id)?)?;
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# newer mine\n");
    Ok(())
}

/// A removed project does not block deleting its profile.
#[test]
fn a_removed_project_does_not_block_deleting_its_profile() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    let p = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    let r = f.library.apply(&p)?;
    f.library.apply(&f.library.plan_removal(r.project_id)?)?;
    f.library.delete_profile(f.profile)?;
    let removed = f.library.removed_projects()?;
    assert_eq!(removed[0].profile_name, "web");
    assert_eq!(removed[0].profile_id, 0);
    Ok(())
}

/// A Replace target deleted between the preview and the apply is refused, not half-applied.
#[test]
fn a_replace_target_deleted_after_the_preview_is_refused() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "a.md", "old\n")?;
    let p = plan(
        &f,
        dir.path(),
        vec![write("a.md", WriteAction::Replace, Some("new\n"))],
    )?;
    fs::remove_file(dir.path().join("a.md"))?;
    assert!(matches!(f.library.apply(&p), Err(Error::Install { .. })));
    assert!(files_under(dir.path()).is_empty());
    Ok(())
}

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// Real refits with random choices over originals (some identical to the profile's files),
/// random restores in between: removal always gives the project's files back byte for byte.
#[test]
fn property_removal_after_real_refits_restores_the_originals() -> TestResult {
    let theirs = [
        ("CLAUDE.md", "# Project\n"),
        (AGENT, AGENT_TEXT),
        (
            ".claude/skills/s/SKILL.md",
            "---\nname: s\ndescription: Does a thing.\n---\nBody.\n",
        ),
    ];
    let project_only = [
        (
            ".claude/agents/mine.md",
            "---\nname: mine\ndescription: x\n---\nmine\n",
        ),
        (".claude/skills/s/extra.md", "extra\n"),
        ("README.md", "readme\n"),
    ];
    for seed in 0..40u64 {
        let mut rng = Rng(seed + 7);
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        let mut originals: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (path, text) in theirs {
            match rng.below(3) {
                0 => {}
                1 => {
                    originals.insert(path.to_owned(), text.as_bytes().to_vec());
                }
                _ => {
                    originals.insert(
                        path.to_owned(),
                        format!("mine {seed} {path}\n").into_bytes(),
                    );
                }
            }
        }
        for (path, text) in project_only {
            if rng.below(2) == 0 {
                originals.insert(path.to_owned(), text.as_bytes().to_vec());
            }
        }
        for (path, bytes) in &originals {
            put(dir.path(), path, &String::from_utf8_lossy(bytes))?;
        }
        let mut project_id = None;
        for round in 0..=rng.below(2) {
            let comparison = f.library.compare(dir.path(), f.profile, None)?;
            let mut choices = BTreeMap::new();
            for row in &comparison.rows {
                let pick = row.options[usize::try_from(rng.below(row.options.len() as u64))?];
                choices.insert(row.item.clone(), pick);
            }
            let plan = f
                .library
                .refit_plan(dir.path(), f.profile, None, &choices)?;
            assert_eq!(plan.kind, PlanKind::Refit);
            let report = f.library.apply(&plan)?;
            project_id = Some(report.project_id);
            if round == 0 && rng.below(2) == 0 {
                // Undo the first refit with a restore of its set, then work on from there.
                if let Some(set) = f.library.backups(report.project_id)?.first() {
                    f.library.restore_backup(set.id)?;
                }
            }
        }
        let project_id = project_id.ok_or("a project")?;
        let removal = f.library.plan_removal(project_id)?;
        f.library.apply(&removal)?;
        let expected: Vec<String> = originals.keys().cloned().collect();
        assert_eq!(files_under(dir.path()), expected, "seed {seed}");
        for (path, bytes) in &originals {
            assert_eq!(&read(dir.path(), path)?, bytes, "seed {seed}: {path}");
        }
    }
    Ok(())
}
