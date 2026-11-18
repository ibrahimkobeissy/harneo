//! ADR-008 import: any plugin's parts are offered, each kept part is its own library item with its
//! plugin and supporting paths, and the plugin's supporting files are stored once. Claude is a
//! fixture script; the real CLI never runs.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;
use crate::bundles::claude_cli_tests::{config, script};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

/// The skills of the fixture plugin.
pub(crate) const SKILLS: [&str; 4] = [
    "incremental-implementation",
    "test-driven-development",
    "planning",
    "code-review",
];

/// A plugin at the repo root shaped like `addyosmani/agent-skills`: skills linking
/// `../../references/`, commands and an agent that call `agent-skills:<skill>`, and `docs/`.
/// Scan options for a preview where the user picked parts of every plugin (plugins start whole
/// since ADR-008's 2026-11-18 amendment).
pub(crate) fn picking_parts() -> crate::artifact::ScanOptions {
    crate::artifact::ScanOptions {
        unpack_by_default: true,
        ..crate::artifact::ScanOptions::default()
    }
}

pub(crate) fn agent_skills_repo(root: &Path) -> io::Result<()> {
    write(
        root,
        ".claude-plugin/plugin.json",
        r#"{"name":"agent-skills","version":"1.0.0","description":"Engineering skills","skills":["./skills/incremental-implementation","./skills/test-driven-development","./skills/planning","./skills/code-review"],"commands":["./commands/build.md","./commands/test.md"],"experimental":{"x":1}}"#,
    )?;
    for skill in SKILLS {
        write(
            root,
            &format!("skills/{skill}/SKILL.md"),
            &format!(
                "---\nname: {skill}\ndescription: {skill} things\n---\nSee [checklist](../../references/checklist.md).\n"
            ),
        )?;
    }
    write(
        root,
        "commands/build.md",
        "---\ndescription: Build\n---\nUse agent-skills:incremental-implementation, then /agent-skills:test-driven-development.\n",
    )?;
    write(
        root,
        "commands/test.md",
        "---\ndescription: Test\n---\nUse agent-skills:test-driven-development.\n",
    )?;
    write(
        root,
        "agents/code-reviewer.md",
        "---\nname: code-reviewer\ndescription: Reviews\n---\nReview the diff.\n",
    )?;
    write(root, "references/checklist.md", "- item\n")?;
    write(root, "references/security.md", "- secure\n")?;
    write(root, "docs/guide.md", "# For people\n")?;
    write(root, "README.md", "# agent skills\n")
}

/// A library, a source folder and a fixture `claude` that answers what each part needs.
pub(crate) struct Lab {
    pub repo: PathBuf,
    pub library: Library,
    bin: tempfile::TempDir,
    _base: tempfile::TempDir,
    _data: tempfile::TempDir,
}

impl Lab {
    pub(crate) fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let base = tempfile::tempdir()?;
        let repo = base.path().join("repo");
        agent_skills_repo(&repo)?;
        let data = tempfile::tempdir()?;
        Ok(Self {
            library: Library::open(data.path())?,
            bin: tempfile::tempdir()?,
            repo,
            _base: base,
            _data: data,
        })
    }

    pub(crate) fn source(&self) -> Source {
        Source::Local {
            path: self.repo.clone(),
        }
    }

    /// Makes `claude` answer `answer`.
    pub(crate) fn claude(&mut self, answer: &serde_json::Value) -> io::Result<()> {
        let d = self.bin.path().display();
        fs::write(self.bin.path().join("answer.json"), answer.to_string())?;
        let program = script(
            self.bin.path(),
            "claude",
            &format!(
                "cat > /dev/null\nprintf '{{\"is_error\":false,\"structured_output\":'\ncat '{d}/answer.json'\nprintf '}}'"
            ),
        )?;
        self.library.claude = config(&program);
        Ok(())
    }

    /// The preview with the plugin split into parts (the user picked parts) and Claude's answer:
    /// every skill needs `references`, docs are not needed.
    pub(crate) fn preview(&mut self) -> std::result::Result<Preview, Box<dyn std::error::Error>> {
        let first = self
            .library
            .preview_with(&self.source(), &picking_parts())?;
        let parts: Vec<_> = first
            .candidates
            .iter()
            .map(|c| {
                let needs = if c.artifact.kind == crate::artifact::ArtifactKind::Skill {
                    vec![json!({"path": "references", "reason": "linked"})]
                } else {
                    Vec::new()
                };
                json!({"kind": "x", "name": c.artifact.name, "path": c.artifact.path, "needs": needs})
            })
            .collect();
        self.claude(&json!({
            "items": [], "ignore": [], "parts": parts,
            "notNeeded": [{"path": "docs", "reason": "for people"}],
        }))?;
        Ok(self.library.preview_layout(&first, &|_| {})?)
    }

    pub(crate) fn id_of(preview: &Preview, name: &str) -> String {
        preview
            .candidates
            .iter()
            .find(|c| c.artifact.name == name)
            .map(|c| c.id.clone())
            .unwrap_or_default()
    }

    fn count(&self, table: &str) -> i64 {
        self.library
            .store
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or(-1)
    }
}

fn names(preview: &Preview) -> Vec<String> {
    let mut out: Vec<String> = preview
        .candidates
        .iter()
        .map(|c| format!("{:?}:{}", c.artifact.kind, c.artifact.name))
        .collect();
    out.sort();
    out
}

#[test]
fn every_skill_command_and_agent_is_offered_as_a_part_and_no_plugin() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let mut expected: Vec<String> = SKILLS.iter().map(|s| format!("Skill:{s}")).collect();
    expected.extend(
        ["Command:build", "Command:test", "Agent:code-reviewer"]
            .iter()
            .map(ToString::to_string),
    );
    expected.sort();
    assert_eq!(names(&preview), expected);
    assert!(preview.candidates.iter().all(|c| c.kept_whole.is_none()));
    Ok(())
}

#[test]
fn dropping_parts_imports_only_the_kept_ones_each_with_its_plugin_and_supporting_paths(
) -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let kept: Vec<String> = ["incremental-implementation", "planning", "build"]
        .iter()
        .map(|n| Lab::id_of(&preview, n))
        .collect();
    lab.library.import(&preview, &kept, &TagChoices::new())?;
    assert_eq!(lab.count("artifacts"), 3);

    let skill = lab.library.detail(&kept[0])?;
    let from = &skill.artifact.metadata["fromPlugin"];
    assert_eq!(from["name"], "agent-skills");
    assert_eq!(from["manifest"], ".claude-plugin/plugin.json");
    assert_eq!(from["needs"], json!(["references"]));
    assert!(!skill.reimport_to_update);
    // Source: the detail's provenance names where the plugin came from.
    assert_eq!(skill.provenance.source.kind, "local");
    // A part's own copy holds only its own files: no `_linked/` copies for plugin parts.
    assert_eq!(skill.files, ["SKILL.md"]);
    assert!(skill.artifact.linked_files.is_empty());
    let command = lab.library.detail(&kept[2])?;
    assert_eq!(command.artifact.metadata["fromPlugin"]["needs"], json!([]));

    // The plugin's record: its name, its manifest and the files its parts need.
    let source_id = skill.provenance.source.id.clone();
    let stored =
        plugin_record::load(&lab.library.store.conn, &source_id, "")?.ok_or("the plugin record")?;
    assert_eq!(stored.name, "agent-skills");
    assert!(stored.manifest.contains("\"version\":\"1.0.0\""));
    let files: Vec<&str> = stored.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["references/checklist.md", "references/security.md"]);
    // Items show up in the library list with the plugin's name.
    let page = lab.library.list(&Query::default())?;
    assert!(page
        .items
        .iter()
        .all(|i| i.from_plugin.as_deref() == Some("agent-skills") && !i.reimport_to_update));
    Ok(())
}

#[test]
fn supporting_files_two_parts_share_are_stored_once() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let skills: Vec<String> = SKILLS.iter().map(|n| Lab::id_of(&preview, n)).collect();
    lab.library.import(&preview, &skills, &TagChoices::new())?;
    assert_eq!(lab.count("plugin_records"), 1);
    assert_eq!(lab.count("plugin_files"), 2, "four skills, two files");
    // Importing another part later adds nothing for a file already stored.
    let build = Lab::id_of(&preview, "build");
    lab.library.import(&preview, &[build], &TagChoices::new())?;
    assert_eq!(lab.count("plugin_files"), 2);
    Ok(())
}

#[test]
fn removing_the_last_part_removes_the_plugin_record_and_its_files() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let one = Lab::id_of(&preview, "planning");
    let two = Lab::id_of(&preview, "code-review");
    lab.library
        .import(&preview, &[one.clone(), two.clone()], &TagChoices::new())?;
    lab.library.remove(&one)?;
    assert_eq!(
        (lab.count("plugin_records"), lab.count("plugin_files")),
        (1, 2)
    );
    lab.library.remove(&two)?;
    assert_eq!(
        (lab.count("plugin_records"), lab.count("plugin_files")),
        (0, 0)
    );
    Ok(())
}

#[test]
fn a_plugin_with_an_mcp_server_or_monitors_is_offered_whole_only_with_the_reason() -> TestResult {
    for (file, text, word) in [
        (
            ".mcp.json",
            r#"{"mcpServers":{"db":{"command":"db"}}}"#,
            "MCP servers",
        ),
        ("monitors/monitors.json", "[]", "monitors"),
    ] {
        let mut lab = Lab::new()?;
        write(&lab.repo, file, text)?;
        let preview = lab.library.preview_with(&lab.source(), &picking_parts())?;
        let plugin = preview
            .candidates
            .iter()
            .find(|c| c.artifact.kind == crate::artifact::ArtifactKind::Plugin)
            .ok_or("the plugin is offered whole")?;
        let reason = plugin.kept_whole_text.as_deref().ok_or("a reason")?;
        assert!(reason.contains(word), "{reason}");
        assert!(preview
            .candidates
            .iter()
            .all(|c| c.artifact.kind == crate::artifact::ArtifactKind::Plugin));
        // Taking it whole imports it as before, with no plugin record.
        let id = plugin.id.clone();
        lab.library.import(&preview, &[id], &TagChoices::new())?;
        assert_eq!(lab.count("plugin_records"), 0);
    }
    Ok(())
}

#[test]
fn a_part_with_no_plugin_record_is_marked_reimport_to_update() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let id = Lab::id_of(&preview, "planning");
    lab.library
        .import(&preview, std::slice::from_ref(&id), &TagChoices::new())?;
    // A library from before ADR-008: the part names its plugin but records no supporting paths.
    lab.library.store.conn.execute(
        "UPDATE artifacts SET artifact = json_remove(artifact, '$.metadata.fromPlugin.needs',
                '$.metadata.fromPlugin.manifest') WHERE id = ?1",
        rusqlite::params![id],
    )?;
    assert!(lab.library.detail(&id)?.reimport_to_update);
    let page = lab.library.list(&Query::default())?;
    assert!(page.items[0].reimport_to_update);
    Ok(())
}

#[test]
fn a_part_updates_with_its_plugin_record_kept() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let id = Lab::id_of(&preview, "planning");
    lab.library
        .import(&preview, std::slice::from_ref(&id), &TagChoices::new())?;
    write(
        &lab.repo,
        "skills/planning/SKILL.md",
        "---\nname: planning\ndescription: v2\n---\nSee ../../references/checklist.md\n",
    )?;
    write(&lab.repo, "references/checklist.md", "- changed\n")?;
    lab.library
        .check_updates(&crate::updates::CheckScope::All)?;
    let report = lab
        .library
        .update(&id, crate::updates::UpdateChoice::TakeUpstream)?;
    assert_eq!(report.outcome, crate::updates::UpdateOutcome::Updated);
    let detail = lab.library.detail(&id)?;
    assert_eq!(
        detail.artifact.metadata["fromPlugin"]["needs"],
        json!(["references"])
    );
    assert!(!detail.reimport_to_update);
    let source_id = detail.provenance.source.id.clone();
    let stored =
        plugin_record::load(&lab.library.store.conn, &source_id, "")?.ok_or("the plugin record")?;
    let checklist = stored
        .files
        .iter()
        .find(|f| f.path == "references/checklist.md")
        .ok_or("checklist")?;
    assert_eq!(checklist.bytes, b"- changed\n");
    Ok(())
}
