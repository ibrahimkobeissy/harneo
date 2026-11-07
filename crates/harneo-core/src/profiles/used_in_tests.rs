use super::tests::{fixture, write, TestResult};
use super::*;
use crate::harnmap::CategoryId;
use crate::library::{Source, TagChoices};

fn spec_of_items(ids: &[&str]) -> ProfileSpec {
    ProfileSpec {
        items: ids.iter().map(|s| (*s).to_owned()).collect(),
        ..ProfileSpec::default()
    }
}

#[test]
fn profiles_using_counts_picked_bundled_settings_and_mcp() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    let settings = f.id(ArtifactKind::Settings, "settings")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let first =
        f.library
            .create_profile("Zed", "", &spec_of_items(&[&a]), CategoryId::Software, None)?;
    f.library.create_profile(
        "Alpha",
        "",
        &ProfileSpec {
            items: vec![b.clone()],
            settings: Some(settings.clone()),
            mcp: vec![McpChoice {
                artifact: api.clone(),
                env: BTreeMap::new(),
            }],
            ..ProfileSpec::default()
        },
        CategoryId::Software,
        None,
    )?;

    let names = |id: &str| -> std::result::Result<Vec<(String, u32)>, crate::Error> {
        Ok(f.library
            .profiles_using(id)?
            .into_iter()
            .map(|p| (p.name, p.version))
            .collect())
    };
    assert_eq!(names(&a)?, vec![("Zed".to_owned(), 1)]);
    // a references helper, so Zed installs it through the bundle.
    assert_eq!(names(&helper)?, vec![("Zed".to_owned(), 1)]);
    assert_eq!(names(&b)?, vec![("Alpha".to_owned(), 1)]);
    assert_eq!(names(&settings)?, vec![("Alpha".to_owned(), 1)]);
    assert_eq!(names(&api)?, vec![("Alpha".to_owned(), 1)]);
    assert!(names("nope")?.is_empty());

    // Only the current version counts, and the version number follows it.
    f.library.save_profile(first, &spec_of_items(&[&b]))?;
    assert!(names(&a)?.is_empty());
    assert_eq!(
        names(&b)?,
        vec![("Alpha".to_owned(), 1), ("Zed".to_owned(), 2)]
    );
    Ok(())
}

#[test]
fn the_invocation_filter_and_flags_follow_the_library_default_and_the_frontmatter() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let list = |manual: Option<bool>| -> std::result::Result<_, crate::Error> {
        f.library.list(&Query {
            filter: Filter {
                manual_only: manual,
                ..Filter::default()
            },
            ..Query::default()
        })
    };
    let all = list(None)?;
    assert_eq!((all.facets.manual_only, all.facets.auto), (0, all.total));
    assert!(all.items.iter().all(|i| !i.manual_only));

    f.library.set_manual_only(&a, true)?;
    let manual = list(Some(true))?;
    assert_eq!(manual.total, 1);
    assert_eq!(manual.items[0].id, a);
    assert!(manual.items[0].manual_only);
    assert!(f.library.detail(&a)?.manual_only);
    assert!(!f.library.detail(&b)?.manual_only);
    let auto = list(Some(false))?;
    assert_eq!(auto.total, all.total - 1);
    // Each side counts over the rest of the filter, not its own.
    assert_eq!(auto.facets.manual_only, 1);
    assert_eq!(auto.facets.auto, all.total - 1);

    // A skill whose own frontmatter turns invocation off is manual only too.
    let src = tempfile::tempdir()?;
    write(
        src.path(),
        "skills/quiet/SKILL.md",
        "---\nname: quiet\ndescription: q\ndisable-model-invocation: true\n---\nBody.\n",
    )?;
    let mut library = f.library;
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let quiet = library
        .find(ArtifactKind::Skill, "quiet")?
        .ok_or("quiet missing")?;
    assert!(library.detail(&quiet)?.manual_only);
    Ok(())
}
