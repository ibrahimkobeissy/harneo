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

/// A fake `claude` whose `plugin install` saves the project's settings again: `reorder` keeps the
/// same settings with the keys in another order, otherwise it adds a key.
fn rewriting(dir: &Path, reorder: bool) -> std::io::Result<std::path::PathBuf> {
    let new = if reorder {
        r#"{"extraKnownMarketplaces":{"brag":{"source":{"repo":"latent-spaces/brag","source":"github"}}},"enabledPlugins":{"brag@brag":true}}"#
    } else {
        r#"{"enabledPlugins":{"brag@brag":true},"extraKnownMarketplaces":{"brag":{"source":{"repo":"latent-spaces/brag","source":"github"}}},"model":"opus"}"#
    };
    fs::write(dir.join("new.json"), new)?;
    let d = dir.display();
    script(
        dir,
        "claude",
        &format!(
            r#"case "$*" in
  "plugin marketplace list --json") echo '[]' ;;
  "plugin list --json") echo '[]' ;;
  "plugin install"*) cp '{d}/new.json' .claude/settings.json; echo '{{"outcome":"ok"}}' ;;
  *) echo '{{"outcome":"ok"}}' ;;
esac"#
        ),
    )
}

const BRAG_ONLY: &str = r#"{
  "enabledPlugins": {
    "brag@brag": true
  },
  "extraKnownMarketplaces": {
    "brag": { "source": { "source": "github", "repo": "latent-spaces/brag" } }
  }
}
"#;

#[test]
fn settings_the_cli_saves_again_unchanged_keep_the_bytes_harneo_wrote() -> TestResult {
    let bin = tempfile::tempdir()?;
    let cli = rewriting(bin.path(), true)?;
    let p = tempfile::tempdir()?;
    fs::create_dir(p.path().join(".claude"))?;
    fs::write(p.path().join(".claude/settings.json"), BRAG_ONLY)?;
    let results = install_plugins(&config(&cli), p.path(), |_| {})?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, PluginInstallStatus::Installed);
    // Same settings in another order: Harneo's bytes are put back, so nothing shows as drift.
    assert_eq!(
        fs::read_to_string(p.path().join(".claude/settings.json"))?,
        BRAG_ONLY
    );
    Ok(())
}

#[test]
fn settings_the_cli_really_changed_are_left_as_it_saved_them() -> TestResult {
    let bin = tempfile::tempdir()?;
    let cli = rewriting(bin.path(), false)?;
    let p = tempfile::tempdir()?;
    fs::create_dir(p.path().join(".claude"))?;
    fs::write(p.path().join(".claude/settings.json"), BRAG_ONLY)?;
    install_plugins(&config(&cli), p.path(), |_| {})?;
    let now = fs::read_to_string(p.path().join(".claude/settings.json"))?;
    assert!(now.contains(r#""model":"opus""#), "a real change stays");
    Ok(())
}

/// Settings that enable only `skills@mattpocock`: `agent-skills` was dropped, `off` is disabled.
const AFTER_REFIT: &str = r#"{
  "enabledPlugins": { "skills@mattpocock": true, "off@brag": false }
}"#;

fn refitted() -> std::io::Result<tempfile::TempDir> {
    let p = tempfile::tempdir()?;
    fs::create_dir(p.path().join(".claude"))?;
    fs::write(p.path().join(".claude/settings.json"), AFTER_REFIT)?;
    Ok(p)
}

#[test]
fn a_plugin_the_refit_dropped_is_uninstalled_for_the_project_and_nothing_else_is() -> TestResult {
    let bin = tempfile::tempdir()?;
    let p = refitted()?;
    let here = p.path().canonicalize()?.display().to_string();
    let program = fake(
        bin.path(),
        "[]",
        &format!(
            r#"[{{"id":"agent-skills@addy-agent-skills","scope":"project","projectPath":"{here}"}},
               {{"id":"off@brag","scope":"project","projectPath":"{here}"}},
               {{"id":"mine@elsewhere","scope":"project","projectPath":"{here}"}},
               {{"id":"skills@mattpocock","scope":"project","projectPath":"{here}"}}]"#
        ),
    )?;
    // Harneo enabled agent-skills, off and skills; `mine` was never Harneo's.
    let previously = vec![
        "agent-skills@addy-agent-skills".to_owned(),
        "gone@nowhere".to_owned(),
        "off@brag".to_owned(),
        "skills@mattpocock".to_owned(),
    ];
    let mut seen = 0;
    let (results, gone) = uninstall_dropped(&config(&program), p.path(), &previously, |_| {
        seen += 1;
    })?;
    let summary: Vec<(&str, PluginInstallStatus)> =
        results.iter().map(|r| (r.id.as_str(), r.status)).collect();
    assert_eq!(
        summary,
        [
            (
                "agent-skills@addy-agent-skills",
                PluginInstallStatus::Uninstalled
            ),
            ("off@brag", PluginInstallStatus::Uninstalled),
        ]
    );
    assert_eq!(seen, 2);
    // A plugin that is no longer installed is forgotten without a report.
    assert_eq!(
        gone,
        ["agent-skills@addy-agent-skills", "gone@nowhere", "off@brag"]
    );
    let calls = fs::read_to_string(bin.path().join("calls.txt"))?;
    assert_eq!(
        calls.lines().collect::<Vec<_>>(),
        [
            format!(
                "plugin uninstall agent-skills@addy-agent-skills --scope project --json @ {here}"
            ),
            format!("plugin uninstall off@brag --scope project --json @ {here}"),
        ]
    );
    Ok(())
}

#[test]
fn nothing_dropped_means_the_cli_is_not_run() -> TestResult {
    let bin = tempfile::tempdir()?;
    let p = refitted()?;
    let program = fake(bin.path(), "[]", "[]")?;
    let (results, gone) = uninstall_dropped(
        &config(&program),
        p.path(),
        &["skills@mattpocock".to_owned()],
        |_| {},
    )?;
    assert!(results.is_empty() && gone.is_empty());
    assert!(!bin.path().join("calls.txt").exists());
    // No settings at all: every plugin of the earlier write is dropped, none installed.
    let empty = tempfile::tempdir()?;
    let (results, gone) =
        uninstall_dropped(&config(&program), empty.path(), &["x@y".to_owned()], |_| {})?;
    assert!(results.is_empty());
    assert_eq!(gone, ["x@y"]);
    Ok(())
}

#[test]
fn an_uninstall_that_fails_is_reported_and_kept_for_next_time() -> TestResult {
    let bin = tempfile::tempdir()?;
    let p = refitted()?;
    let here = p.path().canonicalize()?.display().to_string();
    let program = script(
        bin.path(),
        "claude",
        &format!(
            r#"case "$*" in
  "plugin list --json") echo '[{{"id":"a@m","scope":"project","projectPath":"{here}"}}]' ;;
  *) echo '{{"command":"uninstall","outcome":"failed","message":"locked"}}'; exit 1 ;;
esac"#
        ),
    )?;
    let (results, gone) =
        uninstall_dropped(&config(&program), p.path(), &["a@m".to_owned()], |_| {})?;
    assert_eq!(results[0].status, PluginInstallStatus::NotUninstalled);
    assert_eq!(results[0].message.as_deref(), Some("locked"));
    assert!(gone.is_empty());

    // Without the lists the reason is the same for every dropped plugin.
    let broken = script(bin.path(), "claude2", "echo nope >&2; exit 1")?;
    let (results, _) = uninstall_dropped(
        &config(&broken),
        p.path(),
        &["a@m".to_owned(), "b@m".to_owned()],
        |_| {},
    )?;
    assert!(results
        .iter()
        .all(|r| r.status == PluginInstallStatus::NotUninstalled));
    assert_eq!(results.len(), 2);
    Ok(())
}
