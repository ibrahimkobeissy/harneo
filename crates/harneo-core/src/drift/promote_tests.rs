use std::fs;

use super::fixture::{fx, project, put, read, TestResult, AGENT};
use super::DriftStatus;

const EDITED: &str = "---\nname: rev\ndescription: Reviews code\n---\nOne\nBetter\nThree\n";

fn library_copy(f: &super::fixture::Fx, id: &str, rel: &str) -> std::io::Result<String> {
    fs::read_to_string(f.data.path().join("library/artifacts").join(id).join(rel))
}

#[test]
fn promote_makes_the_project_file_the_library_copy() -> TestResult {
    let f = fx()?;
    let (a, a_id) = project(&f)?;
    let (b, b_id) = project(&f)?;
    let before = read(b.path(), AGENT)?;
    put(a.path(), AGENT, EDITED)?;

    let report = f.library.promote(a_id, AGENT)?;
    assert_eq!(report.path, AGENT);
    assert_eq!(library_copy(&f, &report.artifact, "rev.md")?, EDITED);
    let old = fs::read_to_string(std::path::Path::new(&report.backup).join("rev.md"))?;
    assert!(old.contains("Two"), "the backup holds the previous copy");

    // This project is in sync and the library copy is not a local edit.
    assert_eq!(f.library.check_project(a_id)?.status, DriftStatus::InSync);
    let digest: String = f.library.store.conn.query_row(
        "SELECT copy_digest FROM artifacts WHERE id = ?1",
        [&report.artifact],
        |r| r.get(0),
    )?;
    let folder = f
        .data
        .path()
        .join("library/artifacts")
        .join(&report.artifact);
    assert_eq!(digest, crate::updates::copy_digest(&folder)?);
    let audited: Option<i64> = f.library.store.conn.query_row(
        "SELECT audited_at FROM artifacts WHERE id = ?1",
        [&report.artifact],
        |r| r.get(0),
    )?;
    assert!(audited.is_some());

    // The other project now sees the library ahead of it, and re-applying catches it up.
    let other = f.library.check_project(b_id)?;
    assert_eq!(other.status, DriftStatus::InSync);
    assert_eq!(other.library_newer, [AGENT]);
    let plan = f.library.plan_reapply(b_id, None)?;
    f.library.apply(&plan)?;
    assert_eq!(read(b.path(), AGENT)?, EDITED);
    assert_ne!(read(b.path(), AGENT)?, before);
    let after = f.library.check_project(b_id)?;
    assert!(after.library_newer.is_empty() && after.status == DriftStatus::InSync);
    Ok(())
}

#[test]
fn generated_deleted_unchanged_and_unknown_files_are_refused() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    put(dir.path(), "HARNESS.md", "mine\n")?;
    let generated = f
        .library
        .promote(id, "HARNESS.md")
        .err()
        .map(|e| e.to_string());
    assert!(generated.is_some_and(|m| m.contains("generated")));
    fs::remove_file(dir.path().join(AGENT))?;
    let deleted = f.library.promote(id, AGENT).err().map(|e| e.to_string());
    assert!(deleted.is_some_and(|m| m.contains("deleted")));
    let skill = ".claude/skills/s/SKILL.md";
    let same = f.library.promote(id, skill).err().map(|e| e.to_string());
    assert!(same.is_some_and(|m| m.contains("no changes")));
    assert!(f.library.promote(id, "README.md").is_err());
    Ok(())
}

#[test]
fn promoting_claude_md_leaves_the_harness_import_out_of_the_library() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    assert!(read(dir.path(), "CLAUDE.md")?.contains("@HARNESS.md"));
    put(dir.path(), "CLAUDE.md", "# Project\nmore\n\n@HARNESS.md\n")?;
    let report = f.library.promote(id, "CLAUDE.md")?;
    assert_eq!(
        library_copy(&f, &report.artifact, "CLAUDE.md")?,
        "# Project\nmore\n"
    );
    // Rendering adds the import again, so the project stays in sync.
    let check = f.library.check_project(id)?;
    assert_eq!(check.status, DriftStatus::InSync);
    assert!(check.library_newer.is_empty(), "{:?}", check.library_newer);
    Ok(())
}

#[test]
fn an_added_artifact_comes_back_as_a_preview_with_its_candidate() -> TestResult {
    let f = fx()?;
    let (dir, id) = project(&f)?;
    put(
        dir.path(),
        ".claude/commands/extra.md",
        "---\ndescription: Extra\n---\nGo\n",
    )?;
    let added = f.library.promote_added(id, ".claude/commands/extra.md")?;
    let chosen = added.candidate.ok_or("no candidate")?;
    let candidate = added
        .preview
        .candidates
        .iter()
        .find(|c| c.id == chosen)
        .ok_or("not in the preview")?;
    assert_eq!(candidate.artifact.name, "extra");
    assert!(f.library.promote_added(id, "nope.md").is_err());
    assert!(f.library.promote_added(id, "../x").is_err());
    Ok(())
}
