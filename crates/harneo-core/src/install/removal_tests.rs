use std::collections::BTreeMap;
use std::fs;

use super::fixture::{fx, put, TestResult};
use super::fixture_lifecycle::{files_under, install_over, read, refit_plan, without_agent};
use super::{PlanKind, WriteAction};
use crate::error::Error;

const AGENT: &str = ".claude/agents/rev.md";

#[test]
fn removing_a_scaffold_deletes_every_file_and_unregisters_the_project() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "src/main.rs", "fn main() {}\n")?;
    put(dir.path(), "CLAUDE.local.md", "mine\n")?;
    let plan = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    let report = f.library.apply(&plan)?;
    assert!(dir.path().join("HARNESS.md").exists());

    let removal = f.library.plan_removal(report.project_id)?;
    assert_eq!(removal.kind, PlanKind::Removal);
    assert_eq!(removal.reason, "Remove web v1");
    assert!(removal
        .writes
        .iter()
        .all(|w| w.action == WriteAction::Delete));
    assert!(removal.edited_in_project.is_empty());
    let done = f.library.apply(&removal)?;
    assert_eq!(done.deleted as usize, plan.writes.len());
    assert_eq!(done.backup_id, None);
    assert_eq!(files_under(dir.path()), ["CLAUDE.local.md", "src/main.rs"]);
    assert!(
        !dir.path().join(".claude").exists(),
        "emptied folders go too"
    );
    assert!(f.library.projects()?.is_empty());
    assert!(matches!(
        f.library.project(report.project_id),
        Err(Error::ProjectNotFound { .. })
    ));
    // The files that were taken out are still on disk, in the backup folder of the removal.
    let folder = done.backup_folder.ok_or("backup folder")?;
    assert_eq!(
        read(&f.data.path().join(folder), "HARNESS.md")?,
        plan.writes
            .iter()
            .find(|w| w.path == "HARNESS.md")
            .and_then(|w| w.bytes.clone())
            .ok_or("harness")?
    );
    Ok(())
}

#[test]
fn removal_restores_the_originals_and_backs_up_edited_files() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    put(dir.path(), AGENT, "my agent\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    // Edits after the install, and a second apply that backs up Harneo's own versions.
    fs::write(dir.path().join("CLAUDE.md"), "# my own, then edited\n")?;
    fs::write(
        dir.path().join(".claude/skills/s/SKILL.md"),
        "edited skill\n",
    )?;
    without_agent(&f)?;
    let switch = f.library.plan_switch(report.project_id, f.profile, None)?;
    f.library.apply(&switch)?;
    assert_eq!(read(dir.path(), AGENT)?, b"my agent\n");

    let removal = f.library.plan_removal(report.project_id)?;
    assert_eq!(
        removal.edited_in_project,
        [".claude/skills/s/SKILL.md", "CLAUDE.md"]
    );
    assert_eq!(removal.restored, ["CLAUDE.md"]);
    let done = f.library.apply(&removal)?;
    assert_eq!(files_under(dir.path()), [AGENT, "CLAUDE.md"]);
    assert_eq!(read(dir.path(), "CLAUDE.md")?, b"# my own\n");
    assert_eq!(read(dir.path(), AGENT)?, b"my agent\n");
    let kept = f.data.path().join(done.backup_folder.ok_or("folder")?);
    assert_eq!(read(&kept, "CLAUDE.md")?, b"# my own, then edited\n");
    assert_eq!(read(&kept, ".claude/skills/s/SKILL.md")?, b"edited skill\n");
    Ok(())
}

#[test]
fn a_deleted_original_backup_means_the_file_is_deleted_instead() -> TestResult {
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    for backup in f.library.backups(report.project_id)? {
        f.library.delete_backup(backup.id)?;
    }
    let removal = f.library.plan_removal(report.project_id)?;
    assert!(removal.restored.is_empty());
    f.library.apply(&removal)?;
    assert!(files_under(dir.path()).is_empty());
    Ok(())
}

#[test]
fn a_failed_removal_leaves_the_project_registered_and_intact() -> TestResult {
    use std::io;
    struct FailCommit;
    impl super::apply::ApplyHooks for FailCommit {
        fn before_commit(&self) -> io::Result<()> {
            Err(io::Error::other("no commit"))
        }
    }
    let f = fx()?;
    let dir = tempfile::tempdir()?;
    put(dir.path(), "CLAUDE.md", "# my own\n")?;
    let report = install_over(&f, dir.path(), f.profile)?;
    let before: BTreeMap<String, Vec<u8>> = files_under(dir.path())
        .into_iter()
        .map(|p| Ok((p.clone(), read(dir.path(), &p)?)))
        .collect::<io::Result<_>>()?;
    let removal = f.library.plan_removal(report.project_id)?;
    assert!(f.library.apply_with(&removal, &FailCommit).is_err());
    let after: BTreeMap<String, Vec<u8>> = files_under(dir.path())
        .into_iter()
        .map(|p| Ok((p.clone(), read(dir.path(), &p)?)))
        .collect::<io::Result<_>>()?;
    assert_eq!(before, after);
    assert_eq!(f.library.projects()?.len(), 1);
    assert_eq!(
        f.library.installed_files(report.project_id)?.len(),
        before.len()
    );
    Ok(())
}

#[test]
fn removal_of_an_unregistered_project_is_refused() -> TestResult {
    let f = fx()?;
    assert!(matches!(
        f.library.plan_removal(3),
        Err(Error::ProjectNotFound { id: 3 })
    ));
    // A removal plan for a folder that is not registered is refused by apply.
    let dir = tempfile::tempdir()?;
    let mut plan = refit_plan(&f, dir.path(), f.profile)?;
    plan.kind = PlanKind::Removal;
    assert!(matches!(f.library.apply(&plan), Err(Error::Install { .. })));
    Ok(())
}

/// A small seeded generator (the tests must be repeatable, and proptest is not a dependency).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn bytes(&mut self) -> Vec<u8> {
        let len = self.below(40);
        (0..len)
            .map(|_| u8::try_from(self.below(256)).unwrap_or(0))
            .collect()
    }
}

#[test]
fn property_removal_restores_every_original_byte_for_byte() -> TestResult {
    let others = [
        "README.md",
        "src/lib.rs",
        ".claude/notes/todo.txt",
        "docs/a/b/c.md",
    ];
    for seed in 0..30u64 {
        let f = fx()?;
        let harness_paths: Vec<String> = {
            let empty = tempfile::tempdir()?;
            f.library
                .plan_scaffold(empty.path(), f.profile, None)?
                .writes
                .into_iter()
                .map(|w| w.path)
                .collect()
        };
        let mut rng = Rng(seed + 1);
        let dir = tempfile::tempdir()?;
        let mut originals: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for path in harness_paths.iter().map(String::as_str).chain(others) {
            if rng.below(2) == 0 {
                originals.insert(path.to_owned(), rng.bytes());
            }
        }
        originals.insert("CLAUDE.local.md".to_owned(), rng.bytes());
        for (path, bytes) in &originals {
            let full = dir.path().join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(full, bytes)?;
        }
        let report = install_over(&f, dir.path(), f.profile)?;
        // Some projects keep working before the removal: edits to managed files, a switch.
        for path in &harness_paths {
            if rng.below(4) == 0 && dir.path().join(path).exists() {
                fs::write(dir.path().join(path), rng.bytes())?;
            }
        }
        if rng.below(3) == 0 {
            let version = without_agent(&f)?;
            let switch = f
                .library
                .plan_switch(report.project_id, f.profile, Some(version))?;
            f.library.apply(&switch)?;
        }
        let removal = f.library.plan_removal(report.project_id)?;
        f.library.apply(&removal)?;

        let expected: Vec<String> = originals.keys().cloned().collect();
        assert_eq!(files_under(dir.path()), expected, "seed {seed}");
        for (path, bytes) in &originals {
            assert_eq!(&read(dir.path(), path)?, bytes, "seed {seed}: {path}");
        }
        assert!(f.library.projects()?.is_empty(), "seed {seed}");
    }
    Ok(())
}
