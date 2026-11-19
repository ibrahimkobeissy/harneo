//! *Update all* imports what is new at a source, except what the user deleted (decision 8).
//! Local git repositories only; Claude is never involved.

use super::tests::{agent, commit, fixture, git, write, Fixture, TestResult};
use super::*;
use crate::artifact::ArtifactKind;
use crate::library::{Source, TagChoices};

fn removed(f: &Fixture) -> Result<usize> {
    let source = f.library.list(&crate::library::Query::default())?.items[0]
        .source
        .id
        .clone();
    Ok(records::removed_paths(&f.library.store.conn, &source)?.len())
}

fn new_count(f: &Fixture, report: &CheckReport) -> u32 {
    match f.one(report) {
        SourceResult::Changed { new_upstream, .. } => new_upstream,
        _ => 0,
    }
}

#[test]
fn a_new_artifact_is_imported_audited_and_keeps_its_provenance() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    write(f.repo.path(), "agents/fresh.md", &agent("fresh", "x"))?;
    let head = commit(f.repo.path(), "add fresh")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(new_count(&f, &report), 1);

    let reports = f.library.update_all()?;
    assert_eq!(reports.len(), 1, "{reports:?}");
    let imported = &reports[0];
    assert_eq!(imported.outcome, UpdateOutcome::Imported);
    assert_eq!(imported.name.as_deref(), Some("fresh"));
    assert_eq!(imported.source_path.as_deref(), Some("agents/fresh.md"));

    let detail = f.library.detail(&imported.id)?;
    assert_eq!(detail.provenance.source_path, "agents/fresh.md");
    assert!(detail.audit.is_some(), "audited like an import");
    assert_eq!(
        detail
            .artifact
            .metadata
            .get(IMPORTED_COMMIT)
            .and_then(serde_json::Value::as_str),
        Some(head.as_str()),
        "the commit it came from"
    );
    assert_eq!(f.id_of("fresh")?, imported.id);
    // Nothing is new any more, and an unchanged remote does not bring the count back.
    let again = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&again), SourceResult::Current);
    Ok(())
}

#[test]
fn an_unchanged_remote_keeps_counting_what_is_new() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    write(f.repo.path(), "agents/fresh.md", &agent("fresh", "x"))?;
    commit(f.repo.path(), "add fresh")?;
    f.library.check_updates(&CheckScope::All)?;
    let clones = f.clones();
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.clones(), clones, "the remote did not move: no clone");
    assert_eq!(new_count(&f, &report), 1);
    Ok(())
}

#[test]
fn a_deleted_artifact_is_neither_counted_nor_imported() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    f.library.remove(&b)?;
    assert_eq!(removed(&f)?, 1);
    // Something changed upstream so the source is read again.
    write(f.repo.path(), "agents/a.md", &agent("a", "two"))?;
    commit(f.repo.path(), "touch a")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        new_count(&f, &report),
        0,
        "b is still upstream, but deleted"
    );

    let reports = f.library.update_all()?;
    assert!(reports.iter().all(|r| r.outcome == UpdateOutcome::Updated));
    assert!(f.library.find(ArtifactKind::Agent, "b")?.is_none());
    Ok(())
}

#[test]
fn importing_a_deleted_path_on_purpose_clears_its_tombstone() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    f.library.remove(&b)?;
    assert_eq!(removed(&f)?, 1);

    let source = Source::parse_git(&f.url, None, None)?;
    let preview = f.library.preview(&source)?;
    let again = preview
        .candidates
        .iter()
        .find(|c| c.artifact.name == "b")
        .map(|c| c.id.clone())
        .ok_or("b is in the preview")?;
    f.library.import(&preview, &[again], &TagChoices::new())?;
    assert_eq!(removed(&f)?, 0, "the explicit import cleared it");
    assert_eq!(f.id_of("b")?, b);
    Ok(())
}

#[test]
fn a_removed_source_keeps_its_tombstones() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let ids: Vec<String> = f
        .library
        .list(&crate::library::Query::default())?
        .items
        .into_iter()
        .map(|i| i.id)
        .collect();
    f.library.remove_many(&ids)?;
    let rows: i64 =
        f.library
            .store
            .conn
            .query_row("SELECT COUNT(*) FROM removed_paths", [], |r| r.get(0))?;
    assert_eq!(
        usize::try_from(rows)?,
        ids.len(),
        "the source row went with its last artifact, the tombstones stay"
    );
    Ok(())
}

#[test]
fn a_new_local_artifact_is_imported_without_a_commit() -> TestResult {
    let src = tempfile::tempdir()?;
    write(src.path(), "agents/a.md", &agent("a", "one"))?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    write(src.path(), "agents/z.md", &agent("z", "x"))?;
    library.check_updates(&CheckScope::All)?;
    let reports = library.update_all()?;
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].outcome, UpdateOutcome::Imported);
    let detail = library.detail(&reports[0].id)?;
    assert!(detail.artifact.metadata.get(IMPORTED_COMMIT).is_none());
    Ok(())
}

#[test]
fn update_all_deletes_what_is_removed_at_source_only_with_the_flag() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let (a, b) = (f.id_of("a")?, f.id_of("b")?);
    git(f.repo.path(), &["rm", "-q", "agents/b.md"])?;
    commit(f.repo.path(), "drop b")?;
    f.library.check_updates(&CheckScope::All)?;

    let plan = f.library.removed_at_source_plan()?;
    assert_eq!(plan.artifacts.len(), 1);
    assert_eq!(plan.artifacts[0].id, b);

    // Without the flag nothing is deleted.
    assert!(f.library.update_all()?.is_empty());
    assert!(f.library.detail(&b).is_ok());

    let reports = f.library.update_all_with(true)?;
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert_eq!(reports[0].outcome, UpdateOutcome::Removed);
    assert_eq!(reports[0].id, b);
    assert_eq!(reports[0].name.as_deref(), Some("b"));
    assert_eq!(reports[0].source_path.as_deref(), Some("agents/b.md"));
    assert!(f.library.detail(&b).is_err(), "deleted");
    assert!(f.library.detail(&a).is_ok());
    assert_eq!(removed(&f)?, 1, "the delete left a tombstone");
    Ok(())
}

#[test]
fn layout_changed_is_never_deleted_by_update_all() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    git(f.repo.path(), &["rm", "-q", "agents/b.md"])?;
    commit(f.repo.path(), "drop b")?;
    f.library.check_updates(&CheckScope::All)?;
    f.library.store.conn.execute(
        "UPDATE upstream SET state = 'layout_changed' WHERE artifact_id = ?1",
        [&b],
    )?;
    assert!(f.library.removed_at_source_plan()?.artifacts.is_empty());
    assert!(f.library.update_all_with(true)?.is_empty());
    assert!(f.library.detail(&b).is_ok());
    Ok(())
}
