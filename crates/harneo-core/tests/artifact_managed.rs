//! Claude-managed folders (`skills/synced/`, `dev-mods/`) are skipped by default and counted.

use std::path::Path;

use harneo_core::artifact::{
    AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions, ScanReport, Severity,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

fn scan(root: &Path, include_managed: bool) -> Result<ScanReport, harneo_core::Error> {
    let options = ScanOptions {
        include_managed,
        ..ScanOptions::default()
    };
    ClaudeAdapter.scan(root, &options)
}

fn skipped(report: &ScanReport) -> Vec<(&str, &str, Severity)> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == "scan.managed-skipped")
        .map(|d| (d.file.as_str(), d.message.as_str(), d.severity))
        .collect()
}

#[test]
fn synced_skills_are_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    for rel in [
        "skills/synced/a_b/x/SKILL.md",
        "skills/synced/c_d/x/SKILL.md",
    ] {
        write(dir.path(), rel, "---\nname: x\ndescription: d\n---\n")?;
    }
    write(
        dir.path(),
        "skills/own/SKILL.md",
        "---\nname: own\ndescription: d\n---\n",
    )?;
    let report = scan(dir.path(), false)?;
    let names: Vec<_> = report.artifacts.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["own"]);
    let found = skipped(&report);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "skills/synced");
    assert!(found[0].1.contains("2 skill folders"), "{}", found[0].1);
    assert_eq!(found[0].2, Severity::Info);

    let all = scan(dir.path(), true)?;
    assert_eq!(all.artifacts.len(), 3);
    assert!(skipped(&all).is_empty());
    Ok(())
}

#[test]
fn dot_claude_synced_skills_are_skipped_too() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        ".claude/skills/synced/x/SKILL.md",
        "---\nname: x\n---\n",
    )?;
    let report = scan(dir.path(), false)?;
    assert!(report.artifacts.is_empty());
    assert_eq!(skipped(&report).len(), 1);
    assert_eq!(scan(dir.path(), true)?.artifacts.len(), 1);
    Ok(())
}

#[test]
fn dev_mods_are_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    for rel in [
        "dev-mods/a/m1/.claude-plugin/plugin.json",
        "dev-mods/b/m2/.claude-plugin/plugin.json",
    ] {
        write(dir.path(), rel, r#"{"name":"m"}"#)?;
    }
    let report = scan(dir.path(), false)?;
    assert!(report.artifacts.is_empty());
    let found = skipped(&report);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "dev-mods");
    assert!(found[0].1.contains("2 mod folders"), "{}", found[0].1);

    let all = scan(dir.path(), true)?;
    let plugins = all
        .artifacts
        .iter()
        .filter(|a| a.kind == ArtifactKind::Plugin)
        .count();
    assert_eq!(plugins, 2);
    Ok(())
}
