use std::fs;

use super::*;
use crate::artifact::ArtifactKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn find_matches_kind_and_name_exactly() -> TestResult {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    fs::create_dir_all(src.path().join("agents"))?;
    fs::write(
        src.path().join("agents/scout.md"),
        "---\nname: scout\ndescription: Finds things\n---\nGo.\n",
    )?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<_> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;

    assert_eq!(
        library.find(ArtifactKind::Agent, "scout")?,
        Some(ids[0].clone())
    );
    assert_eq!(library.find(ArtifactKind::Skill, "scout")?, None);
    assert_eq!(library.find(ArtifactKind::Agent, "missing")?, None);
    Ok(())
}
