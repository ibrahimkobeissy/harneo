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

const SKILL: &str = "---\nname: NAME\ndescription: d\n---\n";

fn skill(root: &Path, rel: &str) -> std::io::Result<()> {
    let name = rel.rsplit('/').nth(1).unwrap_or("x");
    write(root, rel, &SKILL.replace("NAME", name))
}

fn scan(root: &Path) -> Result<ScanReport, harneo_core::Error> {
    ClaudeAdapter.scan(root, &ScanOptions::default())
}

fn summary(r: &ScanReport) -> Vec<(ArtifactKind, &str)> {
    r.artifacts
        .iter()
        .map(|a| (a.kind, a.path.as_str()))
        .collect()
}

#[test]
fn plugin_repo_is_one_plugin_without_inner_skills() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(
        r,
        ".claude-plugin/plugin.json",
        r#"{"name":"tool","version":"2.0.0","description":"A tool"}"#,
    )?;
    skill(r, "skills/inner/SKILL.md")?;
    skill(r, "nested/deep/SKILL.md")?;
    let report = scan(r)?;
    assert_eq!(summary(&report), [(ArtifactKind::Plugin, "")]);
    let p = &report.artifacts[0];
    assert_eq!(p.name, "tool");
    assert_eq!(p.description.as_deref(), Some("A tool"));
    assert_eq!(p.metadata["version"], "2.0.0");
    assert!(p.files.contains(&"skills/inner/SKILL.md".to_owned()));
    assert!(p.diagnostics.is_empty());
    Ok(())
}

#[test]
fn nested_plugin_is_not_entered() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(
        r,
        "vendor/tool/.claude-plugin/plugin.json",
        r#"{"name":"tool"}"#,
    )?;
    skill(r, "vendor/tool/skills/inner/SKILL.md")?;
    skill(r, "skills/own/SKILL.md")?;
    let report = scan(r)?;
    assert_eq!(
        summary(&report),
        [
            (ArtifactKind::Skill, "skills/own"),
            (ArtifactKind::Plugin, "vendor/tool")
        ]
    );
    Ok(())
}

#[test]
fn marketplace_gives_one_plugin_per_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(
        r,
        ".claude-plugin/marketplace.json",
        r#"{"name":"mk","owner":{"name":"o"},"plugins":[
          {"name":"local-a","source":"./plugins/a","description":"entry desc"},
          {"name":"gh","source":{"source":"github","repo":"org/gh"}},
          {"name":"web","source":{"source":"url","url":"https://example.com/w.git"}},
          {"name":"bare","source":"./plugins/bare"},
          {"name":"gone","source":"./plugins/gone"},
          {"name":"esc","source":"../outside"},
          {"source":"./plugins/a"}
        ]}"#,
    )?;
    write(
        r,
        "plugins/a/.claude-plugin/plugin.json",
        r#"{"name":"a","version":"1.0.0"}"#,
    )?;
    skill(r, "plugins/a/skills/s/SKILL.md")?;
    write(r, "plugins/bare/README.md", "x")?;
    skill(r, "plugins/bare/skills/s/SKILL.md")?;
    let report = scan(r)?;
    let mut names: Vec<_> = report.artifacts.iter().map(|a| a.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["a", "bare", "esc", "gh", "gone", "local-a", "web"]);
    assert!(report
        .artifacts
        .iter()
        .all(|a| a.kind == ArtifactKind::Plugin));

    let get = |n: &str| report.artifacts.iter().find(|a| a.name == n);
    let local = get("local-a").ok_or("local-a")?;
    assert_eq!(local.path, "plugins/a");
    assert_eq!(local.metadata["version"], "1.0.0");
    assert_eq!(local.metadata["source"], "./plugins/a");
    assert_eq!(local.metadata["marketplace"], "mk");
    assert!(local
        .files
        .contains(&"plugins/a/skills/s/SKILL.md".to_owned()));

    let gh = get("gh").ok_or("gh")?;
    assert_eq!(gh.metadata["source"]["repo"], "org/gh");
    assert_eq!(gh.path, ".claude-plugin/marketplace.json#gh");
    assert!(gh.diagnostics.is_empty());
    let web = get("web").ok_or("web")?;
    assert_eq!(web.metadata["source"]["url"], "https://example.com/w.git");

    let bare = get("bare").ok_or("bare")?;
    assert_eq!(bare.path, "plugins/bare");
    assert!(bare.files.contains(&"plugins/bare/README.md".to_owned()));
    let code = |n: &str| {
        get(n).map(|a| {
            a.diagnostics
                .iter()
                .map(|d| d.code.clone())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        code("gone"),
        Some(vec!["marketplace.source-not-found".to_owned()])
    );
    assert_eq!(code("esc"), Some(vec!["walk.path-escapes-root".to_owned()]));
    assert_eq!(code("a"), Some(vec!["plugin.missing-name".to_owned()]));
    assert!(report.diagnostics.is_empty());
    Ok(())
}

#[test]
fn invalid_marketplace_json_is_reported() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), ".claude-plugin/marketplace.json", "{oops")?;
    let report = scan(dir.path())?;
    let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["marketplace.invalid-json"]);
    assert!(report.artifacts.is_empty());
    Ok(())
}

#[test]
fn invalid_plugin_json_and_missing_name_are_errors() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    write(r, "a/.claude-plugin/plugin.json", "{oops")?;
    write(r, "b/.claude-plugin/plugin.json", "{}")?;
    let report = scan(r)?;
    let codes: Vec<_> = report
        .artifacts
        .iter()
        .map(|a| (a.name.as_str(), a.diagnostics[0].code.as_str()))
        .collect();
    assert_eq!(
        codes,
        [("a", "plugin.invalid-json"), ("b", "plugin.missing-name")]
    );
    Ok(())
}

#[test]
fn claude_home_and_pack_skills_are_reported_once() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    skill(r, "skills/a/SKILL.md")?;
    skill(r, ".claude/skills/b/SKILL.md")?;
    let report = scan(r)?;
    assert_eq!(
        summary(&report),
        [
            (ArtifactKind::Skill, ".claude/skills/b"),
            (ArtifactKind::Skill, "skills/a")
        ]
    );
    Ok(())
}

#[test]
fn loose_skills_respect_the_depth_limit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let r = dir.path();
    skill(r, "one/SKILL.md")?;
    skill(r, "a/b/c/four/SKILL.md")?; // 4 levels: found
    skill(r, "a/b/c/d/five/SKILL.md")?; // 5 levels: not found
    skill(r, "one/inner/SKILL.md")?; // inside a skill folder: not a separate skill
    let report = scan(r)?;
    assert_eq!(
        summary(&report),
        [
            (ArtifactKind::Skill, "a/b/c/four"),
            (ArtifactKind::Skill, "one")
        ]
    );
    let shallow = ClaudeAdapter.scan(
        r,
        &ScanOptions {
            max_loose_skill_depth: 1,
            ..ScanOptions::default()
        },
    )?;
    assert_eq!(summary(&shallow), [(ArtifactKind::Skill, "one")]);
    Ok(())
}

fn same_repo_fixture(r: &Path, entry_repo: &str, manifest: &str) -> std::io::Result<()> {
    write(
        r,
        ".claude-plugin/marketplace.json",
        &format!(
            r#"{{"name":"addy-agent-skills","plugins":[
              {{"name":"agent-skills","source":{{"source":"github","repo":"{entry_repo}"}}}}]}}"#
        ),
    )?;
    write(r, ".claude-plugin/plugin.json", manifest)?;
    skill(r, "skills/s/SKILL.md")
}

fn scan_repo(root: &Path, repo: Option<&str>) -> Result<ScanReport, harneo_core::Error> {
    let options = ScanOptions {
        repo: repo.map(str::to_owned),
        ..ScanOptions::default()
    };
    ClaudeAdapter.scan(root, &options)
}

#[test]
fn a_github_entry_naming_the_imported_repo_merges_with_the_root_plugin() -> TestResult {
    let dir = tempfile::tempdir()?;
    same_repo_fixture(
        dir.path(),
        "addyosmani/agent-skills",
        r#"{"name":"agent-skills","version":"1"}"#,
    )?;
    let report = scan_repo(dir.path(), Some("addyosmani/agent-skills"))?;
    assert_eq!(report.artifacts.len(), 1);
    let p = &report.artifacts[0];
    assert_eq!(p.name, "agent-skills");
    assert_eq!(p.path, "");
    assert!(p.files.contains(&"skills/s/SKILL.md".to_owned()));
    assert_eq!(p.metadata["marketplace"], "addy-agent-skills");
    assert_eq!(p.metadata["source"]["repo"], "addyosmani/agent-skills");
    assert_eq!(p.metadata["version"], "1");
    Ok(())
}

#[test]
fn the_plugins_own_repository_field_also_identifies_the_repo() -> TestResult {
    let dir = tempfile::tempdir()?;
    same_repo_fixture(
        dir.path(),
        "addyosmani/agent-skills",
        r#"{"name":"agent-skills","repository":"https://github.com/AddyOsmani/agent-skills.git"}"#,
    )?;
    let report = scan_repo(dir.path(), None)?;
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(
        report.artifacts[0].metadata["marketplace"],
        "addy-agent-skills"
    );
    Ok(())
}

#[test]
fn a_github_entry_for_another_repo_stays_a_separate_remote_plugin() -> TestResult {
    let dir = tempfile::tempdir()?;
    same_repo_fixture(dir.path(), "someone/else", r#"{"name":"agent-skills"}"#)?;
    let report = scan_repo(dir.path(), Some("addyosmani/agent-skills"))?;
    let mut paths: Vec<_> = report.artifacts.iter().map(|a| a.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["", ".claude-plugin/marketplace.json#agent-skills"]);
    Ok(())
}

#[test]
fn a_same_repo_entry_with_another_name_stays_separate() -> TestResult {
    let dir = tempfile::tempdir()?;
    same_repo_fixture(dir.path(), "o/r", r#"{"name":"other-name"}"#)?;
    let report = scan_repo(dir.path(), Some("o/r"))?;
    assert_eq!(report.artifacts.len(), 2);
    Ok(())
}
