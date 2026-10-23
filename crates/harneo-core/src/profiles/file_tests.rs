use std::path::Path;

use serde_json::Value;

use super::tests::{write, TestResult};
use super::*;
use crate::library::{Source, TagChoices};

struct Lib {
    data: tempfile::TempDir,
    _src: tempfile::TempDir,
    library: Library,
}

impl Lib {
    fn id(&self, kind: ArtifactKind, name: &str) -> std::result::Result<String, String> {
        self.library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} not in library"))
    }
}

fn source() -> std::result::Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    let r = src.path();
    write(
        r,
        "skills/a/SKILL.md",
        "---\nname: a\ndescription: a skill\n---\nBody.\n",
    )?;
    std::fs::create_dir_all(r.join("skills/a/assets"))?;
    std::fs::write(r.join("skills/a/assets/blob.bin"), [0xff_u8, 0xfe, 0, 1, 2])?;
    write(
        r,
        "agents/helper.md",
        "---\nname: helper\ndescription: h\n---\nBody.\n",
    )?;
    write(
        r,
        "settings.json",
        "{\"permissions\":{\"allow\":[\"Read\"]}}\n",
    )?;
    write(
        r,
        ".mcp.json",
        "{\"mcpServers\":{\"api\":{\"command\":\"npx\",\"args\":[\"x\"],\"env\":{\"HOME_URL\":\"${HOME_URL}\"}}}}\n",
    )?;
    Ok(src)
}

fn library_from(src: tempfile::TempDir) -> std::result::Result<Lib, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(Lib {
        data,
        _src: src,
        library,
    })
}

fn empty() -> std::result::Result<Lib, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    Ok(Lib {
        data,
        _src: tempfile::tempdir()?,
        library,
    })
}

/// A profile of everything in `lib`, exported.
fn exported(
    lib: &Lib,
) -> std::result::Result<(ProfileId, ProfileSpec, Vec<u8>), Box<dyn std::error::Error>> {
    let mut env = BTreeMap::new();
    env.insert("HOME_URL".to_owned(), "${HOME_URL}".to_owned());
    let spec = ProfileSpec {
        items: vec![
            lib.id(ArtifactKind::Skill, "a")?,
            lib.id(ArtifactKind::Agent, "helper")?,
        ],
        settings: Some(lib.id(ArtifactKind::Settings, "settings")?),
        mcp: vec![McpChoice {
            artifact: lib.id(ArtifactKind::Mcp, "api")?,
            env,
        }],
        manual_only: BTreeMap::new(),
    };
    let id = lib.library.create_profile("Heavy", "desc", &spec)?;
    let bytes = lib.library.export_profile(id, None)?;
    Ok((id, spec, bytes))
}

fn edit(
    bytes: &[u8],
    f: impl FnOnce(&mut Value),
) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut value: Value = serde_json::from_slice(bytes)?;
    f(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

fn stored(lib: &Lib, id: &str, rel: &str) -> std::path::PathBuf {
    lib.data
        .path()
        .join("library/artifacts")
        .join(id)
        .join(Path::new(rel))
}

#[test]
fn a_profile_survives_the_trip_to_a_fresh_data_dir() -> TestResult {
    let one = library_from(source()?)?;
    let (_, spec, bytes) = exported(&one)?;
    let file: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(file["format"], "harneo-profile");
    assert_eq!(file["formatVersion"], 1);
    assert_eq!(file["profile"]["version"], 1);

    let mut two = empty()?;
    let preview = two.library.import_profile_file(&bytes)?;
    assert_eq!(preview.profile.name, "Heavy");
    assert_eq!(preview.artifacts.len(), 4);
    assert!(preview.artifacts.iter().all(|a| !a.in_library));
    assert!(preview
        .artifacts
        .iter()
        .all(|a| !a.source.is_empty() && a.file_count >= 1));
    assert!(!preview.name_clash);
    assert!(
        two.library
            .list(&crate::library::Query {
                filter: crate::library::Filter::default(),
                sort: crate::library::SortKey::Name,
                descending: false,
                offset: 0,
                limit: 10,
            })?
            .total
            == 0,
        "a preview stores nothing"
    );

    let report = two.library.import_profile_file_commit(&bytes, None)?;
    assert_eq!(report.name, "Heavy");
    assert_eq!(report.imported.len(), 4);
    let view = two.library.profile(report.profile_id, None)?;
    assert_eq!((view.version, view.description.as_str()), (1, "desc"));
    assert_eq!(view.spec, spec);

    // Each artifact is there with the provenance and files it had, and an audit.
    for (kind, name) in [
        (ArtifactKind::Skill, "a"),
        (ArtifactKind::Agent, "helper"),
        (ArtifactKind::Settings, "settings"),
        (ArtifactKind::Mcp, "api"),
    ] {
        let id = one.id(kind, name)?;
        let before = one.library.detail(&id)?;
        let after = two.library.detail(&id)?;
        assert_eq!(after.artifact, before.artifact);
        assert_eq!(after.provenance, before.provenance);
        assert_eq!(after.files, before.files);
        assert!(after.audit.is_some(), "{name} is audited");
        for rel in &before.files {
            assert_eq!(
                std::fs::read(stored(&two, &id, rel))?,
                std::fs::read(stored(&one, &id, rel))?,
                "{name}/{rel}"
            );
        }
    }
    // The binary file went through base64.
    assert!(String::from_utf8(bytes.clone())?.contains("\"base64\""));
    // The imported profile lays out and exports the same as the original.
    assert_eq!(two.library.layout(&view.spec)?, one.library.layout(&spec)?);

    // Importing again: everything is in the library, the name clashes.
    let again = two.library.import_profile_file(&bytes)?;
    assert!(again.name_clash);
    assert!(again.artifacts.iter().all(|a| a.in_library));
    let second = two.library.import_profile_file_commit(&bytes, None)?;
    assert_eq!(second.name, "Heavy (imported)");
    assert!(second.imported.is_empty());
    let third = two.library.import_profile_file_commit(&bytes, None)?;
    assert_eq!(third.name, "Heavy (imported 2)");
    let named = two
        .library
        .import_profile_file_commit(&bytes, Some("Mine"))?;
    assert_eq!(named.name, "Mine");
    assert!(matches!(
        two.library.import_profile_file_commit(&bytes, Some("mine")),
        Err(crate::Error::ProfileExists { .. })
    ));
    Ok(())
}

#[test]
fn an_identical_artifact_under_another_id_is_matched_by_digest() -> TestResult {
    let one = library_from(source()?)?;
    let (_, _, bytes) = exported(&one)?;
    // The same files imported from another folder: other ids, same digests.
    let mut two = library_from(source()?)?;
    let local_a = two.id(ArtifactKind::Skill, "a")?;
    assert_ne!(local_a, one.id(ArtifactKind::Skill, "a")?);
    let preview = two.library.import_profile_file(&bytes)?;
    assert!(preview.artifacts.iter().all(|a| a.in_library));
    let report = two.library.import_profile_file_commit(&bytes, None)?;
    assert!(report.imported.is_empty());
    let view = two.library.profile(report.profile_id, None)?;
    assert!(view.spec.items.contains(&local_a));
    assert!(!view.spec.items.contains(&one.id(ArtifactKind::Skill, "a")?));
    Ok(())
}

#[test]
fn a_newer_format_a_foreign_file_and_a_big_file_are_refused() -> TestResult {
    let one = library_from(source()?)?;
    let (_, _, bytes) = exported(&one)?;
    let two = empty()?;
    let newer = edit(&bytes, |v| v["formatVersion"] = Value::from(2))?;
    let error = two
        .library
        .import_profile_file(&newer)
        .err()
        .map(|e| e.to_string());
    assert!(error.is_some_and(|e| e.contains("newer")));
    let foreign = edit(&bytes, |v| v["format"] = Value::from("other"))?;
    assert!(two.library.import_profile_file(&foreign).is_err());
    assert!(two.library.import_profile_file(b"not json").is_err());
    let huge = vec![b' '; MAX_FILE_BYTES + 1];
    assert!(two.library.import_profile_file(&huge).is_err());
    Ok(())
}

#[test]
fn a_literal_secret_stops_the_export_and_the_import() -> TestResult {
    let one = library_from(source()?)?;
    let (profile, _, bytes) = exported(&one)?;
    let secret = ["gh", "p_", &"a".repeat(36)].concat();
    let settings = one.id(ArtifactKind::Settings, "settings")?;
    let tampered = format!("{{\"env\":{{\"API_TOKEN\":\"{secret}\"}}}}");

    // Import: a file whose settings carry a literal is refused, naming the key only.
    let bad = edit(&bytes, |v| {
        if let Some(artifacts) = v["artifacts"].as_array_mut() {
            for a in artifacts
                .iter_mut()
                .filter(|a| a["id"] == settings.as_str())
            {
                a["files"][0]["content"] = Value::from(tampered.clone());
            }
        }
    })?;
    let two = empty()?;
    let error = two
        .library
        .import_profile_file(&bad)
        .err()
        .map(|e| e.to_string());
    let message = error.unwrap_or_default();
    assert!(message.contains("env.API_TOKEN"), "{message}");
    assert!(!message.contains(&secret));

    // Export: the library copy was edited by hand to hold a literal.
    std::fs::write(stored(&one, &settings, "settings.json"), &tampered)?;
    let error = one
        .library
        .export_profile(profile, None)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(error.contains("env.API_TOKEN"), "{error}");
    assert!(!error.contains(&secret));
    Ok(())
}

#[test]
fn an_unsafe_path_in_a_file_stores_nothing() -> TestResult {
    let one = library_from(source()?)?;
    let (_, _, bytes) = exported(&one)?;
    let skill = one.id(ArtifactKind::Skill, "a")?;
    let bad = edit(&bytes, |v| {
        if let Some(artifacts) = v["artifacts"].as_array_mut() {
            for a in artifacts.iter_mut().filter(|a| a["id"] == skill.as_str()) {
                a["files"][0]["path"] = Value::from("../../escape.md");
            }
        }
    })?;
    let mut two = empty()?;
    assert!(two.library.import_profile_file_commit(&bad, None).is_err());
    assert!(two.library.profiles()?.is_empty());
    let page = two.library.list(&crate::library::Query {
        filter: crate::library::Filter::default(),
        sort: crate::library::SortKey::Name,
        descending: false,
        offset: 0,
        limit: 10,
    })?;
    assert_eq!(page.total, 0);
    let leftover = std::fs::read_dir(two.data.path().join("library/artifacts"))?.count();
    assert_eq!(leftover, 0);
    assert!(!two.data.path().join("escape.md").exists());
    Ok(())
}
