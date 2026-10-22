//! A `~/.claude`-like root with `plugins/installed_plugins.json`: plugins come from that list
//! only, each with its marketplace origin; catalogue clones and the cache are never scanned.

use std::path::Path;

use harneo_core::artifact::{
    AgentAdapter, ArtifactKind, ClaudeAdapter, Origin, ScanOptions, ScanReport,
};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

/// 2 installed plugins (one with two scopes), a 50-entry catalogue clone and cache folders.
fn fixture(root: &Path, outside: &str) -> std::io::Result<()> {
    let base = root.display();
    let cache = |m: &str, p: &str| format!("{base}/plugins/cache/{m}/{p}/1.0.0");
    let installed = json!({"version": 2, "plugins": {
        "alpha@claude-plugins-official": [
            {"scope": "user", "installPath": cache("claude-plugins-official", "alpha"), "version": "1.0.0"},
            {"scope": "project", "projectPath": "/work/app", "installPath": cache("claude-plugins-official", "alpha"), "version": "1.0.0"}
        ],
        "beta@community": [
            {"scope": "user", "installPath": cache("community", "beta"), "version": "2.0.0"},
            {"scope": "user", "installPath": outside}
        ]
    }});
    write(
        root,
        "plugins/installed_plugins.json",
        &installed.to_string(),
    )?;
    let known = json!({
        "claude-plugins-official": {"source": {"source": "github", "repo": "anthropics/claude-plugins-official"}},
        "community": {"source": {"source": "git", "url": "https://github.com/someone/community.git"}}
    });
    write(root, "plugins/known_marketplaces.json", &known.to_string())?;
    for (m, p, d) in [
        ("claude-plugins-official", "alpha", "Alpha tool"),
        ("community", "beta", "Beta tool"),
    ] {
        let dir = format!("plugins/cache/{m}/{p}/1.0.0");
        write(
            root,
            &format!("{dir}/.claude-plugin/plugin.json"),
            &format!(r#"{{"name":"{p}","description":"{d}","version":"1.0.0"}}"#),
        )?;
        write(
            root,
            &format!("{dir}/skills/s/SKILL.md"),
            "---\nname: s\ndescription: d\n---\n",
        )?;
    }
    let entries: Vec<_> = (0..50)
        .map(|i| json!({"name": format!("cat-{i}"), "source": format!("./plugins/cat-{i}")}))
        .collect();
    let market = json!({"name": "claude-plugins-official", "plugins": entries});
    write(
        root,
        "plugins/marketplaces/claude-plugins-official/.claude-plugin/marketplace.json",
        &market.to_string(),
    )?;
    for i in 0..50 {
        write(
            root,
            &format!("plugins/marketplaces/claude-plugins-official/plugins/cat-{i}/.claude-plugin/plugin.json"),
            &format!(r#"{{"name":"cat-{i}"}}"#),
        )?;
    }
    write(
        root,
        "plugins/marketplaces/claude-plugins-official/skills/x/SKILL.md",
        "---\nname: x\n---\n",
    )?;
    Ok(())
}

fn scan(root: &Path, include_managed: bool) -> Result<ScanReport, harneo_core::Error> {
    let options = ScanOptions {
        include_managed,
        ..ScanOptions::default()
    };
    ClaudeAdapter.scan(root, &options)
}

#[test]
fn installed_list_gives_exactly_its_plugins_with_origin() -> TestResult {
    let dir = tempfile::tempdir()?;
    fixture(dir.path(), "/elsewhere/plugins/cache/community/beta/2.0.0")?;
    let report = scan(dir.path(), false)?;
    assert_eq!(
        report.artifacts.len(),
        2,
        "{:?}",
        report.artifacts.iter().map(|a| &a.path).collect::<Vec<_>>()
    );
    assert!(report
        .artifacts
        .iter()
        .all(|a| a.kind == ArtifactKind::Plugin));

    let alpha = &report.artifacts[0];
    assert_eq!(alpha.name, "alpha");
    assert_eq!(
        alpha.path,
        "plugins/cache/claude-plugins-official/alpha/1.0.0"
    );
    assert_eq!(alpha.metadata["version"], "1.0.0");
    assert_eq!(alpha.metadata["pluginId"], "alpha@claude-plugins-official");
    assert_eq!(alpha.metadata["scopes"][0]["scope"], "user");
    assert_eq!(alpha.metadata["scopes"][1]["scope"], "project");
    assert_eq!(alpha.metadata["scopes"][1]["projectPath"], "/work/app");
    assert!(alpha.files.iter().any(|f| f.ends_with("skills/s/SKILL.md")));
    assert!(alpha.digest.is_some());
    assert_eq!(
        alpha.origin,
        Origin::Marketplace {
            name: "claude-plugins-official".into(),
            repo: Some("anthropics/claude-plugins-official".into()),
            official: true
        }
    );

    let beta = &report.artifacts[1];
    assert_eq!(
        beta.origin,
        Origin::Marketplace {
            name: "community".into(),
            repo: Some("someone/community".into()),
            official: false
        }
    );
    Ok(())
}

#[test]
fn install_path_outside_the_cache_is_refused_with_a_diagnostic() -> TestResult {
    let dir = tempfile::tempdir()?;
    fixture(dir.path(), "/elsewhere/plugins/cache/community/beta/2.0.0")?;
    let report = scan(dir.path(), false)?;
    let refused: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "installed.path-outside-cache")
        .collect();
    assert_eq!(refused.len(), 1);
    assert!(refused[0].message.contains("/elsewhere/"));

    // Under the root but not under plugins/cache, and a `..` escape.
    let other = tempfile::tempdir()?;
    fixture(
        other.path(),
        &format!("{}/skills/x", other.path().display()),
    )?;
    assert_eq!(scan(other.path(), false)?.artifacts.len(), 2);
    let list = std::fs::read_to_string(other.path().join("plugins/installed_plugins.json"))?;
    let escaped = list.replace(
        "/plugins/cache/community/beta/1.0.0",
        "/plugins/cache/../../x",
    );
    write(other.path(), "plugins/installed_plugins.json", &escaped)?;
    let report = scan(other.path(), false)?;
    assert_eq!(report.artifacts.len(), 1);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "installed.path-outside-cache"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_folder_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    write(
        outside.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"evil"}"#,
    )?;
    write(dir.path(), "plugins/cache/m/.keep", "")?;
    std::os::unix::fs::symlink(outside.path(), dir.path().join("plugins/cache/m/p"))?;
    let list = json!({"plugins": {"evil@m": [{"scope": "user",
        "installPath": format!("{}/plugins/cache/m/p", dir.path().display())}]}});
    write(
        dir.path(),
        "plugins/installed_plugins.json",
        &list.to_string(),
    )?;
    let report = scan(dir.path(), false)?;
    assert!(report.artifacts.is_empty());
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "installed.path-outside-cache"));
    Ok(())
}

#[test]
fn malformed_json_gives_a_diagnostic_and_no_plugins() -> TestResult {
    let dir = tempfile::tempdir()?;
    fixture(dir.path(), "/x")?;
    write(dir.path(), "plugins/installed_plugins.json", "{nope")?;
    let report = scan(dir.path(), false)?;
    assert!(report.artifacts.is_empty());
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "installed.invalid-json"));

    let fine = tempfile::tempdir()?;
    fixture(fine.path(), "/x")?;
    write(fine.path(), "plugins/known_marketplaces.json", "[")?;
    let report = scan(fine.path(), false)?;
    assert_eq!(report.artifacts.len(), 2);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "marketplaces.invalid-json"));
    assert_eq!(
        report.artifacts[0].origin,
        Origin::Marketplace {
            name: "claude-plugins-official".into(),
            repo: None,
            official: false
        }
    );
    Ok(())
}

#[test]
fn without_an_installed_list_nothing_changes() -> TestResult {
    let dir = tempfile::tempdir()?;
    fixture(dir.path(), "/x")?;
    std::fs::remove_file(dir.path().join("plugins/installed_plugins.json"))?;
    let report = scan(dir.path(), false)?;
    assert!(report.artifacts.len() >= 50);
    assert!(report.artifacts.iter().all(|a| a.origin == Origin::Own));
    Ok(())
}

#[test]
fn managed_folders_get_their_origin_when_included() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        "skills/synced/a/x/SKILL.md",
        "---\nname: synced\ndescription: d\n---\n",
    )?;
    write(
        dir.path(),
        "skills/own/SKILL.md",
        "---\nname: own\ndescription: d\n---\n",
    )?;
    write(
        dir.path(),
        "dev-mods/m/.claude-plugin/plugin.json",
        r#"{"name":"mod"}"#,
    )?;
    let report = scan(dir.path(), true)?;
    let origin = |n: &str| {
        report
            .artifacts
            .iter()
            .find(|a| a.name == n)
            .map(|a| a.origin.clone())
    };
    assert_eq!(origin("synced"), Some(Origin::ClaudeSynced));
    assert_eq!(origin("mod"), Some(Origin::DevMod));
    assert_eq!(origin("own"), Some(Origin::Own));
    Ok(())
}

#[test]
fn origin_serialises_tagged_camel_case() -> TestResult {
    assert_eq!(
        serde_json::to_value(Origin::ClaudeSynced)?,
        json!({"type": "claudeSynced"})
    );
    let m = Origin::Marketplace {
        name: "m".into(),
        repo: None,
        official: false,
    };
    assert_eq!(
        serde_json::to_value(m)?,
        json!({"type":"marketplace","name":"m","repo":null,"official":false})
    );
    Ok(())
}
