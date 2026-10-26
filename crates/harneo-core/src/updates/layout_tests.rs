//! Updates replay the layout Claude proposed at import, without calling Claude. The `claude`
//! CLI is a fixture script that counts its calls; the real one never runs.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;
use crate::bundles::claude_cli_tests::{config, script};
use crate::library::{ArtifactId, Source, TagChoices};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Setup {
    _base: tempfile::TempDir,
    repo: PathBuf,
    bin: tempfile::TempDir,
    library: Library,
    _data: tempfile::TempDir,
}

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

impl Setup {
    fn source(&self) -> Source {
        Source::Local {
            path: self.repo.clone(),
        }
    }

    /// How many times the fake `claude` ran.
    fn calls(&self) -> usize {
        fs::read_dir(self.bin.path())
            .map(|d| {
                d.filter_map(std::result::Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("in-"))
                    .count()
            })
            .unwrap_or(0)
    }

    /// A claude that counts its calls and answers `answer`.
    fn claude(&mut self, answer: &serde_json::Value) -> std::io::Result<()> {
        let d = self.bin.path().display();
        fs::write(self.bin.path().join("answer.json"), answer.to_string())?;
        let program = script(
            self.bin.path(),
            "claude",
            &format!(
                "n=$(ls '{d}' | grep -c '^in-')\ncat > \"{d}/in-$n.txt\"\n\
printf '{{\"is_error\":false,\"structured_output\":'\ncat '{d}/answer.json'\nprintf '}}'"
            ),
        )?;
        self.library.claude = config(&program);
        Ok(())
    }

    fn source_id(&self) -> Result<String> {
        Ok(crate::library::ids::source_id(&self.source().identity()?))
    }

    fn check(&self) -> Result<CheckReport> {
        self.library.check_updates(&CheckScope::All)
    }

    fn id_of(&self, path: &str) -> Result<String> {
        Ok(crate::library::ids::artifact_id(
            &self.source().identity()?,
            path,
        ))
    }
}

fn prompt(kind: &str, path: &str) -> serde_json::Value {
    json!({"kind": kind, "name": path, "path": path, "reason": "a prompt"})
}

/// A repo with an agent (a convention) and two prompts Claude places as commands; Claude's
/// answer also says . Imported with all three.
fn imported() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let base = tempfile::tempdir()?;
    let repo = base.path().join("repo");
    write(
        &repo,
        "agents/a.md",
        "---\nname: a\ndescription: a\n---\none\n",
    )?;
    write(&repo, "prompts/review.md", "Review the diff.\n")?;
    write(&repo, "prompts/ship.md", "Ship it.\n")?;
    write(&repo, "prompts/scratch.md", "scratch\n")?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let bin = tempfile::tempdir()?;
    let mut s = Setup {
        _base: base,
        repo,
        bin,
        library,
        _data: data,
    };
    s.claude(&json!({
        "items": [prompt("command", "prompts/review.md"), prompt("command", "prompts/ship.md")],
        "ignore": [{"path": "prompts/scratch.md", "reason": "notes"}],
    }))?;
    let preview = s.library.preview(&s.source())?;
    let preview = s.library.preview_layout(&preview, &|_| {})?;
    let all: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    assert_eq!(all.len(), 3);
    s.library.import(&preview, &all, &TagChoices::new())?;
    assert_eq!(s.calls(), 1);
    Ok(s)
}

#[test]
fn an_update_replays_a_claude_entry_and_never_calls_claude() -> TestResult {
    let mut s = imported()?;
    let review = s.id_of("prompts/review.md")?;
    let report = s.check()?;
    assert_eq!(report.sources[0].result, SourceResult::Current);
    assert!(
        report.sources[0].unplaced.is_empty(),
        "the ignored note is not unplaced: {:?}",
        report.sources[0].unplaced
    );

    write(&s.repo, "prompts/review.md", "Review the diff, twice.\n")?;
    let report = s.check()?;
    assert_eq!(
        report.sources[0].result,
        SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }
    );
    let status = s.library.update_status(&review)?.expect("outdated");
    assert_eq!(status.state, UpstreamState::Outdated);

    let job = s.library.prepare_update(&review)?;
    let prepared = job.run()?;
    s.library.finish_update(prepared)?;
    assert!(s.library.update_status(&review)?.is_none());
    let detail = s.library.detail(&review)?;
    assert_eq!(detail.artifact.metadata["layoutBy"], "claude");
    assert_eq!(detail.artifact.metadata["layoutReason"], "a prompt");
    assert_eq!(s.calls(), 1, "no CLI call for the check or the update");
    Ok(())
}

#[test]
fn a_claude_path_removed_upstream_is_layout_changed_and_cannot_be_updated() -> TestResult {
    let mut s = imported()?;
    let ship = s.id_of("prompts/ship.md")?;
    fs::remove_file(s.repo.join("prompts/ship.md"))?;
    let report = s.check()?;
    assert_eq!(
        report.sources[0].result,
        SourceResult::Changed {
            outdated: 0,
            removed: 0,
            layout_changed: 1,
            new_upstream: 0
        }
    );
    let status = s.library.update_status(&ship)?.expect("flagged");
    assert_eq!(status.state, UpstreamState::LayoutChanged);
    let refused = s.library.prepare_update(&ship).expect_err("refused");
    assert!(
        refused.to_string().contains("ask Claude again"),
        "{refused}"
    );
    let kept = s.library.update(&ship, UpdateChoice::KeepMine).err();
    assert!(kept.is_some());
    // The other prompt still replays.
    assert!(s
        .library
        .update_status(&s.id_of("prompts/review.md")?)?
        .is_none());

    // A prompt that no longer parses (a directory now) is flagged the same way.
    fs::remove_file(s.repo.join("prompts/review.md"))?;
    fs::create_dir(s.repo.join("prompts/review.md"))?;
    s.check()?;
    let status = s
        .library
        .update_status(&s.id_of("prompts/review.md")?)?
        .expect("flagged");
    assert_eq!(status.state, UpstreamState::LayoutChanged);
    assert_eq!(s.calls(), 1);
    s.library.remove(&ship)?;
    Ok(())
}

#[test]
fn rows_without_a_stored_layout_update_as_before() -> TestResult {
    let s = imported()?;
    let agent = s.id_of("agents/a.md")?;
    // As a row imported before the migration: no `layoutBy` in the stored record.
    {
        let id = &agent;
        let json: String = s.library.store.conn.query_row(
            "SELECT artifact FROM artifacts WHERE id = ?1",
            [id],
            |r| r.get(0),
        )?;
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let metadata = value["metadata"].as_object_mut().expect("metadata");
        metadata.remove("layoutBy");
        metadata.remove("layoutReason");
        s.library.store.conn.execute(
            "UPDATE artifacts SET artifact = ?2 WHERE id = ?1",
            rusqlite::params![id, value.to_string()],
        )?;
    }
    s.library.store.conn.execute(
        "UPDATE sources SET layout_version = 0, layout_ignore = NULL",
        [],
    )?;

    write(
        &s.repo,
        "agents/a.md",
        "---\nname: a\ndescription: a\n---\ntwo\n",
    )?;
    let report = s.check()?;
    assert_eq!(
        report.sources[0].result,
        SourceResult::Changed {
            outdated: 1,
            removed: 0,
            layout_changed: 0,
            new_upstream: 0
        }
    );
    assert_eq!(
        s.library.update_status(&agent)?.expect("flag").state,
        UpstreamState::Outdated
    );
    // Gone upstream, with no layout recorded, is *removed* as it always was.
    fs::remove_file(s.repo.join("agents/a.md"))?;
    s.check()?;
    assert_eq!(
        s.library.update_status(&agent)?.expect("flag").state,
        UpstreamState::Removed
    );
    Ok(())
}

#[test]
fn new_unplaced_files_are_reported_on_the_source() -> TestResult {
    let s = imported()?;
    write(&s.repo, "prompts/extra.md", "Something new.\n")?;
    let report = s.check()?;
    assert_eq!(report.sources[0].unplaced, ["prompts/extra.md"]);
    assert_eq!(report.sources[0].unplaced_overflow, 0);
    assert_eq!(s.calls(), 1);
    Ok(())
}

#[test]
fn a_reimport_replays_the_stored_layout_without_calling_claude() -> TestResult {
    let s = imported()?;
    let preview = s.library.preview(&s.source())?;
    let names: Vec<_> = preview
        .candidates
        .iter()
        .map(|c| (c.artifact.path.as_str(), c.already_in_library))
        .collect();
    assert_eq!(
        names,
        [
            ("agents/a.md", true),
            ("prompts/review.md", true),
            ("prompts/ship.md", true)
        ]
    );
    assert!(preview.unplaced.is_empty(), "{:?}", preview.unplaced);
    assert_eq!(s.calls(), 1);

    fs::remove_file(s.repo.join("prompts/ship.md"))?;
    let preview = s.library.preview(&s.source())?;
    assert!(preview
        .diagnostics
        .iter()
        .any(|d| d.code == "layout.changed" && d.file == "prompts/ship.md"));
    Ok(())
}

#[test]
fn ask_claude_again_scans_the_stored_source_and_runs_tier_three_once() -> TestResult {
    let mut s = imported()?;
    fs::remove_file(s.repo.join("prompts/ship.md"))?;
    write(&s.repo, "prompts/release.md", "Release it.\n")?;
    s.check()?;
    s.claude(&json!({"items": [prompt("command", "prompts/release.md")], "ignore": []}))?;
    let preview = s.library.preview_stored_source(&s.source_id()?)?;
    assert_eq!(preview.unplaced, ["prompts/release.md"]);
    let next = s.library.preview_layout(&preview, &|_| {})?;
    let fresh: Vec<_> = next
        .candidates
        .iter()
        .filter(|c| !c.already_in_library)
        .map(|c| c.artifact.path.as_str())
        .collect();
    assert_eq!(fresh, ["prompts/release.md"]);
    assert_eq!(s.calls(), 2, "one call for the explicit ask");
    assert!(s.library.preview_stored_source("nope").is_err());
    Ok(())
}

#[test]
fn the_ignore_list_and_version_are_stored_on_the_source() -> TestResult {
    let s = imported()?;
    let (version, ignore): (i64, String) = s.library.store.conn.query_row(
        "SELECT layout_version, layout_ignore FROM sources",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    assert_eq!(version, 1);
    assert!(ignore.contains("prompts/scratch.md"), "{ignore}");
    Ok(())
}
