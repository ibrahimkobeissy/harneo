//! Update checks replay the plugins unpacked at import.

use std::fs;

use super::*;
use crate::library::unpack_tests::{plugin_repo, unpacking};
use crate::library::{Library, Source, TagChoices};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_reopened_library_checks_an_unpacked_plugin_with_the_same_choice() -> TestResult {
    let repo = tempfile::tempdir()?;
    plugin_repo(repo.path())?;
    let data = tempfile::tempdir()?;
    let source = Source::Local {
        path: repo.path().to_path_buf(),
    };
    let alpha = {
        let mut library = Library::open(data.path())?;
        let preview = library.preview_with(&source, &unpacking(&[""]))?;
        let ids: Vec<String> = preview
            .candidates
            .iter()
            .filter(|c| c.artifact.name == "alpha" || c.artifact.name == "beta")
            .map(|c| c.id.clone())
            .collect();
        library.import(&preview, &ids, &TagChoices::new())?;
        preview
            .candidates
            .iter()
            .find(|c| c.artifact.name == "alpha")
            .map(|c| c.id.clone())
            .ok_or("alpha")?
    };

    let mut library = Library::open(data.path())?;
    let report = library.check_updates(&CheckScope::All)?;
    assert!(
        matches!(
            report.sources[0].result,
            SourceResult::Changed {
                outdated: 0,
                removed: 0,
                ..
            } | SourceResult::Current
        ),
        "{:?}",
        report.sources[0].result
    );

    fs::write(
        repo.path().join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: The alpha skill\n---\nChanged\n",
    )?;
    let report = library.check_updates(&CheckScope::All)?;
    assert!(
        matches!(
            report.sources[0].result,
            SourceResult::Changed {
                outdated: 1,
                removed: 0,
                ..
            }
        ),
        "{:?}",
        report.sources[0].result
    );
    let status = library.update_status(&alpha)?.ok_or("outdated")?;
    assert_eq!(status.state, UpstreamState::Outdated);

    let prepared = library.prepare_update(&alpha)?.run()?;
    library.finish_update(prepared)?;
    assert!(library.update_status(&alpha)?.is_none());
    let stored = fs::read_to_string(library.store.artifacts_dir().join(&alpha).join("SKILL.md"))?;
    assert!(stored.contains("Changed"));
    Ok(())
}
