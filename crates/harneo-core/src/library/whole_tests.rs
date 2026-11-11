//! Parts of plugins that must now stay whole (ADR-006): the update check marks them and the
//! library replaces them with the whole plugin.

use std::fs;
use std::path::Path;

use super::unpack_tests::{plugin_repo, unpacking};
use super::*;
use crate::artifact::ArtifactKind;
use crate::harnmap::CategoryId;
use crate::profiles::ProfileSpec;
use crate::updates::CheckScope;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Setup {
    _data: tempfile::TempDir,
    repo: tempfile::TempDir,
    library: Library,
    parts: Vec<String>,
}

/// The skills of a plugin at the repo root imported unpacked (its MCP servers left out).
fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let repo = tempfile::tempdir()?;
    plugin_repo(repo.path())?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: repo.path().to_path_buf(),
    };
    let preview = library.preview_with(&source, &unpacking(&[""]))?;
    let parts: Vec<String> = preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind == ArtifactKind::Skill)
        .map(|c| c.id.clone())
        .collect();
    library.import(&preview, &parts, &TagChoices::new())?;
    Ok(Setup {
        _data: data,
        repo,
        library,
        parts,
    })
}

fn add_hooks(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("hooks"))?;
    fs::write(root.join("hooks/hooks.json"), r#"{"hooks":{}}"#)
}

impl Setup {
    fn notices(&self) -> std::result::Result<Vec<Option<String>>, Box<dyn std::error::Error>> {
        let page = self.library.list(&Query::default())?;
        Ok(page.items.into_iter().map(|i| i.must_stay_whole).collect())
    }
}

#[test]
fn parts_of_a_plugin_that_now_has_hooks_carry_the_reason() -> TestResult {
    let setup = setup()?;
    assert!(setup.notices()?.iter().all(Option::is_none));
    add_hooks(setup.repo.path())?;
    setup.library.check_updates(&CheckScope::All)?;
    let notices = setup.notices()?;
    assert_eq!(notices.len(), 3);
    for notice in &notices {
        let text = notice.as_deref().ok_or("no notice")?;
        assert!(text.contains("hooks"), "{text}");
    }
    let detail = setup.library.detail(&setup.parts[0])?;
    assert!(detail.must_stay_whole.is_some_and(|t| t.contains("hooks")));
    Ok(())
}

#[test]
fn parts_of_a_skills_only_plugin_show_no_notice() -> TestResult {
    let setup = setup()?;
    setup.library.check_updates(&CheckScope::All)?;
    assert!(setup.notices()?.iter().all(Option::is_none));
    let detail = setup.library.detail(&setup.parts[0])?;
    assert!(detail.must_stay_whole.is_none());
    Ok(())
}

#[test]
fn a_source_checked_before_the_rule_is_judged_again_at_the_same_commit() -> TestResult {
    let setup = setup()?;
    // The first check records a verdict; clearing it stands for a library from before ADR-006.
    add_hooks(setup.repo.path())?;
    setup
        .library
        .store
        .conn
        .execute("UPDATE sources SET whole_reasons = NULL", [])?;
    setup.library.check_updates(&CheckScope::All)?;
    assert!(setup.notices()?.iter().all(Option::is_some));
    Ok(())
}

fn profile_items(
    setup: &Setup,
    id: i64,
) -> std::result::Result<(u32, Vec<String>), Box<dyn std::error::Error>> {
    let view = setup.library.profile(id, None)?;
    Ok((view.current_version, view.spec.items))
}

#[test]
fn replace_leaves_one_whole_plugin_and_profiles_hold_it_in_a_new_version() -> TestResult {
    let mut setup = setup()?;
    let spec = ProfileSpec {
        items: vec![setup.parts[0].clone(), setup.parts[1].clone()],
        ..ProfileSpec::default()
    };
    let profile = setup
        .library
        .create_profile("kit", "", &spec, CategoryId::Software, None)?;
    add_hooks(setup.repo.path())?;
    setup.library.check_updates(&CheckScope::All)?;

    let plan = setup.library.replace_with_whole_plan(&setup.parts[0])?;
    assert_eq!(plan.plugin, "toolbox");
    assert_eq!(plan.parts.len(), 3);
    assert_eq!(plan.profiles.len(), 1);
    assert_eq!(setup.library.list(&Query::default())?.total, 3);

    let done = setup.library.replace_with_whole(&setup.parts[0])?;
    let page = setup.library.list(&Query::default())?;
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].kind, ArtifactKind::Plugin);
    assert_eq!(page.items[0].id, done.plugin_id);
    assert!(page.items[0].must_stay_whole.is_none());
    assert_eq!(
        profile_items(&setup, profile)?,
        (2, vec![done.plugin_id.clone()])
    );
    assert_eq!(done.profiles.len(), 1);
    for part in &setup.parts {
        assert!(!setup.library.store.artifacts_dir().join(part).exists());
    }
    assert!(setup
        .library
        .store
        .artifacts_dir()
        .join(&done.plugin_id)
        .is_dir());
    // The source now replays the plugin whole, so a check finds nothing unpacked to judge.
    setup.library.check_updates(&CheckScope::All)?;
    assert_eq!(setup.library.list(&Query::default())?.total, 1);
    Ok(())
}

#[test]
fn replace_is_refused_for_parts_whose_plugin_need_not_stay_whole() -> TestResult {
    let mut setup = setup()?;
    setup.library.check_updates(&CheckScope::All)?;
    let refused = setup.library.replace_with_whole(&setup.parts[0]).err();
    assert!(refused.is_some_and(|e| e.to_string().contains("staying whole")));
    assert_eq!(setup.library.list(&Query::default())?.total, 3);
    Ok(())
}
