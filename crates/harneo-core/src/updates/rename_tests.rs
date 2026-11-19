//! Renames at the source (decision 10): detected by the check, applied by *Update all* or the
//! drawer's Update, carrying tags, collections, links, profiles, installed files, fits and
//! usage over. Local git repositories only.

use std::fs;
use std::io;

use super::tests::{agent, commit, fixture, git, write, Fixture, TestResult};
use super::*;
use crate::artifact::ArtifactKind;
use crate::harnmap::CategoryId;
use crate::library::{ImportHooks, TagColor};
use crate::profiles::ProfileSpec;
use crate::usage::UsageIndex;

struct FailAtCommit;
impl ImportHooks for FailAtCommit {
    fn before_commit(&self) -> io::Result<()> {
        Err(io::Error::other("injected at commit"))
    }
}

/// Renames agent `b` to `b2` in the repo: the file and the `name:` line change, nothing else.
fn rename_b(f: &Fixture) -> io::Result<()> {
    git(f.repo.path(), &["mv", "agents/b.md", "agents/b2.md"])?;
    write(f.repo.path(), "agents/b2.md", &b_as("b2", "one"))?;
    commit(f.repo.path(), "rename b to b2")?;
    Ok(())
}

/// Agent `b` under another name: the description is `b`'s own, so only the name differs.
fn b_as(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: b agent\n---\n{body}\n")
}

fn changed(f: &Fixture, report: &CheckReport) -> (u32, u32, u32) {
    match f.one(report) {
        SourceResult::Changed {
            removed,
            new_upstream,
            renamed,
            ..
        } => (removed, new_upstream, renamed),
        other => panic!("expected a change, got {other:?}"),
    }
}

fn count(f: &Fixture, sql: &str) -> std::result::Result<i64, rusqlite::Error> {
    f.library.store.conn.query_row(sql, [], |r| r.get(0))
}

#[test]
fn a_rename_is_reported_as_a_rename_and_not_as_removed_plus_new() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    rename_b(&f)?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (0, 0, 1));

    let status = f.library.update_status(&b)?.ok_or("flagged")?;
    assert_eq!(status.state, UpstreamState::Renamed);
    assert_eq!(status.renamed_to.as_deref(), Some("b2"));
    assert!(f.library.removed_at_source_plan()?.artifacts.is_empty());
    let plan = f.library.renamed_at_source_plan()?;
    assert_eq!(plan.renames.len(), 1);
    assert_eq!(
        (plan.renames[0].id.as_str(), plan.renames[0].name.as_str()),
        (b.as_str(), "b")
    );
    assert_eq!(plan.renames[0].new_name, "b2");
    assert!(!plan.renames[0].local_edits);
    let item = f
        .library
        .list(&crate::library::Query::default())?
        .items
        .into_iter()
        .find(|i| i.id == b)
        .ok_or("listed")?;
    assert_eq!(
        item.update.ok_or("a badge")?.renamed_to.as_deref(),
        Some("b2")
    );
    // The remote did not move: the stored answer stays.
    let again = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &again), (0, 0, 1));
    Ok(())
}

#[test]
fn applying_a_rename_carries_everything_over_and_removes_the_old_copy() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let (a, b) = (f.id_of("a")?, f.id_of("b")?);
    let lib = &f.library;
    let tag = lib.create_tag("keep", TagColor::Cyan)?;
    lib.tag_artifact(&b, tag.id)?;
    let collection = lib.create_manual_collection("mine")?;
    lib.add_to_collection(collection.id, &b)?;
    lib.add_link(&a, &b, crate::bundles::LinkOrigin::Manual, "a needs b")?;
    let spec = ProfileSpec {
        items: vec![a.clone(), b.clone()],
        ..ProfileSpec::default()
    };
    let profile = lib.create_profile("web", "", &spec, CategoryId::Software, None)?;
    let conn = &lib.store.conn;
    conn.execute(
        "INSERT INTO harnfit_fits
            (profile_id, candidate, created_at, model, duration_ms, verdict, basis)
         VALUES (?1, ?2, 1, 'm', 1, '{}', '{}')",
        rusqlite::params![profile, b],
    )?;
    conn.execute(
        "INSERT INTO projects (path, name, profile_id, profile_version, applied_at)
         VALUES ('/p', 'p', ?1, 1, 1)",
        [profile],
    )?;
    conn.execute(
        "INSERT INTO installed_files (project_id, path, digest, artifact_id)
         VALUES (1, '.claude/agents/b.md', 'd', ?1)",
        [&b],
    )?;
    conn.execute(
        "INSERT INTO usage.uses (session_id, day, project_path, kind, name)
         VALUES ('s1', date('now'), '', 'agent', 'b')",
        [],
    )?;
    let imported_at: i64 = conn.query_row(
        "SELECT imported_at FROM artifacts WHERE id = ?1",
        [&b],
        |r| r.get(0),
    )?;

    rename_b(&f)?;
    f.library.check_updates(&CheckScope::All)?;
    let reports = f.library.update_all()?;
    assert_eq!(reports.len(), 1, "{reports:?}");
    let report = &reports[0];
    assert_eq!(report.outcome, UpdateOutcome::Renamed);
    assert_eq!(report.id, b);
    assert_eq!(report.name.as_deref(), Some("b2"));
    assert_eq!(report.source_path.as_deref(), Some("agents/b2.md"));

    // One artifact with the new name, path and id; the old one is gone, folder included.
    let new = f.library.find(ArtifactKind::Agent, "b2")?.ok_or("b2")?;
    assert_ne!(new, b);
    assert!(f.library.find(ArtifactKind::Agent, "b")?.is_none());
    assert!(f.library.detail(&b).is_err());
    let detail = f.library.detail(&new)?;
    assert_eq!(detail.provenance.source_path, "agents/b2.md");
    assert!(detail.audit.is_some());
    assert_eq!(detail.provenance.imported_at, imported_at);
    let artifacts = f.data.path().join("library/artifacts");
    assert!(!artifacts.join(&b).exists());
    assert!(artifacts.join(&new).join("b2.md").is_file());

    // What pointed at the old id points at the new one.
    let lib = &f.library;
    let tags = lib.tags()?;
    assert_eq!(tags.iter().map(|t| t.count).sum::<u32>(), 1, "{tags:?}");
    assert_eq!(lib.artifact_collections(&new)?, vec![collection.id]);
    assert_eq!(lib.links_of(&new)?.needed_by.len(), 1);
    assert_eq!(lib.links_of(&a)?.needs.len(), 1);
    let view = lib.profile(profile, None)?;
    assert_eq!(view.current_version, 2, "a new version names the new id");
    assert_eq!(view.spec.items, vec![a.clone(), new.clone()]);
    assert_eq!(
        count(
            &f,
            &format!("SELECT COUNT(*) FROM harnfit_fits WHERE candidate = '{new}'")
        )?,
        1
    );
    assert_eq!(
        count(
            &f,
            &format!("SELECT COUNT(*) FROM installed_files WHERE artifact_id = '{new}'")
        )?,
        1
    );
    // No tombstone for the old path: it was not removed on purpose.
    assert_eq!(count(&f, "SELECT COUNT(*) FROM removed_paths")?, 0);

    // The uses logged under the old name count for the new one, with new ones added.
    let usage = UsageIndex::open(f.data.path())?;
    assert_eq!(usage.usage_of(ArtifactKind::Agent, "b2")?.sessions_8w, 1);
    f.library.store.conn.execute(
        "INSERT INTO usage.uses (session_id, day, project_path, kind, name)
         VALUES ('s2', date('now'), '', 'agent', 'b2')",
        [],
    )?;
    assert_eq!(usage.usage_of(ArtifactKind::Agent, "b2")?.sessions_8w, 2);
    let listed = f
        .library
        .list(&crate::library::Query::default())?
        .items
        .into_iter()
        .find(|i| i.id == new)
        .ok_or("listed")?;
    assert_eq!(listed.usage.ok_or("usage")?.sessions, 2);

    // Nothing is left to do.
    let again = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&again), SourceResult::Current);
    Ok(())
}

#[test]
fn a_renamed_skill_folder_is_a_rename_too() -> TestResult {
    let mut f = fixture()?;
    let skill = |name: &str| format!("---\nname: {name}\ndescription: does a thing\n---\nSteps.\n");
    write(f.repo.path(), "skills/old/SKILL.md", &skill("old"))?;
    write(f.repo.path(), "skills/old/run.sh", "echo hi\n")?;
    commit(f.repo.path(), "add a skill")?;
    f.import(None, None)?;
    let old = f.library.find(ArtifactKind::Skill, "old")?.ok_or("old")?;
    git(f.repo.path(), &["mv", "skills/old", "skills/fresh"])?;
    write(f.repo.path(), "skills/fresh/SKILL.md", &skill("fresh"))?;
    commit(f.repo.path(), "rename the skill")?;

    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (0, 0, 1));
    let reports = f.library.update_all()?;
    assert_eq!(reports[0].outcome, UpdateOutcome::Renamed, "{reports:?}");
    let new = f.library.find(ArtifactKind::Skill, "fresh")?.ok_or("new")?;
    assert!(f.library.detail(&old).is_err());
    assert_eq!(
        f.library.detail(&new)?.provenance.source_path,
        "skills/fresh"
    );
    Ok(())
}

#[test]
fn a_rename_with_other_changes_stays_removed_plus_new() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    git(f.repo.path(), &["mv", "agents/b.md", "agents/b2.md"])?;
    write(f.repo.path(), "agents/b2.md", &b_as("b2", "and more"))?;
    commit(f.repo.path(), "rename b and edit it")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (1, 1, 0));
    assert_eq!(f.library.removed_at_source_plan()?.artifacts.len(), 1);
    assert!(f.library.renamed_at_source_plan()?.renames.is_empty());
    Ok(())
}

#[test]
fn two_candidates_with_the_same_contents_are_not_a_rename() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    git(f.repo.path(), &["mv", "agents/b.md", "agents/b2.md"])?;
    write(f.repo.path(), "agents/b2.md", &b_as("b2", "one"))?;
    write(f.repo.path(), "agents/b3.md", &b_as("b3", "one"))?;
    commit(f.repo.path(), "b2 and b3")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (1, 2, 0));
    Ok(())
}

#[test]
fn two_removed_with_the_same_contents_are_not_a_rename() -> TestResult {
    let mut f = fixture()?;
    write(f.repo.path(), "agents/c.md", &b_as("c", "one"))?;
    commit(f.repo.path(), "add c, the same as b")?;
    f.import(None, None)?;
    git(f.repo.path(), &["rm", "-q", "agents/b.md", "agents/c.md"])?;
    write(f.repo.path(), "agents/d.md", &b_as("d", "one"))?;
    commit(f.repo.path(), "b and c gone, d new")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (2, 1, 0));
    Ok(())
}

#[test]
fn a_different_kind_is_not_a_rename() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    git(f.repo.path(), &["rm", "-q", "agents/b.md"])?;
    write(
        f.repo.path(),
        "skills/b2/SKILL.md",
        "---\nname: b2\ndescription: b2 agent\n---\none\n",
    )?;
    commit(f.repo.path(), "b becomes a skill")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(changed(&f, &report), (1, 1, 0));
    Ok(())
}

#[test]
fn update_all_leaves_an_edited_old_copy_and_does_not_import_its_new_place() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    // The library copy was edited (the name line), which the rename must not overwrite silently.
    fs::write(
        f.data
            .path()
            .join("library/artifacts")
            .join(&b)
            .join("b.md"),
        // Only the name line differs from upstream's copy, so it still counts as a rename.
        b_as("bee", "one"),
    )?;
    write(f.repo.path(), "agents/fresh.md", &agent("fresh", "x"))?;
    rename_b(&f)?;
    f.library.check_updates(&CheckScope::All)?;
    assert!(f.library.renamed_at_source_plan()?.renames[0].local_edits);

    let reports = f.library.update_all()?;
    let outcomes: Vec<(&str, UpdateOutcome)> = reports
        .iter()
        .map(|r| (r.name.as_deref().unwrap_or(r.id.as_str()), r.outcome))
        .collect();
    assert!(
        outcomes.contains(&("fresh", UpdateOutcome::Imported)),
        "{outcomes:?}"
    );
    assert!(
        reports
            .iter()
            .any(|r| r.id == b && r.outcome == UpdateOutcome::SkippedLocalEdits),
        "{outcomes:?}"
    );
    assert!(f.library.detail(&b).is_ok(), "the edited copy stays");
    assert!(
        f.library.find(ArtifactKind::Agent, "b2")?.is_none(),
        "the new place is not imported as a new artifact"
    );

    // From the drawer the user applies it: the edited copy is backed up first.
    let report = f.library.update(&b, UpdateChoice::TakeUpstream)?;
    assert_eq!(report.outcome, UpdateOutcome::Renamed);
    assert!(report.backup.is_some());
    assert!(f.library.find(ArtifactKind::Agent, "b2")?.is_some());
    assert!(f.library.detail(&b).is_err());
    Ok(())
}

#[test]
fn a_failure_while_renaming_leaves_the_old_artifact_intact() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    let tag = f.library.create_tag("keep", TagColor::Sky)?;
    f.library.tag_artifact(&b, tag.id)?;
    let spec = ProfileSpec {
        items: vec![b.clone()],
        ..ProfileSpec::default()
    };
    let profile = f
        .library
        .create_profile("web", "", &spec, CategoryId::Software, None)?;
    let folders = fs::read_dir(f.data.path().join("library/artifacts"))?.count();
    rename_b(&f)?;
    f.library.check_updates(&CheckScope::All)?;

    let outcome = f
        .library
        .update_with(&b, UpdateChoice::TakeUpstream, &FailAtCommit);
    assert!(outcome.is_err());

    assert!(f.library.detail(&b).is_ok());
    assert!(f.library.find(ArtifactKind::Agent, "b2")?.is_none());
    let artifacts = f.data.path().join("library/artifacts");
    assert!(artifacts.join(&b).join("b.md").is_file(), "old folder back");
    assert_eq!(
        fs::read_dir(&artifacts)?.count(),
        folders,
        "no new folder left"
    );
    assert_eq!(
        f.library.profile(profile, None)?.spec.items,
        vec![b.clone()]
    );
    assert_eq!(f.library.profile(profile, None)?.current_version, 1);
    assert_eq!(f.library.tags()?.iter().map(|t| t.count).sum::<u32>(), 1);
    assert_eq!(
        f.library.update_status(&b)?.ok_or("flag")?.state,
        UpstreamState::Renamed,
        "still flagged, so it can be retried"
    );
    assert_eq!(count(&f, "SELECT COUNT(*) FROM usage.aliases")?, 0);
    // And the retry works.
    assert_eq!(
        f.library.update(&b, UpdateChoice::TakeUpstream)?.outcome,
        UpdateOutcome::Renamed
    );
    Ok(())
}

#[test]
fn keeping_mine_does_not_forget_a_rename() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    rename_b(&f)?;
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        f.library.update(&b, UpdateChoice::KeepMine)?.outcome,
        UpdateOutcome::Kept
    );
    assert_eq!(
        f.library.update_status(&b)?.ok_or("flag")?.state,
        UpstreamState::Renamed
    );
    Ok(())
}

#[test]
fn a_rename_back_and_a_second_rename_keep_every_usage_name_counted() -> TestResult {
    let f = fixture()?;
    let conn = &f.library.store.conn;
    crate::usage::add_alias(conn, ArtifactKind::Agent, "one", "two")?;
    crate::usage::add_alias(conn, ArtifactKind::Agent, "two", "three")?;
    let names = |sql: &str| -> std::result::Result<Vec<String>, rusqlite::Error> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    };
    assert_eq!(
        names("SELECT alias || '>' || name FROM usage.aliases ORDER BY alias")?,
        vec!["one>three", "two>three"]
    );
    // Back to the first name: it stops being an alias of itself.
    crate::usage::add_alias(conn, ArtifactKind::Agent, "three", "one")?;
    assert_eq!(
        names("SELECT alias || '>' || name FROM usage.aliases ORDER BY alias")?,
        vec!["three>one", "two>one"]
    );
    Ok(())
}
