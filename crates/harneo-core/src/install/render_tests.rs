use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::params;
use serde_json::Value;

use super::render::{set_manual_only, with_harness_import};
use super::*;
use crate::artifact::{ArtifactKind, Origin};
use crate::library::{Library, Source, TagChoices};
use crate::profiles::{Generated, McpChoice, PathFrom, ProfileSpec};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const SKILL: &str = "---\nname: s\ndescription: Does a thing. Then more.\n---\nBody.\n";
const SETTINGS: &str = "{\n  \"permissions\": {\n    \"allow\": [\n      \"Read\"\n    ]\n  }\n}";

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

struct Lib {
    _data: tempfile::TempDir,
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

    fn text(&self, rendered: &Rendered, path: &str) -> std::result::Result<String, String> {
        let file = rendered
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| format!("{path} not rendered"))?;
        String::from_utf8(file.bytes.clone()).map_err(|e| e.to_string())
    }
}

fn import(files: &[(&str, &str)]) -> std::result::Result<Lib, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    for (rel, text) in files {
        write(src.path(), rel, text)?;
    }
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(Lib {
        _data: data,
        _src: src,
        library,
    })
}

fn full_project() -> std::result::Result<Lib, Box<dyn std::error::Error>> {
    import(&[
        (".claude/skills/s/SKILL.md", SKILL),
        (".claude/skills/s/scripts/run.sh", "echo hi\n"),
        (
            ".claude/agents/rev.md",
            "---\nname: rev\ndescription: Reviews code\n---\nB\n",
        ),
        ("CLAUDE.md", "# Project\n\nNotes.\n"),
        (".claude/settings.json", SETTINGS),
        (
            ".mcp.json",
            "{\"mcpServers\":{\"api\":{\"command\":\"npx\",\"args\":[\"x\"],\"env\":{\"TOKEN\":\"${TOKEN}\"}}}}\n",
        ),
        (
            "plugins/p/.claude-plugin/plugin.json",
            "{\"name\":\"p\",\"description\":\"A plugin | with pipe\"}\n",
        ),
    ])
}

/// Makes the library's plugin look like one installed from `market`.
fn from_marketplace(
    lib: &Lib,
    market: &str,
    repo: Option<&str>,
    official: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let id = lib.id(ArtifactKind::Plugin, "p")?;
    let conn = &lib.library.store.conn;
    let json: String = conn.query_row(
        "SELECT artifact FROM artifacts WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let mut artifact: crate::artifact::Artifact = serde_json::from_str(&json)?;
    artifact.origin = Origin::Marketplace {
        name: market.to_owned(),
        repo: repo.map(str::to_owned),
        official,
    };
    conn.execute(
        "UPDATE artifacts SET artifact = ?1 WHERE id = ?2",
        params![serde_json::to_string(&artifact)?, id],
    )?;
    Ok(())
}

fn everything(lib: &Lib) -> std::result::Result<ProfileSpec, Box<dyn std::error::Error>> {
    Ok(ProfileSpec {
        items: vec![
            lib.id(ArtifactKind::Skill, "s")?,
            lib.id(ArtifactKind::Agent, "rev")?,
            lib.id(ArtifactKind::ClaudeMd, "CLAUDE.md")?,
            lib.id(ArtifactKind::Plugin, "p")?,
        ],
        settings: Some(lib.id(ArtifactKind::Settings, "settings")?),
        mcp: vec![McpChoice {
            artifact: lib.id(ArtifactKind::Mcp, "api")?,
            env: BTreeMap::new(),
        }],
        manual_only: BTreeMap::new(),
    })
}

#[test]
fn every_kind_is_rendered_and_copies_are_byte_equal() -> TestResult {
    let lib = full_project()?;
    from_marketplace(&lib, "official", Some("anthropics/x"), true)?;
    let rendered = lib
        .library
        .render(&everything(&lib)?, "Full", 3, "2026-10-23")?;
    let paths: Vec<&str> = rendered.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            ".claude/agents/rev.md",
            ".claude/settings.json",
            ".claude/skills/s/SKILL.md",
            ".claude/skills/s/scripts/run.sh",
            ".mcp.json",
            "CLAUDE.md",
            "HARNESS.md"
        ]
    );
    assert_eq!(lib.text(&rendered, ".claude/skills/s/SKILL.md")?, SKILL);
    assert_eq!(
        lib.text(&rendered, ".claude/skills/s/scripts/run.sh")?,
        "echo hi\n"
    );
    assert_eq!(
        lib.text(&rendered, ".claude/agents/rev.md")?,
        "---\nname: rev\ndescription: Reviews code\n---\nB\n"
    );
    let from = |p: &str| rendered.files.iter().find(|f| f.path == p).map(|f| &f.from);
    assert_eq!(
        from(".mcp.json"),
        Some(&PathFrom::Generated {
            generated: Generated::McpJson
        })
    );
    // Same inputs, same bytes.
    assert_eq!(
        rendered,
        lib.library
            .render(&everything(&lib)?, "Full", 3, "2026-10-23")?
    );
    Ok(())
}

#[test]
fn manual_only_changes_only_the_frontmatter_key() -> TestResult {
    let lib = full_project()?;
    let skill = lib.id(ArtifactKind::Skill, "s")?;
    let spec = ProfileSpec {
        items: vec![skill.clone()],
        manual_only: BTreeMap::from([(skill, true)]),
        ..ProfileSpec::default()
    };
    let rendered = lib.library.render(&spec, "P", 1, "d")?;
    assert_eq!(
        lib.text(&rendered, ".claude/skills/s/SKILL.md")?,
        "---\nname: s\ndescription: Does a thing. Then more.\ndisable-model-invocation: true\n---\nBody.\n"
    );
    assert_eq!(
        lib.text(&rendered, ".claude/skills/s/scripts/run.sh")?,
        "echo hi\n"
    );
    Ok(())
}

#[test]
fn frontmatter_edit_adds_replaces_creates_and_leaves_the_rest() {
    let edit = |t: &str, on: bool| {
        String::from_utf8(set_manual_only(t.as_bytes(), on)).unwrap_or_default()
    };
    // Replace, keeping neighbours and the line ending.
    assert_eq!(
        edit(
            "---\r\na: 1\r\ndisable-model-invocation: false\r\nb: 2\r\n---\r\nBody\r\n",
            true
        ),
        "---\r\na: 1\r\ndisable-model-invocation: true\r\nb: 2\r\n---\r\nBody\r\n"
    );
    // Add with CRLF.
    assert_eq!(
        edit("---\r\na: 1\r\n---\r\nBody", true),
        "---\r\na: 1\r\ndisable-model-invocation: true\r\n---\r\nBody"
    );
    // Already right, or nothing to turn off: untouched.
    let already = "---\ndisable-model-invocation: \"true\"\n---\nB\n";
    assert_eq!(edit(already, true), already);
    assert_eq!(edit("---\na: 1\n---\nB\n", false), "---\na: 1\n---\nB\n");
    // Turned off by the profile.
    assert_eq!(
        edit("---\ndisable-model-invocation: true\n---\nB\n", false),
        "---\ndisable-model-invocation: false\n---\nB\n"
    );
    // Frontmatter created only when absent and on.
    assert_eq!(
        edit("Just text\n", true),
        "---\ndisable-model-invocation: true\n---\nJust text\n"
    );
    assert_eq!(edit("Just text\n", false), "Just text\n");
    // BOM kept; unclosed frontmatter untouched.
    assert_eq!(
        edit("\u{feff}---\na: 1\n---\nB\n", true),
        "\u{feff}---\na: 1\ndisable-model-invocation: true\n---\nB\n"
    );
    assert_eq!(edit("---\na: 1\nB\n", true), "---\na: 1\nB\n");
    // Not UTF-8: untouched.
    assert_eq!(set_manual_only(&[0xff, 0xfe], true), vec![0xff, 0xfe]);
}

#[test]
fn settings_gets_plugins_and_a_template_without_plugins_is_untouched() -> TestResult {
    let lib = full_project()?;
    let settings = lib.id(ArtifactKind::Settings, "settings")?;
    let plain = ProfileSpec {
        settings: Some(settings),
        ..ProfileSpec::default()
    };
    let rendered = lib.library.render(&plain, "P", 1, "d")?;
    assert_eq!(lib.text(&rendered, ".claude/settings.json")?, SETTINGS);

    from_marketplace(&lib, "addy-skills", Some("addyosmani/agent-skills"), false)?;
    let rendered = lib.library.render(&everything(&lib)?, "P", 1, "d")?;
    let json: Value = serde_json::from_str(&lib.text(&rendered, ".claude/settings.json")?)?;
    assert_eq!(json["permissions"]["allow"][0], "Read");
    assert_eq!(json["enabledPlugins"]["p@addy-skills"], true);
    assert_eq!(
        json["extraKnownMarketplaces"]["addy-skills"]["source"],
        serde_json::json!({"source": "github", "repo": "addyosmani/agent-skills"})
    );

    from_marketplace(&lib, "official", Some("anthropics/x"), true)?;
    let rendered = lib.library.render(&everything(&lib)?, "P", 1, "d")?;
    let json: Value = serde_json::from_str(&lib.text(&rendered, ".claude/settings.json")?)?;
    assert_eq!(json["enabledPlugins"]["p@official"], true);
    assert!(json.get("extraKnownMarketplaces").is_none());
    Ok(())
}

#[test]
fn plugins_without_a_template_make_a_minimal_settings_file() -> TestResult {
    let lib = full_project()?;
    from_marketplace(&lib, "addy-skills", Some("addyosmani/agent-skills"), false)?;
    let spec = ProfileSpec {
        items: vec![lib.id(ArtifactKind::Plugin, "p")?],
        ..ProfileSpec::default()
    };
    let rendered = lib.library.render(&spec, "P", 1, "d")?;
    let file = rendered
        .files
        .iter()
        .find(|f| f.path == ".claude/settings.json")
        .ok_or("no settings")?;
    assert_eq!(
        file.from,
        PathFrom::Generated {
            generated: Generated::Settings
        }
    );
    let json: Value = serde_json::from_slice(&file.bytes)?;
    let keys: Vec<&String> = json.as_object().ok_or("not an object")?.keys().collect();
    assert_eq!(keys, ["enabledPlugins", "extraKnownMarketplaces"]);
    Ok(())
}

#[test]
fn mcp_json_holds_the_servers_with_the_profiles_env() -> TestResult {
    let lib = full_project()?;
    let api = lib.id(ArtifactKind::Mcp, "api")?;
    let spec = ProfileSpec {
        mcp: vec![McpChoice {
            artifact: api,
            env: BTreeMap::from([
                ("TOKEN".to_owned(), "${MY_TOKEN}".to_owned()),
                ("EXTRA".to_owned(), "${EXTRA}".to_owned()),
            ]),
        }],
        ..ProfileSpec::default()
    };
    let rendered = lib.library.render(&spec, "P", 1, "d")?;
    let json: Value = serde_json::from_str(&lib.text(&rendered, ".mcp.json")?)?;
    assert_eq!(
        json,
        serde_json::json!({"mcpServers": {"api": {
            "command": "npx", "args": ["x"],
            "env": {"EXTRA": "${EXTRA}", "TOKEN": "${MY_TOKEN}"}
        }}})
    );
    Ok(())
}

#[test]
fn claude_md_gets_the_import_once_and_only_with_a_claude_md_artifact() -> TestResult {
    let lib = full_project()?;
    let md = lib.id(ArtifactKind::ClaudeMd, "CLAUDE.md")?;
    let spec = ProfileSpec {
        items: vec![md],
        ..ProfileSpec::default()
    };
    let rendered = lib.library.render(&spec, "P", 1, "d")?;
    assert_eq!(
        lib.text(&rendered, "CLAUDE.md")?,
        "# Project\n\nNotes.\n\n@HARNESS.md\n"
    );
    assert_eq!(
        with_harness_import(b"# P\n  @HARNESS.md  \nrest"),
        b"# P\n  @HARNESS.md  \nrest".to_vec()
    );
    assert_eq!(
        with_harness_import(b"no newline"),
        b"no newline\n\n@HARNESS.md\n".to_vec()
    );
    assert_eq!(with_harness_import(b""), b"@HARNESS.md\n".to_vec());

    let skill = lib.id(ArtifactKind::Skill, "s")?;
    let rendered = lib.library.render(
        &ProfileSpec {
            items: vec![skill],
            ..ProfileSpec::default()
        },
        "P",
        1,
        "d",
    )?;
    assert!(rendered.files.iter().all(|f| f.path != "CLAUDE.md"));
    Ok(())
}

#[test]
fn harness_md_lists_artifacts_files_and_the_managed_line() -> TestResult {
    let lib = full_project()?;
    from_marketplace(&lib, "official", Some("anthropics/x"), true)?;
    let rendered = lib
        .library
        .render(&everything(&lib)?, "Full stack", 3, "2026-10-23")?;
    let text = lib.text(&rendered, "HARNESS.md")?;
    assert!(text.starts_with(
        "# Harness\n\nInstalled by Harneo from profile **Full stack v3** on 2026-10-23.\n\n\
         | Artifact | Type | Source | Why it's here |\n|---|---|---|---|\n"
    ));
    assert!(
        text.contains("| s | Skill | Local | Does a thing |\n"),
        "{text}"
    );
    assert!(text.contains("| rev | Subagent | Local | Reviews code |\n"));
    assert!(text.contains("| p | Plugin | anthropics/x | A plugin \\| with pipe |\n"));
    assert!(text.contains("## Files\n\n- `.claude/agents/rev.md`\n"));
    assert!(text.contains("- `HARNESS.md`\n"));
    assert!(text
        .ends_with("\nManaged by Harneo: edit the library copy, or your edits show as drift.\n"));
    Ok(())
}

#[test]
fn rendered_files_round_trip_through_json_as_base64() -> TestResult {
    let file = RenderedFile {
        path: "a".to_owned(),
        bytes: vec![0, 159, 255],
        from: PathFrom::Generated {
            generated: Generated::HarnessMd,
        },
    };
    let json = serde_json::to_value(&file)?;
    assert_eq!(json["bytes"], "AJ//");
    assert_eq!(serde_json::from_value::<RenderedFile>(json)?, file);
    Ok(())
}
