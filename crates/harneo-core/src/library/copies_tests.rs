use std::fs;
use std::path::Path;

use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn skill(root: &Path, dir: &str, body: &str) -> std::io::Result<()> {
    let folder = root.join(dir);
    fs::create_dir_all(&folder)?;
    fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: notes\ndescription: Takes notes\n---\n{body}\n"),
    )
}

fn preview_of(root: &Path) -> std::result::Result<Preview, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    Ok(Library::open(data.path())?.preview(&Source::Local {
        path: root.to_path_buf(),
    })?)
}

#[test]
fn three_identical_copies_are_one_candidate_and_import_stores_one() -> TestResult {
    let src = tempfile::tempdir()?;
    for d in [
        "packs/c/notes",
        "packs/a/notes",
        "packs/b/notes",
    ] {
        skill(src.path(), d, "Write.")?;
    }
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    assert_eq!(preview.candidates.len(), 1);
    let c = &preview.candidates[0];
    assert_eq!(c.artifact.path, "packs/a/notes");
    assert_eq!(c.copies, ["packs/b/notes", "packs/c/notes"]);
    assert!(c
        .artifact
        .diagnostics
        .iter()
        .all(|d| d.code != "scan.duplicate-name"));
    let report = library.import(
        &preview,
        std::slice::from_ref(&c.id),
        &TagChoices::default(),
    )?;
    assert_eq!(report.imported.len(), 1);
    Ok(())
}

#[test]
fn copies_that_differ_stay_separate_and_keep_the_warning() -> TestResult {
    let src = tempfile::tempdir()?;
    skill(src.path(), "skills/a/notes", "One.")?;
    skill(src.path(), "skills/b/notes", "Two.")?;
    skill(src.path(), "skills/c/notes", "Two.")?;
    let preview = preview_of(src.path())?;
    assert_eq!(preview.candidates.len(), 2);
    for c in &preview.candidates {
        assert!(c
            .artifact
            .diagnostics
            .iter()
            .any(|d| d.code == "scan.duplicate-name"));
    }
    let two = preview
        .candidates
        .iter()
        .find(|c| !c.copies.is_empty())
        .ok_or("collapsed pair")?;
    assert_eq!(two.artifact.path, "skills/b/notes");
    assert_eq!(two.copies, ["skills/c/notes"]);
    Ok(())
}
