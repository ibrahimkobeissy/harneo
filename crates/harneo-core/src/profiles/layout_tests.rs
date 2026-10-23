use std::path::Path;

use super::tests::{write, TestResult};
use super::*;
use crate::library::{Source, TagChoices};

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
}

fn import(root: &Path) -> std::result::Result<Lib, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: root.to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(Lib {
        _data: data,
        _src: tempfile::tempdir()?,
        library,
    })
}

/// One artifact of each kind, in a project-shaped folder.
fn project() -> std::result::Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    let r = src.path();
    write(
        r,
        ".claude/skills/s/SKILL.md",
        "---\nname: s\ndescription: d\n---\nB\n",
    )?;
    write(r, ".claude/skills/s/scripts/run.sh", "echo hi\n")?;
    write(
        r,
        ".claude/agents/team/rev.md",
        "---\nname: rev\ndescription: d\n---\nB\n",
    )?;
    write(r, ".claude/rules/api/style.md", "Be terse.\n")?;
    write(
        r,
        ".claude/commands/go.md",
        "---\ndescription: d\n---\nGo\n",
    )?;
    write(
        r,
        ".claude/output-styles/terse.md",
        "---\nname: terse\ndescription: d\n---\nB\n",
    )?;
    write(r, "CLAUDE.md", "# Project\n")?;
    write(
        r,
        ".claude/settings.json",
        "{\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"bash scripts/hook.sh\"}]}]}}\n",
    )?;
    write(r, "scripts/hook.sh", "exit 0\n")?;
    write(
        r,
        ".mcp.json",
        "{\"mcpServers\":{\"api\":{\"command\":\"npx\",\"args\":[\"x\"]}}}\n",
    )?;
    Ok(src)
}

fn paths(layout: &Layout) -> Vec<&str> {
    layout.paths.iter().map(|p| p.path.as_str()).collect()
}

#[test]
fn every_kind_lands_where_the_adapter_installs_it() -> TestResult {
    let src = project()?;
    let lib = import(src.path())?;
    let list = lib.library.list(&crate::library::Query {
        filter: crate::library::Filter::default(),
        sort: crate::library::SortKey::Name,
        descending: false,
        offset: 0,
        limit: 100,
    })?;
    let ids = |kinds: &[ArtifactKind]| -> Vec<String> {
        list.items
            .iter()
            .filter(|i| kinds.contains(&i.kind))
            .map(|i| i.id.clone())
            .collect()
    };
    let items = ids(&[
        ArtifactKind::Skill,
        ArtifactKind::Agent,
        ArtifactKind::Rule,
        ArtifactKind::Command,
        ArtifactKind::OutputStyle,
        ArtifactKind::ClaudeMd,
    ]);
    assert_eq!(items.len(), 6);
    let settings = lib.id(ArtifactKind::Settings, "settings")?;
    let api = lib.id(ArtifactKind::Mcp, "api")?;
    let spec = ProfileSpec {
        items,
        settings: Some(settings.clone()),
        mcp: vec![McpChoice {
            artifact: api,
            env: BTreeMap::new(),
        }],
        manual_only: BTreeMap::new(),
    };
    let layout = lib.library.layout(&spec)?;
    assert_eq!(
        paths(&layout),
        [
            ".claude/agents/team/rev.md",
            ".claude/commands/go.md",
            ".claude/output-styles/terse.md",
            ".claude/rules/api/style.md",
            ".claude/settings.json",
            ".claude/skills/s/SKILL.md",
            ".claude/skills/s/scripts/run.sh",
            ".mcp.json",
            "CLAUDE.md",
            "HARNESS.md",
            "scripts/hook.sh",
        ]
    );
    assert!(layout.conflicts.is_empty());
    let from = |path: &str| {
        layout
            .paths
            .iter()
            .find(|p| p.path == path)
            .map(|p| p.from.clone())
    };
    assert_eq!(
        from(".claude/settings.json"),
        Some(PathFrom::Artifact {
            artifact: settings.clone()
        })
    );
    assert_eq!(
        from("scripts/hook.sh"),
        Some(PathFrom::Artifact { artifact: settings })
    );
    assert_eq!(
        from(".mcp.json"),
        Some(PathFrom::Generated {
            generated: Generated::McpJson
        })
    );
    assert_eq!(
        from("HARNESS.md"),
        Some(PathFrom::Generated {
            generated: Generated::HarnessMd
        })
    );
    Ok(())
}

#[test]
fn claude_md_only_when_the_profile_has_one_and_generated_files_follow_the_content() -> TestResult {
    let src = project()?;
    let lib = import(src.path())?;
    let skill = lib.id(ArtifactKind::Skill, "s")?;
    let layout = lib.library.layout(&ProfileSpec {
        items: vec![skill],
        ..ProfileSpec::default()
    })?;
    assert_eq!(
        paths(&layout),
        [
            ".claude/skills/s/SKILL.md",
            ".claude/skills/s/scripts/run.sh",
            "HARNESS.md"
        ]
    );
    Ok(())
}

#[test]
fn a_plugin_alone_produces_only_the_settings_key_and_a_template_keeps_its_own_file() -> TestResult {
    let src = tempfile::tempdir()?;
    write(
        src.path(),
        "plugins/p/.claude-plugin/plugin.json",
        "{\"name\":\"p\",\"description\":\"d\"}\n",
    )?;
    write(
        src.path(),
        "plugins/p/skills/x/SKILL.md",
        "---\nname: x\ndescription: d\n---\n",
    )?;
    write(src.path(), "settings.json", "{}\n")?;
    let lib = import(src.path())?;
    let plugin = lib.id(ArtifactKind::Plugin, "p")?;
    let alone = lib.library.layout(&ProfileSpec {
        items: vec![plugin.clone()],
        ..ProfileSpec::default()
    })?;
    assert_eq!(paths(&alone), [".claude/settings.json", "HARNESS.md"]);
    assert_eq!(
        alone.paths[0].from,
        PathFrom::Generated {
            generated: Generated::Settings
        }
    );
    let settings = lib.id(ArtifactKind::Settings, "settings")?;
    let with = lib.library.layout(&ProfileSpec {
        items: vec![plugin],
        settings: Some(settings.clone()),
        ..ProfileSpec::default()
    })?;
    assert_eq!(paths(&with), [".claude/settings.json", "HARNESS.md"]);
    assert_eq!(
        with.paths[0].from,
        PathFrom::Artifact { artifact: settings }
    );
    Ok(())
}

#[test]
fn two_artifacts_claiming_one_path_are_a_conflict() -> TestResult {
    let src = tempfile::tempdir()?;
    for (dir, body) in [("skills/x", "One."), (".claude/skills/x", "Two.")] {
        write(
            src.path(),
            &format!("{dir}/SKILL.md"),
            &format!("---\nname: x\ndescription: d\n---\n{body}\n"),
        )?;
    }
    let lib = import(src.path())?;
    let list = lib.library.list(&crate::library::Query {
        filter: crate::library::Filter::default(),
        sort: crate::library::SortKey::Name,
        descending: false,
        offset: 0,
        limit: 10,
    })?;
    let ids: Vec<String> = list.items.iter().map(|i| i.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    let layout = lib.library.layout(&ProfileSpec {
        items: ids.clone(),
        ..ProfileSpec::default()
    })?;
    assert_eq!(layout.conflicts.len(), 1);
    assert_eq!(layout.conflicts[0].path, ".claude/skills/x/SKILL.md");
    let mut claimants = layout.conflicts[0].artifacts.clone();
    claimants.sort();
    let mut expected = ids;
    expected.sort();
    assert_eq!(claimants, expected);
    assert_eq!(paths(&layout), [".claude/skills/x/SKILL.md", "HARNESS.md"]);
    Ok(())
}
