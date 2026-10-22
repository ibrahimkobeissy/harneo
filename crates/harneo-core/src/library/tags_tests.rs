use std::fs;

use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Setup {
    _data: tempfile::TempDir,
    _source_dir: tempfile::TempDir,
    library: Library,
    source: Source,
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let root = source_dir.path();
    fs::create_dir_all(root.join("agents"))?;
    fs::write(
        root.join("agents/scout.md"),
        "---\nname: scout\ndescription: Finds things\nkeywords: [Search, search, \" recon \", \"\"]\n---\nGo.\n",
    )?;
    fs::write(
        root.join("agents/plain.md"),
        "---\nname: plain\ndescription: Nothing\ntags: \"a, B, a\"\n---\nGo.\n",
    )?;
    fs::write(
        root.join("agents/bare.md"),
        "---\nname: bare\ndescription: None\n---\nGo.\n",
    )?;
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: root.to_path_buf(),
    };
    Ok(Setup {
        _data: data,
        _source_dir: source_dir,
        library,
        source,
    })
}

fn candidate<'a>(preview: &'a Preview, name: &str) -> &'a Candidate {
    preview
        .candidates
        .iter()
        .find(|c| c.artifact.name == name)
        .expect("candidate")
}

fn names(tags: &[Tag]) -> Vec<&str> {
    tags.iter().map(|t| t.name.as_str()).collect()
}

#[test]
fn create_rename_recolor_delete() -> TestResult {
    let s = setup()?;
    let tag = s.library.create_tag("  agents ", TagColor::Violet)?;
    assert_eq!((tag.name.as_str(), tag.color), ("agents", TagColor::Violet));
    let renamed = s.library.rename_tag(tag.id, "bots")?;
    assert_eq!(renamed.name, "bots");
    let recolored = s.library.recolor_tag(tag.id, TagColor::Stone)?;
    assert_eq!(recolored.color, TagColor::Stone);
    let listed = s.library.tags()?;
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].name.as_str(), listed[0].count), ("bots", 0));
    s.library.delete_tag(tag.id)?;
    assert!(s.library.tags()?.is_empty());
    assert!(matches!(
        s.library.delete_tag(tag.id),
        Err(Error::TagNotFound { .. })
    ));
    assert!(matches!(
        s.library.rename_tag(99, "x"),
        Err(Error::TagNotFound { .. })
    ));
    Ok(())
}

#[test]
fn names_are_validated_and_unique_ignoring_case() -> TestResult {
    let s = setup()?;
    let first = s.library.create_tag("Docs", TagColor::Cyan)?;
    assert!(matches!(
        s.library.create_tag("docs", TagColor::Sky),
        Err(Error::TagExists { .. })
    ));
    assert!(matches!(
        s.library.create_tag("   ", TagColor::Sky),
        Err(Error::InvalidTag { .. })
    ));
    assert!(matches!(
        s.library.create_tag(&"x".repeat(41), TagColor::Sky),
        Err(Error::InvalidTag { .. })
    ));
    s.library.create_tag(&"x".repeat(40), TagColor::Sky)?;
    let other = s.library.create_tag("other", TagColor::Sky)?;
    assert!(matches!(
        s.library.rename_tag(other.id, " DOCS "),
        Err(Error::TagExists { .. })
    ));
    // Renaming a tag to its own name in another case is allowed.
    assert_eq!(s.library.rename_tag(first.id, "DOCS")?.name, "DOCS");
    Ok(())
}

#[test]
fn colors_are_the_palette_and_an_invalid_one_is_rejected() {
    for name in [
        "cyan", "sky", "indigo", "violet", "fuchsia", "stone", "slate",
    ] {
        let color: TagColor = serde_json::from_str(&format!("\"{name}\"")).expect("palette color");
        assert_eq!(
            serde_json::to_string(&color).expect("json"),
            format!("\"{name}\"")
        );
    }
    for bad in ["green", "red", "amber", "blue", "plain", "Cyan"] {
        assert!(
            serde_json::from_str::<TagColor>(&format!("\"{bad}\"")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn tagging_is_idempotent_and_deleting_a_tag_untags_artifacts() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let ids: Vec<_> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    let (a, b) = (&ids[0], &ids[1]);
    let tag = s.library.create_tag("keep", TagColor::Indigo)?;
    s.library.tag_artifact(a, tag.id)?;
    s.library.tag_artifact(a, tag.id)?;
    s.library.tag_artifact(b, tag.id)?;
    assert_eq!(s.library.artifact_tags(a)?.len(), 1);
    assert_eq!(s.library.tags()?[0].count, 2);
    s.library.untag_artifact(a, tag.id)?;
    s.library.untag_artifact(a, tag.id)?;
    assert!(s.library.artifact_tags(a)?.is_empty());
    assert!(matches!(
        s.library.tag_artifact("0000000000000000", tag.id),
        Err(Error::NotInLibrary { .. })
    ));
    assert!(matches!(
        s.library.tag_artifact(a, 999),
        Err(Error::TagNotFound { .. })
    ));
    s.library.delete_tag(tag.id)?;
    assert!(s.library.artifact_tags(b)?.is_empty());
    Ok(())
}

#[test]
fn frontmatter_keywords_and_tags_become_suggestions() -> TestResult {
    let s = setup()?;
    let preview = s.library.preview(&s.source)?;
    assert_eq!(
        candidate(&preview, "scout").suggested_tags,
        ["Search", "recon"]
    );
    assert_eq!(candidate(&preview, "plain").suggested_tags, ["a", "B"]);
    assert!(candidate(&preview, "bare").suggested_tags.is_empty());
    Ok(())
}

#[test]
fn suggestions_are_capped_at_ten() -> TestResult {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    fs::create_dir_all(src.path().join("agents"))?;
    let many: Vec<_> = (0..15).map(|n| format!("k{n}")).collect();
    fs::write(
        src.path().join("agents/a.md"),
        format!(
            "---\nname: a\ndescription: d\nkeywords: [{}]\n---\nGo.\n",
            many.join(", ")
        ),
    )?;
    let library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    assert_eq!(preview.candidates[0].suggested_tags.len(), 10);
    Ok(())
}

#[test]
fn import_applies_chosen_tags_and_creates_missing_ones() -> TestResult {
    let mut s = setup()?;
    let existing = s.library.create_tag("search", TagColor::Fuchsia)?;
    let preview = s.library.preview(&s.source)?;
    let scout = candidate(&preview, "scout").id.clone();
    let bare = candidate(&preview, "bare").id.clone();
    let mut choices = TagChoices::new();
    choices.insert(scout.clone(), vec!["Search".into(), "recon".into()]);
    s.library
        .import(&preview, &[scout.clone(), bare.clone()], &choices)?;
    let tags = s.library.artifact_tags(&scout)?;
    assert_eq!(names(&tags), ["recon", "search"]);
    // The existing tag was reused, with its color, not duplicated.
    assert_eq!(tags[1].id, existing.id);
    assert_eq!(tags[1].color, TagColor::Fuchsia);
    assert!(s.library.artifact_tags(&bare)?.is_empty());
    // The new tag's color is deterministic for its name.
    let again = tags::TagColor::default_for("recon");
    assert_eq!(tags[0].color, again);
    Ok(())
}

#[test]
fn a_bad_tag_name_fails_the_import_and_leaves_nothing() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let id = preview.candidates[0].id.clone();
    let mut choices = TagChoices::new();
    choices.insert(id.clone(), vec!["ok".into(), " ".into()]);
    assert!(matches!(
        s.library
            .import(&preview, std::slice::from_ref(&id), &choices),
        Err(Error::InvalidTag { .. })
    ));
    assert!(s.library.tags()?.is_empty());
    assert!(s.library.detail(&id).is_err());
    Ok(())
}
