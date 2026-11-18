//! Linked files (ADR-007): carried with the skill that names them and placed where the link points.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::fixture::{put, TestResult};
use crate::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions};
use crate::harnmap::CategoryId;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

fn skill(root: &Path, dir: &str, name: &str, body: &str) -> std::io::Result<()> {
    put(
        root,
        &format!("{dir}/{name}/SKILL.md"),
        &format!("---\nname: {name}\ndescription: d\n---\n{body}\n"),
    )
}

/// A source shaped like agent-skills: loose skills linking the shared `references/` folder.
fn pack(root: &Path) -> std::io::Result<()> {
    put(root, "references/a.md", "checklist A\n")?;
    skill(root, "skills", "one", "See [a](../../references/a.md).")?;
    skill(root, "skills", "two", "Also `../../references/a.md`.")?;
    skill(root, "skills", "three", "No links.")
}

fn library_of(
    roots: &[&Path],
) -> std::result::Result<(tempfile::TempDir, Library), Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    for root in roots {
        let preview = library.preview(&Source::Local {
            path: root.to_path_buf(),
        })?;
        let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        library.import(&preview, &ids, &TagChoices::new())?;
    }
    Ok((data, library))
}

fn profile(
    library: &Library,
    names: &[&str],
) -> std::result::Result<i64, Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for n in names {
        items.push(
            library
                .find(ArtifactKind::Skill, n)?
                .ok_or("skill missing")?,
        );
    }
    let spec = ProfileSpec {
        items,
        settings: None,
        mcp: Vec::new(),
        manual_only: BTreeMap::new(),
    };
    Ok(library.create_profile("p", "", &spec, CategoryId::Software, None)?)
}

#[test]
fn the_scan_records_the_files_a_skill_links_to() -> TestResult {
    let src = tempfile::tempdir()?;
    pack(src.path())?;
    let report = ClaudeAdapter.scan(src.path(), &ScanOptions::default())?;
    let by_name = |n: &str| report.artifacts.iter().find(|a| a.name == n);
    assert_eq!(
        by_name("one").ok_or("one")?.linked_files,
        ["references/a.md"]
    );
    assert_eq!(
        by_name("two").ok_or("two")?.linked_files,
        ["references/a.md"]
    );
    assert!(by_name("three").ok_or("three")?.linked_files.is_empty());
    Ok(())
}

#[test]
fn a_changed_linked_file_changes_the_digest() -> TestResult {
    let src = tempfile::tempdir()?;
    pack(src.path())?;
    let digest = |root: &Path| -> std::result::Result<Option<String>, Box<dyn std::error::Error>> {
        let r = ClaudeAdapter.scan(root, &ScanOptions::default())?;
        Ok(r.artifacts
            .into_iter()
            .find(|a| a.name == "one")
            .and_then(|a| a.digest))
    };
    let before = digest(src.path())?;
    put(src.path(), "references/a.md", "checklist A, edited\n")?;
    assert_ne!(before, digest(src.path())?);
    Ok(())
}

#[test]
fn links_that_find_nothing_or_leave_the_scan_are_ignored() -> TestResult {
    let src = tempfile::tempdir()?;
    let r = src.path();
    put(r, "skills/x/notes.md", "own\n")?;
    skill(
        r,
        "skills",
        "x",
        "`../../missing.md` `../../../outside.md` `./notes.md` `../x/notes.md` https://a.b/../c.md",
    )?;
    let report = ClaudeAdapter.scan(r, &ScanOptions::default())?;
    assert!(report.artifacts[0].linked_files.is_empty());
    assert!(report.artifacts[0]
        .diagnostics
        .iter()
        .all(|d| !d.code.contains("link")));
    Ok(())
}

#[test]
fn agents_and_commands_carry_their_links_too() -> TestResult {
    let src = tempfile::tempdir()?;
    let r = src.path();
    put(r, "references/c.md", "c\n")?;
    put(
        r,
        "agents/rev.md",
        "---\nname: rev\ndescription: d\n---\nSee ../references/c.md\n",
    )?;
    put(
        r,
        "commands/go.md",
        "---\ndescription: d\n---\nSee ../references/c.md\n",
    )?;
    let report = ClaudeAdapter.scan(r, &ScanOptions::default())?;
    let linked: Vec<_> = report
        .artifacts
        .iter()
        .filter(|a| a.kind != ArtifactKind::Skill)
        .map(|a| a.linked_files.clone())
        .collect();
    assert_eq!(linked.len(), 2);
    assert!(linked.iter().all(|l| l == &["references/c.md"]));
    Ok(())
}

#[test]
fn a_link_that_would_land_outside_the_harness_is_not_carried_and_warns() -> TestResult {
    let src = tempfile::tempdir()?;
    let r = src.path();
    put(r, "docs/guide.md", "guide\n")?;
    skill(
        r,
        ".claude/skills",
        "x",
        "See ../../../docs/guide.md and ../../shared.md",
    )?;
    put(r, ".claude/shared.md", "ok\n")?;
    let report = ClaudeAdapter.scan(r, &ScanOptions::default())?;
    let a = &report.artifacts[0];
    assert_eq!(a.linked_files, [".claude/shared.md"]);
    let warning = a
        .diagnostics
        .iter()
        .find(|d| d.code == "skill.link-outside-harness")
        .ok_or("no warning")?;
    assert!(warning.message.contains("docs/guide.md"));
    Ok(())
}

#[test]
fn scaffolding_writes_a_shared_linked_file_once_under_the_first_skill() -> TestResult {
    let src = tempfile::tempdir()?;
    pack(src.path())?;
    let (_data, library) = library_of(&[src.path()])?;
    // The library copy holds it under `_linked/`.
    let id = library.find(ArtifactKind::Skill, "one")?.ok_or("one")?;
    assert_eq!(
        fs::read_to_string(
            library
                .store
                .artifacts_dir()
                .join(&id)
                .join("_linked/references/a.md")
        )?,
        "checklist A\n"
    );
    let profile = profile(&library, &["one", "two", "three"])?;
    let dir = tempfile::tempdir()?;
    let plan = library.plan_scaffold(dir.path(), profile, None)?;
    let writes: Vec<_> = plan
        .writes
        .iter()
        .filter(|w| w.path == ".claude/references/a.md")
        .collect();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].artifact.as_deref(), Some(id.as_str()));
    let spec = library.profile(profile, None)?.spec;
    assert!(library.layout(&spec)?.conflicts.is_empty());
    let report = library.apply(&plan)?;
    assert_eq!(
        fs::read_to_string(dir.path().join(".claude/references/a.md"))?,
        "checklist A\n"
    );
    let manifest = library.installed_files(report.project_id)?;
    let entry = manifest
        .iter()
        .find(|m| m.path == ".claude/references/a.md")
        .ok_or("not in the manifest")?;
    assert_eq!(entry.artifact_id.as_deref(), Some(id.as_str()));
    // Not reported as drift.
    let drift = library.check_project(report.project_id)?;
    assert!(drift.files.is_empty(), "{:?}", drift.files);
    Ok(())
}

#[test]
fn two_artifacts_with_different_bytes_at_one_path_conflict() -> TestResult {
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    put(a.path(), "references/a.md", "from A\n")?;
    skill(a.path(), "skills", "one", "../../references/a.md")?;
    put(b.path(), "references/a.md", "from B\n")?;
    skill(b.path(), "skills", "two", "../../references/a.md")?;
    let (_data, library) = library_of(&[a.path(), b.path()])?;
    let profile = profile(&library, &["one", "two"])?;
    let spec = library.profile(profile, None)?.spec;
    let layout = library.layout(&spec)?;
    assert_eq!(layout.conflicts.len(), 1);
    assert_eq!(layout.conflicts[0].path, ".claude/references/a.md");
    Ok(())
}

/// Skills that link each other's `SKILL.md` (the user's `principle-*` skills), plus a plain
/// shared file.
fn principles(root: &Path) -> std::io::Result<()> {
    put(root, "references/a.md", "checklist A\n")?;
    skill(
        root,
        "skills",
        "lever",
        "Then `../prove/SKILL.md`, with ../../references/a.md.",
    )?;
    skill(root, "skills", "premise", "Before, read ../lever/SKILL.md.")?;
    skill(root, "skills", "prove", "Last step.")
}

#[test]
fn a_link_into_another_artifact_is_a_reference_not_a_carried_file() -> TestResult {
    let src = tempfile::tempdir()?;
    principles(src.path())?;
    let report = ClaudeAdapter.scan(src.path(), &ScanOptions::default())?;
    let by_name = |n: &str| {
        report
            .artifacts
            .iter()
            .find(|a| a.name == n)
            .ok_or("missing")
    };
    let lever = by_name("lever")?;
    assert_eq!(lever.linked_files, ["references/a.md"]);
    assert!(lever
        .references
        .iter()
        .any(|r| r.kind == ArtifactKind::Skill && r.target == "prove" && r.resolved));
    let premise = by_name("premise")?;
    assert!(premise.linked_files.is_empty());
    assert!(premise.references.iter().any(|r| r.target == "lever"));
    Ok(())
}

#[test]
fn skills_linking_each_other_install_without_conflict_and_pull_what_they_need() -> TestResult {
    let src = tempfile::tempdir()?;
    principles(src.path())?;
    let (_data, library) = library_of(&[src.path()])?;
    let id = |n: &str| -> std::result::Result<String, Box<dyn std::error::Error>> {
        Ok(library.find(ArtifactKind::Skill, n)?.ok_or("missing")?)
    };
    let all = ProfileSpec {
        items: vec![id("lever")?, id("premise")?, id("prove")?],
        ..ProfileSpec::default()
    };
    assert!(library.layout(&all)?.conflicts.is_empty());
    // Picking `premise` alone pulls `lever`, which pulls `prove`: whole skills, no stray copies.
    let alone = ProfileSpec {
        items: vec![id("premise")?],
        ..ProfileSpec::default()
    };
    let names: Vec<String> = library
        .resolve(&alone)?
        .artifacts
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert_eq!(names, ["premise", "lever", "prove"]);
    let layout = library.layout(&alone)?;
    assert!(layout.conflicts.is_empty());
    let paths: Vec<&str> = layout.paths.iter().map(|p| p.path.as_str()).collect();
    assert!(paths.contains(&".claude/skills/prove/SKILL.md"));
    assert!(paths.contains(&".claude/references/a.md"));
    Ok(())
}
