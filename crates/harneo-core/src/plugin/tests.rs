use std::fs;
use std::path::Path;

use super::*;
use crate::artifact::walk::Walker;
use crate::artifact::{AgentAdapter, ClaudeAdapter, ScanOptions};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn skill(root: &Path, name: &str, body: &str) -> std::io::Result<()> {
    write(
        root,
        &format!("skills/{name}/SKILL.md"),
        &format!("---\nname: {name}\ndescription: The {name} skill\n---\n{body}\n"),
    )
}

/// The first Plugin the scan finds in `root`.
fn read(root: &Path) -> Result<Artifact, Box<dyn std::error::Error>> {
    let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
    report
        .artifacts
        .into_iter()
        .find(|a| a.kind == ArtifactKind::Plugin)
        .ok_or_else(|| "no plugin".into())
}

fn ask(root: &Path) -> Result<(PluginContents, Option<WholeReason>), Box<dyn std::error::Error>> {
    let plugin = read(root)?;
    let options = ScanOptions::default();
    let mut walker = Walker::new(root, &options);
    Ok((
        contents(&mut walker, &plugin),
        must_stay_whole(&mut walker, &plugin),
    ))
}

fn manifest(root: &Path) -> std::io::Result<()> {
    write(root, ".claude-plugin/plugin.json", r#"{"name":"pack"}"#)
}

#[test]
fn a_skills_only_pack_can_be_unpacked() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    skill(dir.path(), "a", "Body")?;
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers":{"db":{"command":"x"}}}"#,
    )?;
    write(
        dir.path(),
        "commands/go.md",
        "---\ndescription: Go\n---\nGo\n",
    )?;
    let (held, whole) = ask(dir.path())?;
    assert_eq!(
        held.pickable,
        [
            PluginPart::Skills,
            PluginPart::Commands,
            PluginPart::McpServers
        ]
    );
    assert!(held.whole_only.is_empty() && held.root_ties.is_empty());
    assert_eq!(whole, None);
    Ok(())
}

#[test]
fn a_hooks_only_plugin_must_stay_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    write(dir.path(), "hooks/hooks.json", r#"{"hooks":{}}"#)?;
    let (held, whole) = ask(dir.path())?;
    assert!(held.pickable.is_empty());
    assert_eq!(held.whole_only, [PluginPart::Hooks]);
    assert_eq!(
        whole,
        Some(WholeReason::WholeOnlyParts {
            parts: vec![PluginPart::Hooks]
        })
    );
    Ok(())
}

#[test]
fn a_mixed_plugin_lists_both_kinds_and_every_whole_only_part() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"pack","lspServers":{"rust":{"command":"ra"}}}"#,
    )?;
    skill(dir.path(), "a", "Body")?;
    write(dir.path(), "bin/tool", "#!/bin/sh")?;
    write(
        dir.path(),
        "hooks/hooks.json",
        r#"{"modules":["./register.tsx"]}"#,
    )?;
    write(dir.path(), "settings.json", "{}")?;
    write(dir.path(), "themes/dark.json", "{}")?;
    write(dir.path(), "workflows/w.md", "w")?;
    write(dir.path(), "monitors/monitors.json", "[]")?;
    let (held, whole) = ask(dir.path())?;
    assert_eq!(held.pickable, [PluginPart::Skills]);
    let all = vec![
        PluginPart::Monitors,
        PluginPart::LspServers,
        PluginPart::Mods,
        PluginPart::Bin,
        PluginPart::Themes,
        PluginPart::Workflows,
        PluginPart::Settings,
    ];
    assert_eq!(held.whole_only, all);
    assert_eq!(whole, Some(WholeReason::WholeOnlyParts { parts: all }));
    Ok(())
}

#[test]
fn a_part_tied_to_the_plugin_root_keeps_the_plugin_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    skill(dir.path(), "ok", "Body")?;
    skill(
        dir.path(),
        "needy",
        "Run ${CLAUDE_PLUGIN_ROOT}/shared/tool.sh",
    )?;
    write(dir.path(), "shared/tool.sh", "echo")?;
    let (held, whole) = ask(dir.path())?;
    assert_eq!(held.root_ties, [PluginPart::Skills]);
    assert_eq!(
        whole,
        Some(WholeReason::PluginRootTie {
            parts: vec![PluginPart::Skills]
        })
    );
    Ok(())
}

#[test]
fn a_remote_only_entry_must_stay_whole_and_holds_nothing_visible() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/marketplace.json",
        r#"{"name":"m","plugins":[{"name":"far","source":{"source":"github","repo":"o/r"}}]}"#,
    )?;
    let plugin = read(dir.path())?;
    let options = ScanOptions::default();
    let mut walker = Walker::new(dir.path(), &options);
    assert_eq!(
        must_stay_whole(&mut walker, &plugin),
        Some(WholeReason::RemoteOnly)
    );
    assert_eq!(contents(&mut walker, &plugin), PluginContents::default());
    Ok(())
}

#[test]
fn only_a_plugin_has_contents() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    let mut plugin = read(dir.path())?;
    plugin.kind = ArtifactKind::Skill;
    let options = ScanOptions::default();
    let mut walker = Walker::new(dir.path(), &options);
    assert_eq!(contents(&mut walker, &plugin), PluginContents::default());
    assert_eq!(must_stay_whole(&mut walker, &plugin), None);
    Ok(())
}

#[test]
fn reasons_read_as_sentences_and_cross_the_wire_tagged() -> TestResult {
    let whole = WholeReason::WholeOnlyParts {
        parts: vec![PluginPart::Hooks, PluginPart::Mods],
    };
    assert_eq!(
        whole.text(),
        "Has hooks and mods: they only work inside the plugin."
    );
    assert_eq!(
        serde_json::to_value(&whole)?,
        serde_json::json!({"type": "wholeOnlyParts", "parts": ["hooks", "mods"]})
    );
    let many = WholeReason::PluginRootTie {
        parts: vec![PluginPart::Skills, PluginPart::Agents, PluginPart::Commands],
    };
    assert!(many
        .text()
        .starts_with("Its skills, agents and commands refer"));
    let one = WholeReason::PluginRootTie {
        parts: vec![PluginPart::Skills],
    };
    assert!(one.text().starts_with("Its skills refer"));
    assert!(WholeReason::RemoteOnly.text().contains("another repo"));
    assert_eq!(join(&[]), "");
    Ok(())
}

/// A mod is a hooks module (`hooks/hooks.json` → `modules`); with ordinary hooks beside it, the
/// plugin holds both.
#[test]
fn hooks_json_with_modules_and_hooks_holds_mods_and_hooks() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    skill(dir.path(), "a", "Body")?;
    write(
        dir.path(),
        "hooks/hooks.json",
        r#"{"hooks":{"Stop":[]},"modules":["./register.tsx"]}"#,
    )?;
    let (held, whole) = ask(dir.path())?;
    assert_eq!(held.whole_only, [PluginPart::Hooks, PluginPart::Mods]);
    assert_eq!(
        whole,
        Some(WholeReason::WholeOnlyParts {
            parts: vec![PluginPart::Hooks, PluginPart::Mods]
        })
    );
    Ok(())
}
