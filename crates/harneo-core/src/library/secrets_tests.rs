//! Literal secrets never reach the library: they become `${KEY}` placeholders in the copy.

use std::fs;
use std::io;
use std::path::Path;

use super::*;
use crate::artifact::ArtifactKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

/// Not token-shaped; found only through its key name.
const LITERAL: &str = "harneo-test-literal-value-123";

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn fixture(root: &Path) -> io::Result<()> {
    write(
        root,
        ".mcp.json",
        &format!(
            r#"{{"mcpServers":{{"api":{{"type":"http","url":"https://example.test/mcp",
"headers":{{"Authorization":"Bearer {LITERAL}","X-Api-Key":"{LITERAL}","X-Trace":"${{TRACE_ID}}"}},
"env":{{"API_TOKEN":"{LITERAL}","LOG_LEVEL":"debug"}}}}}}}}"#
        ),
    )?;
    write(
        root,
        ".claude/settings.json",
        &format!(
            r#"{{"model":"opus","env":{{"API_TOKEN":"{LITERAL}","DB_PASSWORD":"${{DB_PASSWORD}}","LOG_LEVEL":"debug"}}}}"#
        ),
    )?;
    write(
        root,
        "agents/a.md",
        &format!("---\nname: a\n---\nThe API_TOKEN is {LITERAL}\n"),
    )
}

fn open() -> std::result::Result<
    (tempfile::TempDir, tempfile::TempDir, Library, Source),
    Box<dyn std::error::Error>,
> {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    fixture(src.path())?;
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    Ok((data, src, library, source))
}

fn candidate(preview: &Preview, kind: ArtifactKind) -> Result<&Candidate> {
    preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == kind)
        .ok_or_else(|| Error::NotInPreview {
            id: format!("{kind:?}"),
        })
}

fn read_json(path: &Path) -> std::result::Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn every_byte_under(dir: &Path, out: &mut Vec<u8>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            every_byte_under(&entry.path(), out)?;
        } else {
            out.extend(fs::read(entry.path())?);
        }
    }
    Ok(())
}

#[test]
fn preview_lists_the_keys_that_will_be_replaced() -> TestResult {
    let (_data, _src, library, source) = open()?;
    let preview = library.preview(&source)?;
    assert_eq!(
        candidate(&preview, ArtifactKind::Mcp)?.replaced_secret_keys,
        [
            "env.API_TOKEN",
            "headers.Authorization",
            "headers.X-Api-Key"
        ]
    );
    assert_eq!(
        candidate(&preview, ArtifactKind::Settings)?.replaced_secret_keys,
        ["env.API_TOKEN"]
    );
    assert!(candidate(&preview, ArtifactKind::Agent)?
        .replaced_secret_keys
        .is_empty());
    assert!(!serde_json::to_string(&preview)?.contains(LITERAL));
    Ok(())
}

#[test]
fn stored_files_hold_placeholders_and_provenance_the_keys() -> TestResult {
    let (data, _src, mut library, source) = open()?;
    let preview = library.preview(&source)?;
    let ids: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;

    let mcp = candidate(&preview, ArtifactKind::Mcp)?;
    let server = read_json(
        &data
            .path()
            .join("library/artifacts")
            .join(&mcp.id)
            .join("server.json"),
    )?;
    assert_eq!(
        server["api"]["headers"]["Authorization"],
        "Bearer ${AUTHORIZATION}"
    );
    assert_eq!(server["api"]["headers"]["X-Api-Key"], "${X_API_KEY}");
    assert_eq!(server["api"]["headers"]["X-Trace"], "${TRACE_ID}");
    assert_eq!(server["api"]["env"]["API_TOKEN"], "${API_TOKEN}");
    assert_eq!(server["api"]["env"]["LOG_LEVEL"], "debug");

    let settings = candidate(&preview, ArtifactKind::Settings)?;
    let stored = data
        .path()
        .join("library/artifacts")
        .join(&settings.id)
        .join("settings.json");
    let text = fs::read_to_string(&stored)?;
    assert!(text.contains("\n  "), "pretty-printed");
    let json = read_json(&stored)?;
    assert_eq!(json["env"]["API_TOKEN"], "${API_TOKEN}");
    assert_eq!(json["env"]["DB_PASSWORD"], "${DB_PASSWORD}");
    assert_eq!(json["env"]["LOG_LEVEL"], "debug");
    assert_eq!(json["model"], "opus");

    let detail = library.detail(&settings.id)?;
    assert_eq!(detail.provenance.replaced_secret_keys, ["env.API_TOKEN"]);
    let detail = library.detail(&mcp.id)?;
    assert_eq!(detail.provenance.replaced_secret_keys.len(), 3);
    assert!(!serde_json::to_string(&detail)?.contains(LITERAL));
    Ok(())
}

#[test]
fn the_literal_value_is_nowhere_under_the_data_dir() -> TestResult {
    let (data, _src, mut library, source) = open()?;
    let preview = library.preview(&source)?;
    // The agent's body is prose, copied as is; leave it out so the check is about secrets.
    let ids: Vec<ArtifactId> = preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind != ArtifactKind::Agent)
        .map(|c| c.id.clone())
        .collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let mut bytes = Vec::new();
    every_byte_under(data.path(), &mut bytes)?;
    assert!(!bytes.is_empty());
    assert!(!bytes
        .windows(LITERAL.len())
        .any(|w| w == LITERAL.as_bytes()));
    // Dropping the library flushes the database; check again once it is closed.
    drop(library);
    let mut bytes = Vec::new();
    every_byte_under(data.path(), &mut bytes)?;
    assert!(!bytes
        .windows(LITERAL.len())
        .any(|w| w == LITERAL.as_bytes()));
    Ok(())
}

#[test]
fn other_files_are_copied_byte_for_byte() -> TestResult {
    let (data, src, mut library, source) = open()?;
    let preview = library.preview(&source)?;
    let agent = candidate(&preview, ArtifactKind::Agent)?;
    library.import(
        &preview,
        std::slice::from_ref(&agent.id),
        &TagChoices::new(),
    )?;
    let stored = fs::read(
        data.path()
            .join("library/artifacts")
            .join(&agent.id)
            .join("a.md"),
    )?;
    assert_eq!(stored, fs::read(src.path().join("agents/a.md"))?);
    Ok(())
}

#[test]
fn an_unparseable_settings_file_is_refused_and_nothing_is_stored() -> TestResult {
    let (data, src, mut library, source) = open()?;
    let preview = library.preview(&source)?;
    let settings = candidate(&preview, ArtifactKind::Settings)?;
    write(
        src.path(),
        ".claude/settings.json",
        &format!(r#"{{"env":{{"API_TOKEN":"{LITERAL}",}}"#),
    )?;
    let err = library
        .import(
            &preview,
            std::slice::from_ref(&settings.id),
            &TagChoices::new(),
        )
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(err.contains(".claude/settings.json"), "{err}");
    assert!(!err.contains(LITERAL));
    assert_eq!(
        fs::read_dir(data.path().join("library/artifacts"))?.count(),
        0
    );
    Ok(())
}

#[test]
fn key_names_become_upper_snake_case() {
    for (key, want) in [
        ("apiKey", "${API_KEY}"),
        ("X-Api-Key", "${X_API_KEY}"),
        ("api_key", "${API_KEY}"),
        ("Authorization", "${AUTHORIZATION}"),
        ("--", "${SECRET}"),
    ] {
        assert_eq!(copy::placeholder(key), want);
    }
}

#[test]
fn a_token_in_a_default_is_replaced_and_a_harmless_default_stays() -> TestResult {
    let (data, src, mut library, source) = open()?;
    let token = ["gh", "p_", &"a".repeat(36)].concat();
    write(
        src.path(),
        ".claude/settings.json",
        &format!(r#"{{"env":{{"API_KEY":"${{API_KEY:-{token}}}","PORT":"${{PORT:-8080}}"}}}}"#),
    )?;
    let preview = library.preview(&source)?;
    let ids: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let settings = candidate(&preview, ArtifactKind::Settings)?;
    let stored = data
        .path()
        .join("library/artifacts")
        .join(&settings.id)
        .join("settings.json");
    let json = read_json(&stored)?;
    assert_eq!(json["env"]["API_KEY"], "${API_KEY}");
    assert_eq!(json["env"]["PORT"], "${PORT:-8080}");
    assert!(!fs::read_to_string(&stored)?.contains(&token));
    Ok(())
}
