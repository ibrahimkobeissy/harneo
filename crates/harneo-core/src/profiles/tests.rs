use std::fs;
use std::path::Path;

use super::*;
use crate::library::{Source, TagChoices};

pub(super) type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

pub(super) fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

pub(super) struct Fixture {
    pub _data: tempfile::TempDir,
    pub _src: tempfile::TempDir,
    pub library: Library,
}

impl Fixture {
    pub fn id(&self, kind: ArtifactKind, name: &str) -> std::result::Result<String, String> {
        self.library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} not in library"))
    }
}

/// Skills a and b, agent helper (a references it, so a pulls helper), a Settings template and an
/// MCP server `api`.
pub(super) fn fixture() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    for name in ["a", "b"] {
        write(
            src.path(),
            &format!("skills/{name}/SKILL.md"),
            &format!("---\nname: {name}\ndescription: {name} skill\n---\nBody.\n"),
        )?;
    }
    write(
        src.path(),
        "agents/helper.md",
        "---\nname: helper\ndescription: helper agent\n---\nBody.\n",
    )?;
    write(
        src.path(),
        "settings.json",
        "{\"permissions\":{\"allow\":[\"Read\"]}}\n",
    )?;
    write(
        src.path(),
        ".mcp.json",
        "{\"mcpServers\":{\"api\":{\"command\":\"npx\",\"args\":[\"x\"]}}}\n",
    )?;
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let fixture = Fixture {
        _data: data,
        _src: src,
        library,
    };
    let a = fixture.id(ArtifactKind::Skill, "a")?;
    let helper = fixture.id(ArtifactKind::Agent, "helper")?;
    fixture
        .library
        .add_link(&a, &helper, crate::bundles::LinkOrigin::Manual, "")?;
    Ok(fixture)
}

pub(super) fn spec(items: &[&str]) -> ProfileSpec {
    ProfileSpec {
        items: items.iter().map(|s| (*s).to_owned()).collect(),
        ..ProfileSpec::default()
    }
}

#[test]
fn versions_bump_only_on_change_and_old_ones_stay_readable() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let id = f.library.create_profile("Light", " first ", &spec(&[&a]))?;
    assert_eq!(f.library.profile(id, None)?.version, 1);
    assert_eq!(f.library.save_profile(id, &spec(&[&a]))?, 1);
    assert_eq!(f.library.save_profile(id, &spec(&[&a, &b]))?, 2);
    assert_eq!(f.library.save_profile(id, &spec(&[&a, &b]))?, 2);
    f.library.rename_profile(id, "Lighter")?;
    f.library.set_profile_description(id, "d")?;
    let view = f.library.profile(id, None)?;
    assert_eq!((view.current_version, view.versions.len()), (2, 2));
    assert_eq!(view.name, "Lighter");
    assert_eq!(f.library.profile(id, Some(1))?.spec, spec(&[&a]));
    assert_eq!(f.library.profile(id, Some(1))?.current_version, 2);
    assert!(f.library.profile(id, Some(9)).is_err());
    let list = f.library.profiles()?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].version, 2);
    assert_eq!(list[0].project_count, 0);
    Ok(())
}

#[test]
fn names_are_unique_ignoring_case_and_checked() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let one = f.library.create_profile("Light", "", &spec(&[&a]))?;
    assert!(matches!(
        f.library.create_profile("light", "", &spec(&[&a])),
        Err(Error::ProfileExists { .. })
    ));
    assert!(matches!(
        f.library.create_profile("  ", "", &spec(&[&a])),
        Err(Error::InvalidProfile { .. })
    ));
    let two = f.library.create_profile("Heavy", "", &spec(&[&a]))?;
    assert!(matches!(
        f.library.rename_profile(two, "LIGHT"),
        Err(Error::ProfileExists { .. })
    ));
    assert!(matches!(
        f.library.rename_profile(999, "x"),
        Err(Error::ProfileNotFound { .. })
    ));
    f.library.delete_profile(one)?;
    assert!(matches!(
        f.library.profile(one, None),
        Err(Error::ProfileNotFound { .. })
    ));
    assert!(f.library.delete_profile(one).is_err());
    Ok(())
}

#[test]
fn validation_refuses_wrong_kinds_unknown_ids_and_empty_specs() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let settings = f.id(ArtifactKind::Settings, "settings")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let create = |s: &ProfileSpec| f.library.create_profile("P", "", s);
    assert!(create(&ProfileSpec::default()).is_err());
    assert!(create(&spec(&["ffffffffffffffff"])).is_err());
    assert!(create(&spec(&[&settings])).is_err());
    assert!(create(&spec(&[&api])).is_err());
    assert!(create(&spec(&[&a, &a])).is_err());
    let mut bad = spec(&[&a]);
    bad.settings = Some(a.clone());
    assert!(create(&bad).is_err());
    let mut bad = spec(&[&a]);
    bad.mcp = vec![McpChoice {
        artifact: a.clone(),
        env: BTreeMap::new(),
    }];
    assert!(create(&bad).is_err());
    let mut good = spec(&[&a]);
    good.settings = Some(settings);
    good.mcp = vec![McpChoice {
        artifact: api,
        env: BTreeMap::from([("TOKEN".to_owned(), "${API_TOKEN}".to_owned())]),
    }];
    let id = create(&good)?;
    assert_eq!(f.library.profile(id, None)?.spec, good);
    Ok(())
}

#[test]
fn a_literal_mcp_env_value_is_refused_and_never_stored() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let literal = ["sk-", &"a".repeat(30)].concat();
    let mut s = spec(&[&a]);
    s.mcp = vec![McpChoice {
        artifact: api,
        env: BTreeMap::from([("TOKEN".to_owned(), literal.clone())]),
    }];
    let err = f.library.create_profile("P", "", &s).err();
    assert!(matches!(err, Some(Error::InvalidProfile { .. })));
    assert!(!err
        .map(|e| e.to_string())
        .unwrap_or_default()
        .contains(&literal));
    assert!(f.library.profiles()?.is_empty());
    let id = f.library.create_profile("Q", "", &spec(&[&a]))?;
    assert!(f.library.save_profile(id, &s).is_err());
    assert_eq!(f.library.profile(id, None)?.version, 1);
    for ok in ["${A}", "${A_1}", "${9}"] {
        assert!(is_placeholder(ok), "{ok}");
    }
    for bad in [
        "", "${}", "${a}", "$A", "x${A}", "${A}x", "${A B}", "${A}${B}",
    ] {
        assert!(!is_placeholder(bad), "{bad}");
    }
    Ok(())
}

#[test]
fn clone_copies_the_current_version_and_says_where_from() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let id = f.library.create_profile("Light", "", &spec(&[&a]))?;
    f.library.save_profile(id, &spec(&[&a, &b]))?;
    let copy = f.library.clone_profile(id, "Heavy")?;
    let view = f.library.profile(copy, None)?;
    assert_eq!(view.description, "Cloned from Light v2");
    assert_eq!((view.version, view.spec), (1, spec(&[&a, &b])));
    assert!(matches!(
        f.library.clone_profile(id, "light"),
        Err(Error::ProfileExists { .. })
    ));
    Ok(())
}

#[test]
fn from_collection_takes_the_items_and_leaves_settings_and_mcp_out() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let settings = f.id(ArtifactKind::Settings, "settings")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let manual = f.library.create_manual_collection("Mine")?;
    for id in [&a, &settings, &api] {
        f.library.add_to_collection(manual.id, id)?;
    }
    let id = f.library.profile_from_collection(manual.id, "From mine")?;
    let view = f.library.profile(id, None)?;
    assert_eq!(view.spec, spec(&[&a]));
    assert!(view.description.contains("Mine"));

    let smart = f.library.create_smart_collection(
        "Skills",
        Filter {
            types: vec![ArtifactKind::Skill],
            ..Filter::default()
        },
    )?;
    let id = f.library.profile_from_collection(smart.id, "From skills")?;
    assert_eq!(f.library.profile(id, None)?.spec.items.len(), 2);

    let empty = f.library.create_manual_collection("Empty")?;
    assert!(f.library.profile_from_collection(empty.id, "Nope").is_err());
    assert!(f.library.profile_from_collection(9999, "Nope").is_err());
    Ok(())
}

#[test]
fn manual_only_is_a_library_default_with_a_profile_override() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    assert!(f.library.set_manual_only(&helper, true).is_err());
    assert!(f.library.set_manual_only("ffffffffffffffff", true).is_err());
    let plain = spec(&[&a]);
    let manual = |s: &ProfileSpec| -> std::result::Result<bool, Error> {
        Ok(f.library.resolve(s)?.artifacts[0].manual_only)
    };
    assert!(!manual(&plain)?);
    f.library.set_manual_only(&a, true)?;
    assert!(manual(&plain)?);
    let mut off = plain.clone();
    off.manual_only.insert(a.clone(), false);
    assert!(!manual(&off)?);
    f.library.set_manual_only(&a, false)?;
    let mut on = plain;
    on.manual_only.insert(a, true);
    assert!(manual(&on)?);
    Ok(())
}

#[test]
fn removing_an_artifact_gives_every_profile_holding_it_a_new_version() -> TestResult {
    let mut f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let settings = f.id(ArtifactKind::Settings, "settings")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let mut full = spec(&[&a, &b]);
    full.settings = Some(settings.clone());
    full.mcp = vec![McpChoice {
        artifact: api.clone(),
        env: BTreeMap::new(),
    }];
    full.manual_only.insert(a.clone(), true);
    let with_a = f.library.create_profile("With a", "", &full)?;
    let only_b = f.library.create_profile("Only b", "", &spec(&[&b]))?;

    f.library.remove(&a)?;
    let view = f.library.profile(with_a, None)?;
    assert_eq!(view.current_version, 2);
    assert!(view.spec.items == [b.clone()] && view.spec.manual_only.is_empty());
    assert!(f.library.profile(with_a, Some(1))?.spec.items.contains(&a));
    assert_eq!(f.library.profile(only_b, None)?.current_version, 1);

    f.library.remove(&settings)?;
    f.library.remove(&api)?;
    let view = f.library.profile(with_a, None)?;
    assert_eq!(view.current_version, 4);
    assert_eq!((view.spec.settings, view.spec.mcp.len()), (None, 0));
    Ok(())
}

#[test]
fn a_failed_removal_leaves_profiles_alone() -> TestResult {
    let mut f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let id = f.library.create_profile("P", "", &spec(&[&a]))?;
    assert!(f.library.remove("ffffffffffffffff").is_err());
    assert_eq!(f.library.profile(id, None)?.current_version, 1);
    Ok(())
}

#[test]
fn removing_many_removes_only_those_and_rolls_back_on_failure() -> TestResult {
    let mut f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    let id = f.library.create_profile("P", "", &spec(&[&a, &b]))?;

    // An unknown id anywhere in the set: nothing goes, on disk or in the index.
    let before = f.library.list(&Default::default())?.total;
    let bad = vec![a.clone(), "ffffffffffffffff".to_owned()];
    assert!(f.library.remove_many(&bad).is_err());
    assert_eq!(f.library.list(&Default::default())?.total, before);
    assert!(f.id(ArtifactKind::Skill, "a").is_ok());
    assert_eq!(f.library.profile(id, None)?.current_version, 1);

    f.library.remove_many(&[a.clone(), b.clone(), a.clone()])?;
    assert_eq!(f.library.list(&Default::default())?.total, before - 2);
    assert!(f.id(ArtifactKind::Skill, "a").is_err());
    assert!(f.id(ArtifactKind::Skill, "b").is_err());
    assert_eq!(f.id(ArtifactKind::Agent, "helper")?, helper);
    let view = f.library.profile(id, None)?;
    assert!(view.spec.items.is_empty());
    // One bulk delete is one new version of each profile, not one per artifact.
    assert_eq!(view.current_version, 2);
    Ok(())
}

#[test]
fn the_removal_plan_lists_every_match_with_its_profiles() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    f.library.create_profile("P", "", &spec(&[&a]))?;
    let filter = crate::library::Filter {
        types: vec![ArtifactKind::Skill],
        ..Default::default()
    };
    let plan = f.library.removal_plan(&filter)?;
    let names: Vec<&str> = plan.artifacts.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(plan.artifacts[0].id, a);
    assert_eq!(plan.artifacts[0].profiles.len(), 1);
    assert_eq!(plan.artifacts[0].profiles[0].name, "P");
    assert_eq!(plan.artifacts[1].id, b);
    assert!(plan.artifacts[1].profiles.is_empty());
    Ok(())
}
