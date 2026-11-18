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

fn ask(root: &Path) -> Result<Option<WholeReason>, Box<dyn std::error::Error>> {
    let plugin = read(root)?;
    let options = ScanOptions::default();
    let mut walker = Walker::new(root, &options);
    Ok(kept_whole(&mut walker, &plugin))
}

fn manifest(root: &Path) -> std::io::Result<()> {
    write(root, ".claude-plugin/plugin.json", r#"{"name":"pack"}"#)
}

#[test]
fn any_plugin_can_be_split_hooks_scripts_config_and_namespaced_calls_included() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"pack","dependencies":["x"],"experimental":{"a":1},"strange":true}"#,
    )?;
    skill(
        dir.path(),
        "a",
        "Run ${CLAUDE_PLUGIN_ROOT}/x.sh and pack:b and ${user_config.token}",
    )?;
    skill(dir.path(), "b", "Body")?;
    write(dir.path(), "hooks/hooks.json", r#"{"hooks":{}}"#)?;
    write(dir.path(), "bin/tool", "#!/bin/sh")?;
    write(dir.path(), "settings.json", "{}")?;
    write(dir.path(), "themes/dark.json", "{}")?;
    write(
        dir.path(),
        "agents/x.md",
        "---\nname: x\ndescription: d\nhooks: {}\npermissionMode: plan\n---\nBody\n",
    )?;
    assert_eq!(ask(dir.path())?, None);
    Ok(())
}

#[test]
fn monitors_mcp_servers_and_lsp_servers_keep_a_plugin_whole() -> TestResult {
    let cases: [(&str, &str, PluginPart); 3] = [
        (
            ".mcp.json",
            r#"{"mcpServers":{"db":{"command":"x"}}}"#,
            PluginPart::McpServers,
        ),
        ("monitors/monitors.json", "[]", PluginPart::Monitors),
        (".lsp.json", "{}", PluginPart::LspServers),
    ];
    for (file, text, part) in cases {
        let dir = tempfile::tempdir()?;
        manifest(dir.path())?;
        skill(dir.path(), "a", "Body")?;
        write(dir.path(), file, text)?;
        assert_eq!(
            ask(dir.path())?,
            Some(WholeReason::WholeOnlyParts { parts: vec![part] }),
            "{file}"
        );
    }
    Ok(())
}

#[test]
fn manifest_declared_servers_and_monitors_keep_a_plugin_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"pack","mcpServers":{"db":{"command":"x"}},"lspServers":{"rust":{"command":"ra"}},"monitors":[]}"#,
    )?;
    skill(dir.path(), "a", "Body")?;
    assert_eq!(
        ask(dir.path())?,
        Some(WholeReason::WholeOnlyParts {
            parts: vec![
                PluginPart::McpServers,
                PluginPart::Monitors,
                PluginPart::LspServers
            ]
        })
    );
    Ok(())
}

#[test]
fn a_remote_only_marketplace_entry_stays_whole() -> TestResult {
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
        kept_whole(&mut walker, &plugin),
        Some(WholeReason::RemoteOnly)
    );
    Ok(())
}

#[test]
fn only_a_plugin_can_be_asked_whether_it_stays_whole() -> TestResult {
    let dir = tempfile::tempdir()?;
    manifest(dir.path())?;
    let mut plugin = read(dir.path())?;
    plugin.kind = ArtifactKind::Skill;
    let options = ScanOptions::default();
    let mut walker = Walker::new(dir.path(), &options);
    assert_eq!(kept_whole(&mut walker, &plugin), None);
    Ok(())
}

#[test]
fn reasons_read_as_sentences_and_cross_the_wire_tagged() -> TestResult {
    let whole = WholeReason::WholeOnlyParts {
        parts: vec![PluginPart::McpServers, PluginPart::Monitors],
    };
    assert!(whole
        .text()
        .starts_with("Has MCP servers and monitors, which a plugin scaffolded under .claude/skills/ does not support"));
    assert_eq!(
        serde_json::to_value(&whole)?,
        serde_json::json!({"type": "wholeOnlyParts", "parts": ["mcpServers", "monitors"]})
    );
    assert!(WholeReason::RemoteOnly.text().contains("another repo"));
    assert!(WholeReason::NoParts.text().contains("no skills"));
    assert_eq!(join(&[]), "");
    Ok(())
}

fn artifact(kind: ArtifactKind, path: &str) -> Artifact {
    Artifact {
        kind,
        name: "x".to_owned(),
        description: None,
        path: path.to_owned(),
        files: Vec::new(),
        metadata: serde_json::Map::new(),
        references: Vec::new(),
        diagnostics: Vec::new(),
        linked_files: Vec::new(),
        digest: None,
        origin: crate::artifact::Origin::Own,
    }
}

#[test]
fn a_hash_in_the_path_marks_a_remote_only_entry() {
    let remote = artifact(ArtifactKind::Plugin, "marketplace.json#tools");
    let local = artifact(ArtifactKind::Plugin, "plugins/tools");
    assert!(is_remote_only(&remote) && is_remote_path(&remote.path));
    assert!(!is_remote_only(&local));
    assert_eq!(remote_file(&remote.path), "marketplace.json");
    assert_eq!(remote_file(&local.path), "plugins/tools");
    assert_eq!(local_folder(&remote), None);
    assert_eq!(local_folder(&local), Some(Path::new("plugins/tools")));
    let skill = artifact(ArtifactKind::Skill, "skills/a");
    assert_eq!(local_folder(&skill), None);
}

#[test]
fn the_marketplace_and_manifest_come_from_the_metadata_and_path() {
    let mut p = artifact(ArtifactKind::Plugin, "plugins/tools");
    assert_eq!(marketplace_of(&p), None);
    assert!(!is_marketplace_entry(&p));
    assert_eq!(manifest_name(&p), "plugin.json");
    p.metadata.insert("marketplace".into(), "acme".into());
    assert_eq!(marketplace_of(&p), Some("acme"));
    assert!(is_marketplace_entry(&p));
    assert_eq!(manifest_name(&p), "marketplace.json");
    p.metadata
        .insert("marketplace".into(), serde_json::json!(3));
    assert_eq!(marketplace_of(&p), None);
    assert!(is_marketplace_entry(&p));
    let by_path = artifact(ArtifactKind::Plugin, "x/marketplace.json#a");
    assert_eq!(manifest_name(&by_path), "marketplace.json");
}

#[test]
fn namespaced_names_and_enabled_keys_are_read() {
    assert_eq!(namespaced_plugin("fmt:run"), Some("fmt"));
    assert_eq!(namespaced_plugin(":run"), None);
    assert_eq!(namespaced_plugin("run"), None);
    let settings = serde_json::json!({"enabledPlugins": {"a@m": true, "b@m": false}});
    let map = settings.as_object().cloned().unwrap_or_default();
    assert_eq!(enabled_keys(&map), vec!["a@m", "b@m"]);
    assert!(enabled_keys(&serde_json::Map::new()).is_empty());
}

#[test]
fn harnfit_refuses_only_plugins() {
    assert!(ensure_fits_harnfit(ArtifactKind::Skill).is_ok());
    assert!(matches!(
        ensure_fits_harnfit(ArtifactKind::Plugin),
        Err(crate::error::Error::HarnfitPluginUnsupported)
    ));
}

#[test]
fn a_plugin_with_user_config_stays_whole_because_a_skills_plugin_never_asks_for_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"pack","userConfig":{"token":{"description":"t"}}}"#,
    )?;
    skill(dir.path(), "a", "Body")?;
    assert_eq!(
        ask(dir.path())?,
        Some(WholeReason::WholeOnlyParts {
            parts: vec![PluginPart::UserConfig]
        })
    );
    Ok(())
}
