//! Scanning with `ScanOptions.unpack`: a plugin's parts instead of the plugin.

use std::fs;
use std::path::Path;

use harneo_core::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions, ScanReport};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

/// A plugin at `base` (`""` for the repo root) with one part of every kind, and hooks.
fn plugin(root: &Path, base: &str, manifest: &str) -> std::io::Result<()> {
    let at = |rel: &str| {
        if base.is_empty() {
            rel.to_owned()
        } else {
            format!("{base}/{rel}")
        }
    };
    write(root, &at(".claude-plugin/plugin.json"), manifest)?;
    write(
        root,
        &at("skills/lint/SKILL.md"),
        "---\nname: lint\ndescription: Lint\n---\nRun ${CLAUDE_PLUGIN_ROOT}/skills/lint/run.sh",
    )?;
    write(root, &at("skills/lint/run.sh"), "echo lint")?;
    write(
        root,
        &at("agents/fixer.md"),
        "---\nname: fixer\ndescription: Fixes\n---\nBody",
    )?;
    write(
        root,
        &at("commands/ship.md"),
        "---\ndescription: Ship\n---\nShip it",
    )?;
    write(
        root,
        &at("output-styles/terse.md"),
        "---\nname: terse\ndescription: Short\n---\nBe brief",
    )?;
    write(
        root,
        &at(".mcp.json"),
        r#"{"mcpServers":{"db":{"command":"db-server","args":["--ro"]}}}"#,
    )?;
    write(
        root,
        &at("hooks/hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"${CLAUDE_PLUGIN_ROOT}/hooks/check.sh"}]}]}}"#,
    )?;
    write(root, &at("hooks/check.sh"), "exit 0")
}

fn scan(root: &Path, unpack: &[&str]) -> Result<ScanReport, harneo_core::Error> {
    let options = ScanOptions {
        unpack: unpack.iter().map(|s| (*s).to_owned()).collect(),
        ..ScanOptions::default()
    };
    ClaudeAdapter.scan(root, &options)
}

fn kinds(report: &ScanReport) -> Vec<(ArtifactKind, &str)> {
    let mut v: Vec<_> = report
        .artifacts
        .iter()
        .map(|a| (a.kind, a.name.as_str()))
        .collect();
    v.sort();
    v
}

const MANIFEST: &str = r#"{"name":"toolbox","description":"Tools"}"#;

#[test]
fn unpacking_reports_the_five_parts_and_no_plugin_or_hook() -> TestResult {
    let dir = tempfile::tempdir()?;
    plugin(dir.path(), "toolbox", MANIFEST)?;
    let report = scan(dir.path(), &["toolbox"])?;
    let mut expected = vec![
        (ArtifactKind::Skill, "lint"),
        (ArtifactKind::Command, "ship"),
        (ArtifactKind::Agent, "fixer"),
        (ArtifactKind::Mcp, "db"),
        (ArtifactKind::OutputStyle, "terse"),
    ];
    expected.sort();
    assert_eq!(kinds(&report), expected);
    for a in &report.artifacts {
        assert_eq!(a.metadata["layoutBy"], "plugin.json");
        assert_eq!(a.metadata["fromPlugin"]["name"], "toolbox");
        assert_eq!(a.metadata["fromPlugin"]["folder"], "toolbox");
        assert!(a.metadata["fromPlugin"]["marketplace"].is_null());
        assert!(a.digest.is_some());
        assert!(a.diagnostics.is_empty(), "{}: {:?}", a.name, a.diagnostics);
    }
    let by = |name: &str| report.artifacts.iter().find(|a| a.name == name);
    let skill = by("lint").ok_or("no skill")?;
    assert_eq!(
        skill.files,
        ["toolbox/skills/lint/SKILL.md", "toolbox/skills/lint/run.sh"]
    );
    assert_eq!(skill.path, "toolbox/skills/lint");
    assert_eq!(
        by("fixer").ok_or("no agent")?.files,
        ["toolbox/agents/fixer.md"]
    );
    assert_eq!(
        by("ship").ok_or("no command")?.path,
        "toolbox/commands/ship.md"
    );
    let server = by("db").ok_or("no server")?;
    assert_eq!(server.path, "toolbox/.mcp.json#db");
    assert_eq!(server.files, ["toolbox/.mcp.json"]);
    assert_eq!(server.metadata["command"], "db-server");
    assert!(report.unplaced.is_empty());
    Ok(())
}

#[test]
fn without_unpack_the_plugin_is_reported_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    plugin(dir.path(), "toolbox", MANIFEST)?;
    let report = scan(dir.path(), &[])?;
    assert_eq!(kinds(&report), [(ArtifactKind::Plugin, "toolbox")]);
    let other = scan(dir.path(), &["somewhere-else"])?;
    assert_eq!(other, report);
    Ok(())
}

#[test]
fn a_part_reaching_outside_itself_needs_the_plugin() -> TestResult {
    let dir = tempfile::tempdir()?;
    plugin(dir.path(), "toolbox", MANIFEST)?;
    write(
        dir.path(),
        "toolbox/skills/shared-user/SKILL.md",
        "---\nname: shared-user\ndescription: d\n---\nsh ${CLAUDE_PLUGIN_ROOT}/../shared/x.sh and ${CLAUDE_PLUGIN_ROOT}",
    )?;
    write(
        dir.path(),
        "toolbox/agents/helper.md",
        "---\nname: helper\ndescription: d\n---\nuse ${CLAUDE_PLUGIN_ROOT}/lib/tool.sh",
    )?;
    write(
        dir.path(),
        "toolbox/.claude-plugin/plugin.json",
        r#"{"name":"toolbox","mcpServers":{"inline":{"command":"${CLAUDE_PLUGIN_ROOT}/bin/srv"},"plain":{"url":"https://x.test"}}}"#,
    )?;
    let report = scan(dir.path(), &["toolbox"])?;
    let diag = |name: &str| -> Vec<(String, String)> {
        report
            .artifacts
            .iter()
            .filter(|a| a.name == name)
            .flat_map(|a| a.diagnostics.iter())
            .filter(|d| d.code == "plugin.part-needs-plugin")
            .map(|d| (format!("{:?}", d.severity), d.message.clone()))
            .collect()
    };
    let skill = diag("shared-user");
    assert_eq!(skill.len(), 2, "{skill:?}");
    assert!(skill.iter().all(|(sev, _)| sev == "Error"));
    assert!(skill
        .iter()
        .any(|(_, m)| m.contains("${CLAUDE_PLUGIN_ROOT}/../shared/x.sh")));
    assert!(diag("lint").is_empty());
    assert_eq!(diag("helper").len(), 1);
    assert_eq!(diag("inline").len(), 1);
    assert!(diag("plain").is_empty());
    let inline = report
        .artifacts
        .iter()
        .find(|a| a.name == "inline")
        .ok_or("no inline server")?;
    assert_eq!(inline.path, "toolbox/.claude-plugin/plugin.json#inline");
    assert!(report.artifacts.iter().any(|a| a.name == "shared-user"));
    Ok(())
}

#[test]
fn a_plugin_at_the_repo_root_unpacks() -> TestResult {
    let dir = tempfile::tempdir()?;
    plugin(dir.path(), "", MANIFEST)?;
    for spelling in ["", ".", "./"] {
        let report = scan(dir.path(), &[spelling])?;
        assert_eq!(report.artifacts.len(), 5, "{spelling:?}");
        assert!(report
            .artifacts
            .iter()
            .all(|a| a.kind != ArtifactKind::Plugin));
        assert!(report
            .artifacts
            .iter()
            .any(|a| a.path == ".mcp.json#db" && a.metadata["fromPlugin"]["folder"] == ""));
    }
    Ok(())
}

#[test]
fn a_marketplace_entry_with_a_local_source_unpacks_and_a_remote_one_stays_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(
        r,
        ".claude-plugin/marketplace.json",
        r#"{"name":"team","plugins":[
          {"name":"toolbox","source":"./plugins/toolbox"},
          {"name":"far","source":{"source":"github","repo":"example-org/far"}}]}"#,
    )?;
    plugin(r, "plugins/toolbox", MANIFEST)?;
    let report = scan(
        r,
        &["plugins/toolbox", ".claude-plugin/marketplace.json#far"],
    )?;
    let plugins: Vec<_> = report
        .artifacts
        .iter()
        .filter(|a| a.kind == ArtifactKind::Plugin)
        .collect();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "far");
    assert!(plugins[0]
        .diagnostics
        .iter()
        .any(|d| d.code == "plugin.unpack-remote-only"));
    let parts: Vec<_> = report
        .artifacts
        .iter()
        .filter(|a| a.kind != ArtifactKind::Plugin)
        .collect();
    assert_eq!(parts.len(), 5);
    assert!(parts
        .iter()
        .all(|a| a.metadata["fromPlugin"]["marketplace"] == "team"));
    Ok(())
}

#[test]
fn declared_paths_and_unpickable_components_are_honoured() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(
        r,
        "p/.claude-plugin/plugin.json",
        r#"{"name":"p","agents":["./team/a.md"],"commands":{"go":{"source":"./cmds/go.md"}},
            "skills":"./extra","mcpServers":"./servers.json","workflows":"./wf"}"#,
    )?;
    write(r, "p/team/a.md", "---\nname: a\ndescription: d\n---\n")?;
    write(
        r,
        "p/agents/ignored.md",
        "---\nname: ignored\ndescription: d\n---\n",
    )?;
    write(r, "p/cmds/go.md", "---\ndescription: d\n---\n")?;
    write(
        r,
        "p/extra/one/SKILL.md",
        "---\nname: one\ndescription: d\n---\n",
    )?;
    write(
        r,
        "p/servers.json",
        r#"{"s1":{"command":"x"},"s2":"not a server"}"#,
    )?;
    write(r, "p/wf/flow.md", "---\nname: flow\n---\n")?;
    write(r, "p/settings.json", "{}")?;
    write(r, "p/bin/tool", "x")?;
    let report = scan(r, &["p"])?;
    let mut found: Vec<_> = report.artifacts.iter().map(|a| a.path.as_str()).collect();
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "p/cmds/go.md",
            "p/extra/one",
            "p/servers.json#s1",
            "p/team/a.md"
        ]
    );
    Ok(())
}

#[test]
fn an_unpacked_plugin_claims_its_folder_however_it_is_spelled_and_nothing_else() -> TestResult {
    let dir = tempfile::tempdir()?;
    plugin(dir.path(), "plugins/tools", r#"{"name":"tools"}"#)?;
    write(
        dir.path(),
        "notes/prompt.md",
        "---\nname: loose\ndescription: A prompt nobody placed\n---\nBody",
    )?;

    let report = scan(dir.path(), &["./plugins/tools/"])?;
    assert!(report
        .artifacts
        .iter()
        .all(|a| a.kind != ArtifactKind::Plugin));
    // The plugin's hooks are not parts, and not unplaced either.
    assert!(
        report
            .unplaced
            .iter()
            .all(|p| !p.starts_with("plugins/tools")),
        "{:?}",
        report.unplaced
    );
    assert!(report.unplaced.iter().any(|p| p == "notes/prompt.md"));

    // An entry naming no plugin unpacks nothing and hides nothing.
    let other = scan(dir.path(), &["notes"])?;
    assert!(other.unplaced.iter().any(|p| p == "notes/prompt.md"));
    assert!(other
        .artifacts
        .iter()
        .any(|a| a.kind == ArtifactKind::Plugin));
    Ok(())
}

#[test]
fn a_plugin_with_nothing_to_pick_stays_whole_with_a_note() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"hooksonly"}"#,
    )?;
    write(
        dir.path(),
        "hooks/hooks.json",
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"true"}]}]}}"#,
    )?;
    let report = scan(dir.path(), &[""])?;
    let plugin = report
        .artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::Plugin)
        .ok_or("the plugin is still listed")?;
    assert!(plugin
        .diagnostics
        .iter()
        .any(|d| d.code == "plugin.no-parts"));
    Ok(())
}
