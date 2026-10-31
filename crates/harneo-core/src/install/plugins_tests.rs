//! Plugin installs against a fixture script standing in for `claude`. The real CLI never runs.

use std::fs;
use std::path::Path;

use super::plugins::*;
use crate::bundles::claude_cli_tests::{config, script};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const SETTINGS: &str = r#"{
  "enabledPlugins": {
    "agent-skills@addy-agent-skills": true,
    "skills@mattpocock": true,
    "brag@brag": true,
    "off@brag": false,
    "bad@brag": true
  },
  "extraKnownMarketplaces": {
    "addy-agent-skills": { "source": { "source": "github", "repo": "addyosmani/agent-skills" } },
    "mattpocock": { "source": { "source": "github", "repo": "mattpocock/skills" } },
    "brag": { "source": { "source": "github", "repo": "latent-spaces/brag" } }
  }
}"#;

/// A fake `claude`: answers the two listings from files, logs every other call with its working
/// folder, and fails `install bad@brag`.
fn fake(dir: &Path, markets: &str, installed: &str) -> std::io::Result<std::path::PathBuf> {
    let d = dir.display();
    fs::write(dir.join("markets.json"), markets)?;
    fs::write(dir.join("installed.json"), installed)?;
    script(
        dir,
        "claude",
        &format!(
            r#"case "$*" in
  "plugin marketplace list --json") cat '{d}/markets.json' ;;
  "plugin list --json") cat '{d}/installed.json' ;;
  "plugin install bad@brag --scope project --json")
    echo '{{"command":"install","outcome":"failed","message":"Plugin \"bad\" not found in marketplace \"brag\"."}}'
    exit 1 ;;
  *) echo "$* @ $(pwd)" >> '{d}/calls.txt'; echo '{{"outcome":"ok","message":"done"}}' ;;
esac"#
        ),
    )
}

fn project() -> std::io::Result<tempfile::TempDir> {
    let p = tempfile::tempdir()?;
    fs::create_dir(p.path().join(".claude"))?;
    fs::write(p.path().join(".claude/settings.json"), SETTINGS)?;
    Ok(p)
}

#[test]
fn reads_the_enabled_plugins_and_their_marketplace_sources() -> TestResult {
    let p = project()?;
    let found = targets(p.path())?;
    let ids: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|t| (t.id.as_str(), t.source.as_deref()))
        .collect();
    assert_eq!(
        ids,
        [
            (
                "agent-skills@addy-agent-skills",
                Some("addyosmani/agent-skills")
            ),
            ("bad@brag", Some("latent-spaces/brag")),
            ("brag@brag", Some("latent-spaces/brag")),
            ("skills@mattpocock", Some("mattpocock/skills")),
        ]
    );
    assert!(
        targets(tempfile::tempdir()?.path())?.is_empty(),
        "no settings, nothing to do"
    );
    Ok(())
}

#[test]
fn adds_missing_marketplaces_installs_for_the_project_and_reports_each_plugin() -> TestResult {
    let bin = tempfile::tempdir()?;
    let p = project()?;
    let here = p.path().canonicalize()?.display().to_string();
    // addy is known under its git URL (the same repo); brag is installed for the user already.
    let program = fake(
        bin.path(),
        r#"[{"name":"addy-agent-skills","source":"git","url":"https://github.com/addyosmani/agent-skills.git"},
            {"name":"brag","source":"github","repo":"latent-spaces/brag"}]"#,
        r#"[{"id":"brag@brag","scope":"user"},
            {"id":"skills@mattpocock","scope":"project","projectPath":"/somewhere/else"}]"#,
    )?;
    let settings_before = fs::read(p.path().join(".claude/settings.json"))?;
    let mut seen = Vec::new();
    let results = install_plugins(&config(&program), p.path(), |r| seen.push(r.id.clone()))?;

    let summary: Vec<(&str, PluginInstallStatus)> =
        results.iter().map(|r| (r.id.as_str(), r.status)).collect();
    assert_eq!(
        summary,
        [
            (
                "agent-skills@addy-agent-skills",
                PluginInstallStatus::Installed
            ),
            ("bad@brag", PluginInstallStatus::Failed),
            ("brag@brag", PluginInstallStatus::AlreadyInstalled),
            ("skills@mattpocock", PluginInstallStatus::Installed),
        ]
    );
    assert_eq!(seen.len(), 4, "progress reports every plugin");
    assert_eq!(
        results[1].message.as_deref(),
        Some(r#"Plugin "bad" not found in marketplace "brag"."#)
    );
    let calls = fs::read_to_string(bin.path().join("calls.txt"))?;
    let calls: Vec<&str> = calls.lines().collect();
    // Only the unknown marketplace is added; installs run in the project, scoped to it.
    assert!(calls[0].starts_with("plugin marketplace add mattpocock/skills --json @ "));
    assert_eq!(
        &calls[1..],
        [
            format!(
                "plugin install agent-skills@addy-agent-skills --scope project --json @ {here}"
            ),
            format!("plugin install skills@mattpocock --scope project --json @ {here}"),
        ]
    );
    assert_eq!(
        fs::read(p.path().join(".claude/settings.json"))?,
        settings_before,
        "Harneo never edits the settings it wrote"
    );
    Ok(())
}

#[test]
fn a_marketplace_name_taken_by_another_repo_fails_its_plugins_without_installing() -> TestResult {
    let bin = tempfile::tempdir()?;
    let p = project()?;
    let program = fake(
        bin.path(),
        r#"[{"name":"addy-agent-skills","source":"github","repo":"someone/else"},
            {"name":"brag","source":"github","repo":"latent-spaces/brag"},
            {"name":"mattpocock","source":"github","repo":"mattpocock/skills"}]"#,
        "[]",
    )?;
    let results = install_plugins(&config(&program), p.path(), |_| {})?;
    let addy = &results[0];
    assert_eq!(addy.status, PluginInstallStatus::Failed);
    assert!(addy
        .message
        .as_deref()
        .is_some_and(|m| m.contains("a different marketplace named \"addy-agent-skills\"")));
    let calls = fs::read_to_string(bin.path().join("calls.txt"))?;
    assert!(!calls.contains("agent-skills@"), "{calls}");
    Ok(())
}

#[test]
fn without_the_claude_cli_every_plugin_fails_with_the_reason() -> TestResult {
    let p = project()?;
    let missing = config(Path::new("/nonexistent/claude"));
    let results = install_plugins(&missing, p.path(), |_| {})?;
    assert_eq!(results.len(), 4);
    assert!(results
        .iter()
        .all(|r| r.status == PluginInstallStatus::Failed
            && r.message
                .as_deref()
                .is_some_and(|m| m.contains("claude CLI was not found"))));
    Ok(())
}
