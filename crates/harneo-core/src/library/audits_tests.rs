use std::fs;

use super::*;
use crate::artifact::ArtifactKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn audits_lists_every_artifact_with_its_stored_audit() -> TestResult {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    fs::create_dir_all(src.path().join("agents"))?;
    fs::write(
        src.path().join("agents/scout.md"),
        "---\nname: scout\ndescription: Finds things\n---\nLook.\n",
    )?;
    fs::write(
        src.path().join("agents/zed.md"),
        "---\nname: zed\ndescription: Last\n---\nGo.\n",
    )?;
    let mut library = Library::open(data.path())?;
    assert!(library.audits()?.is_empty());
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;

    let all = library.audits()?;
    let names: Vec<&str> = all.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["scout", "zed"]);
    let scout = &all[0];
    assert_eq!(scout.kind, ArtifactKind::Agent);
    assert_eq!(scout.audit.as_ref().map(|a| a.score), Some(100));

    // A row that was never audited has no audit.
    library.store.conn.execute(
        "UPDATE artifacts SET trust_score = NULL, tier = NULL, findings = NULL, reach = NULL,
                audited_at = NULL WHERE name = 'zed'",
        [],
    )?;
    let all = library.audits()?;
    assert!(all[0].audit.is_some());
    assert!(all[1].audit.is_none());
    Ok(())
}
