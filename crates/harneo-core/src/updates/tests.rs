//! The update check against local git repositories (reached by `file://` URL) and local folders.
//! Nothing here touches the network. A wrapper around `git` logs every call, so the tests can
//! count the clones.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;
use crate::library::{Filter, Query, Source, TagChoices};

pub(super) type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

pub(super) fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub(super) fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

pub(super) fn agent(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {name} agent\n---\n{body}\n")
}

pub(super) fn commit(dir: &Path, message: &str) -> io::Result<String> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", message])?;
    git(dir, &["rev-parse", "HEAD"])
}

pub(super) struct Fixture {
    pub repo: tempfile::TempDir,
    pub url: String,
    pub first: String,
    pub log: PathBuf,
    pub data: tempfile::TempDir,
    pub library: Library,
    _bin: tempfile::TempDir,
}

impl Fixture {
    /// How many `git clone` runs the library has made so far.
    pub fn clones(&self) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.split_whitespace().any(|w| w == "clone"))
            .count()
    }

    pub fn import(&mut self, branch: Option<&str>, sub: Option<&str>) -> Result<Vec<String>> {
        let source = Source::parse_git(&self.url, branch, sub)?;
        let preview = self.library.preview(&source)?;
        let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        self.library.import(&preview, &ids, &TagChoices::new())?;
        Ok(ids)
    }

    pub fn id_of(&self, name: &str) -> Result<String> {
        let found = self
            .library
            .find(crate::artifact::ArtifactKind::Agent, name)?
            .ok_or(Error::NotInLibrary {
                id: name.to_owned(),
            })?;
        Ok(found)
    }

    pub fn one(&self, report: &CheckReport) -> SourceResult {
        assert_eq!(report.sources.len(), 1, "{report:?}");
        report.sources[0].result.clone()
    }
}

/// A repo with agents `a` and `b` on `main`, and `pack/agents/inner.md`, one commit.
pub(super) fn fixture() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let repo = tempfile::tempdir()?;
    let dir = repo.path();
    git(dir, &["init", "-q", "-b", "main"])?;
    write(dir, "agents/a.md", &agent("a", "one"))?;
    write(dir, "agents/b.md", &agent("b", "one"))?;
    write(dir, "pack/agents/inner.md", &agent("inner", "one"))?;
    let first = commit(dir, "one")?;
    let bin = tempfile::tempdir()?;
    let log = bin.path().join("calls.log");
    let script = bin.path().join("logging-git");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec git \"$@\"\n",
            log.display()
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    }
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    library.git.program = script.into_os_string();
    Ok(Fixture {
        url: format!("file://{}", dir.display()),
        repo,
        first,
        log,
        data,
        library,
        _bin: bin,
    })
}

#[test]
fn only_the_artifact_whose_own_files_changed_is_outdated() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let (a, b) = (f.id_of("a")?, f.id_of("b")?);
    let second = commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        f.one(&report),
        SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }
    );
    assert_eq!(f.library.update_status(&a)?, None, "a was not touched");
    let status = f.library.update_status(&b)?.expect("b is outdated");
    assert_eq!(status.state, UpstreamState::Outdated);
    assert_eq!(status.from_version, Some(f.first[..7].to_owned()));
    assert_eq!(status.to_version, Some(second[..7].to_owned()));
    assert!(!status.local_edits);
    Ok(())
}

fn commit_change(f: &Fixture, rel: &str, contents: &str) -> io::Result<String> {
    write(f.repo.path(), rel, contents)?;
    commit(f.repo.path(), "change")
}

#[test]
fn the_version_field_wins_over_commits() -> TestResult {
    let mut f = fixture()?;
    write(
        f.repo.path(),
        "agents/v.md",
        "---\nname: v\ndescription: v\nversion: 1.0.0\n---\nx\n",
    )?;
    commit(f.repo.path(), "add v")?;
    f.import(None, None)?;
    let id = f.id_of("v")?;
    commit_change(
        &f,
        "agents/v.md",
        "---\nname: v\ndescription: v\nversion: 1.1.0\n---\nx\n",
    )?;
    f.library.check_updates(&CheckScope::All)?;
    let status = f.library.update_status(&id)?.expect("outdated");
    assert_eq!(status.from_version.as_deref(), Some("1.0.0"));
    assert_eq!(status.to_version.as_deref(), Some("1.1.0"));
    Ok(())
}

#[test]
fn an_unchanged_remote_is_not_cloned_and_a_moved_one_is() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    assert_eq!(f.clones(), 1, "the import clone");
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&report), SourceResult::Current);
    assert_eq!(f.clones(), 1, "unchanged remote: ls-remote only");

    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.clones(), 2, "the remote moved: one clone");
    // The commit was recorded, so the next check does not clone again and still reports b.
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.clones(), 2);
    assert_eq!(
        f.one(&report),
        SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }
    );
    Ok(())
}

#[test]
fn the_clone_is_kept_for_the_session_and_removed_with_the_library() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    let clones = |data: &Path| -> io::Result<usize> {
        let mut n = 0;
        for entry in fs::read_dir(data.join("library"))? {
            if entry?.file_name().to_string_lossy().starts_with(".clone-") {
                n += 1;
            }
        }
        Ok(n)
    };
    assert_eq!(clones(f.data.path())?, 1);
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(clones(f.data.path())?, 1);
    let Fixture { data, library, .. } = f;
    drop(library);
    assert_eq!(clones(data.path())?, 0);
    Ok(())
}

#[test]
fn a_subfolder_source_ignores_commits_outside_it() -> TestResult {
    let mut f = fixture()?;
    f.import(None, Some("pack"))?;
    let inner = f.id_of("inner")?;
    commit_change(&f, "agents/a.md", &agent("a", "two"))?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&report), SourceResult::Current);
    assert_eq!(f.clones(), 2, "the remote moved, so it was looked at");
    assert_eq!(f.library.update_status(&inner)?, None);

    commit_change(&f, "pack/agents/inner.md", &agent("inner", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert!(f.library.update_status(&inner)?.is_some());
    Ok(())
}

#[test]
fn a_branch_source_follows_its_own_branch() -> TestResult {
    let mut f = fixture()?;
    git(f.repo.path(), &["checkout", "-q", "-b", "dev"])?;
    commit_change(&f, "agents/c.md", &agent("c", "dev"))?;
    git(f.repo.path(), &["checkout", "-q", "main"])?;
    f.import(Some("dev"), None)?;
    let (a, c) = (f.id_of("a")?, f.id_of("c")?);
    // main moves; the dev source is not affected.
    commit_change(&f, "agents/a.md", &agent("a", "main moved"))?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&report), SourceResult::Current);
    assert_eq!(f.clones(), 1, "dev did not move: no clone");
    // dev moves.
    git(f.repo.path(), &["checkout", "-q", "dev"])?;
    commit_change(&f, "agents/c.md", &agent("c", "dev two"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert!(f.library.update_status(&c)?.is_some());
    assert_eq!(f.library.update_status(&a)?, None, "a on dev is unchanged");
    Ok(())
}

#[test]
fn removed_and_new_upstream_are_reported() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    git(f.repo.path(), &["rm", "-q", "agents/b.md"])?;
    write(f.repo.path(), "agents/new.md", &agent("new", "x"))?;
    commit(f.repo.path(), "swap")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        f.one(&report),
        SourceResult::Changed {
            outdated: 0,
            removed: 1,
            layout_changed: 0,
            new_upstream: 1
        }
    );
    let status = f.library.update_status(&b)?.expect("removed");
    assert_eq!(status.state, UpstreamState::Removed);
    assert!(f.library.detail(&b).is_ok(), "never deleted automatically");
    Ok(())
}

#[test]
fn an_unreachable_source_does_not_stop_the_others() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    // A second source: a local folder.
    let local = tempfile::tempdir()?;
    write(local.path(), "agents/l.md", &agent("l", "one"))?;
    let preview = f.library.preview(&Source::Local {
        path: local.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    write(local.path(), "agents/l.md", &agent("l", "two"))?;
    fs::remove_dir_all(f.repo.path().join(".git"))?;

    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(report.sources.len(), 2);
    let unreachable = report
        .sources
        .iter()
        .filter(|s| matches!(s.result, SourceResult::Unreachable { .. }))
        .count();
    assert_eq!(unreachable, 1, "{report:?}");
    assert!(report.sources.iter().any(|s| s.result
        == SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }));
    Ok(())
}

#[test]
fn a_missing_folder_or_subfolder_is_reported_missing() -> TestResult {
    let mut f = fixture()?;
    f.import(None, Some("pack"))?;
    git(f.repo.path(), &["rm", "-rq", "pack"])?;
    commit(f.repo.path(), "drop pack")?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(f.one(&report), SourceResult::Missing);

    let local = tempfile::tempdir()?;
    write(local.path(), "agents/l.md", &agent("l", "one"))?;
    let source = Source::Local {
        path: local.path().to_path_buf(),
    };
    let preview = f.library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    drop(local);
    let report = f.library.check_updates(&CheckScope::All)?;
    let missing = report
        .sources
        .iter()
        .filter(|s| s.result == SourceResult::Missing)
        .count();
    assert_eq!(missing, 2, "{report:?}");
    Ok(())
}

#[test]
fn a_missing_branch_is_unreachable_with_a_reason() -> TestResult {
    let mut f = fixture()?;
    git(f.repo.path(), &["checkout", "-q", "-b", "dev"])?;
    commit_change(&f, "agents/c.md", &agent("c", "dev"))?;
    f.import(Some("dev"), None)?;
    git(f.repo.path(), &["checkout", "-q", "main"])?;
    git(f.repo.path(), &["branch", "-q", "-D", "dev"])?;
    let report = f.library.check_updates(&CheckScope::All)?;
    let SourceResult::Unreachable { reason } = f.one(&report) else {
        panic!("{report:?}");
    };
    assert!(reason.contains("dev"), "{reason}");
    Ok(())
}

#[test]
fn the_artifact_scope_judges_only_that_artifact() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let (a, b) = (f.id_of("a")?, f.id_of("b")?);
    commit_change(&f, "agents/a.md", &agent("a", "two"))?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library
        .check_updates(&CheckScope::Artifact { id: a.clone() })?;
    assert!(f.library.update_status(&a)?.is_some());
    assert_eq!(f.library.update_status(&b)?, None);
    let err = f
        .library
        .check_updates(&CheckScope::Artifact { id: "nope".into() })
        .expect_err("unknown");
    assert!(matches!(err, Error::NotInLibrary { .. }));
    Ok(())
}

#[test]
fn an_artifact_check_never_hides_the_other_changed_artifacts() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let (a, b) = (f.id_of("a")?, f.id_of("b")?);
    commit_change(&f, "agents/a.md", &agent("a", "two"))?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library
        .check_updates(&CheckScope::Artifact { id: a.clone() })?;
    assert_eq!(f.library.update_status(&b)?, None);
    f.library.check_updates(&CheckScope::All)?;
    assert!(f.library.update_status(&a)?.is_some());
    assert!(f.library.update_status(&b)?.is_some(), "b is flagged too");
    Ok(())
}

#[test]
fn a_source_scope_checks_only_that_source() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let source_id: String = f
        .library
        .store
        .conn
        .query_row("SELECT id FROM sources", [], |r| r.get(0))?;
    let none = f
        .library
        .check_updates(&CheckScope::Source { id: "other".into() })?;
    assert!(none.sources.is_empty());
    let one = f.library.check_updates(&CheckScope::Source {
        id: source_id.clone(),
    })?;
    assert_eq!(one.sources[0].id, source_id);
    Ok(())
}

#[test]
fn a_local_source_is_rescanned_in_place() -> TestResult {
    let mut f = fixture()?;
    let local = tempfile::tempdir()?;
    write(local.path(), "agents/l.md", &agent("l", "one"))?;
    write(local.path(), "agents/m.md", &agent("m", "one"))?;
    let preview = f.library.preview(&Source::Local {
        path: local.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(report.sources[0].result, SourceResult::Current);
    write(local.path(), "agents/l.md", &agent("l", "two"))?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        report.sources[0].result,
        SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }
    );
    let status = f.library.update_status(&f.id_of("l")?)?.expect("outdated");
    assert_eq!((status.from_version, status.to_version), (None, None));
    // Reverting the file clears the flag.
    write(local.path(), "agents/l.md", &agent("l", "one"))?;
    let report = f.library.check_updates(&CheckScope::All)?;
    assert_eq!(report.sources[0].result, SourceResult::Current);
    assert_eq!(f.library.update_status(&f.id_of("l")?)?, None);
    Ok(())
}

#[test]
fn a_skipped_version_stays_quiet_until_upstream_changes_again() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    f.library.store.conn.execute(
        "UPDATE upstream SET state = 'skipped' WHERE artifact_id = ?1",
        params![b],
    )?;
    // Moving the repo for another artifact re-reads b but keeps it skipped.
    commit_change(&f, "agents/a.md", &agent("a", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        f.library.update_status(&b)?.expect("row").state,
        UpstreamState::Skipped
    );
    commit_change(&f, "agents/b.md", &agent("b", "three"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        f.library.update_status(&b)?.expect("row").state,
        UpstreamState::Outdated
    );
    Ok(())
}

#[test]
fn local_edits_are_detected_and_old_rows_get_a_digest_on_first_check() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    let digest = |f: &Fixture| -> Result<Option<String>> {
        Ok(f.library.store.conn.query_row(
            "SELECT copy_digest FROM artifacts WHERE id = ?1",
            params![b],
            |r| r.get(0),
        )?)
    };
    let recorded = digest(&f)?.expect("recorded at import");
    assert_eq!(
        recorded,
        copy_digest(&f.library.store.artifacts_dir().join(&b))?
    );

    // A row from before the digest existed: filled by the first check, counted as unedited.
    f.library
        .store
        .conn
        .execute("UPDATE artifacts SET copy_digest = NULL", [])?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    assert_eq!(digest(&f)?, Some(recorded));
    assert!(!f.library.update_status(&b)?.expect("outdated").local_edits);

    // Editing the library copy shows as a local edit, on the status and on the list badge.
    let file = f.library.store.artifacts_dir().join(&b).join("b.md");
    fs::write(&file, "edited")?;
    assert!(f.library.update_status(&b)?.expect("outdated").local_edits);
    let page = f.library.list(&Query::default())?;
    let item = page.items.iter().find(|i| i.id == b).expect("listed");
    assert!(item.update.as_ref().expect("badge").local_edits);
    Ok(())
}

#[test]
fn the_list_filters_and_counts_outdated_artifacts() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    let all = f.library.list(&Query::default())?;
    assert_eq!(all.facets.outdated, 1);
    assert_eq!(all.items.iter().filter(|i| i.update.is_some()).count(), 1);
    let query = Query {
        filter: Filter {
            outdated: true,
            ..Filter::default()
        },
        ..Query::default()
    };
    let page = f.library.list(&query)?;
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, b);
    assert_eq!(page.facets.outdated, 1, "the count ignores its own filter");
    assert_eq!(page.unfiltered_total, 2);
    Ok(())
}

#[test]
fn removing_an_artifact_drops_its_upstream_row() -> TestResult {
    let mut f = fixture()?;
    f.import(None, None)?;
    let b = f.id_of("b")?;
    commit_change(&f, "agents/b.md", &agent("b", "two"))?;
    f.library.check_updates(&CheckScope::All)?;
    f.library.remove(&b)?;
    let rows: i64 = f
        .library
        .store
        .conn
        .query_row("SELECT COUNT(*) FROM upstream", [], |r| r.get(0))?;
    assert_eq!(rows, 0);
    assert!(matches!(
        f.library.update_status(&b),
        Err(Error::NotInLibrary { .. })
    ));
    Ok(())
}
