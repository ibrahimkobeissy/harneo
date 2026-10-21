use std::fs;
use std::path::Path;

use harneo_core::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions};
use harneo_core::Error;

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

#[test]
fn finds_skills_in_both_locations() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write(
        root,
        "skills/alpha/SKILL.md",
        "---\nname: alpha\ndescription: First\n---\nBody",
    )?;
    write(root, "skills/alpha/scripts/run.sh", "echo hi")?;
    write(
        root,
        ".claude/skills/beta/SKILL.md",
        "---\ndescription: Second\n---\n",
    )?;
    write(root, "skills/not-a-skill/README.md", "nothing")?;

    let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
    let paths: Vec<_> = report.artifacts.iter().map(|a| a.path.as_str()).collect();
    assert_eq!(paths, [".claude/skills/beta", "skills/alpha"]);
    let names: Vec<_> = report.artifacts.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["beta", "alpha"]);

    let alpha = &report.artifacts[1];
    assert_eq!(alpha.kind, ArtifactKind::Skill);
    assert_eq!(alpha.path, "skills/alpha");
    assert_eq!(alpha.description.as_deref(), Some("First"));
    assert_eq!(
        alpha.files,
        ["skills/alpha/SKILL.md", "skills/alpha/scripts/run.sh"]
    );
    assert!(alpha.diagnostics.is_empty());
    assert_eq!(report.artifacts[0].path, ".claude/skills/beta");
    assert_eq!(
        ClaudeAdapter.install_path(alpha),
        Path::new(".claude/skills/alpha")
    );
    Ok(())
}

#[test]
fn malformed_and_oversized_skills_become_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write(root, "skills/bad/SKILL.md", "---\nname: [oops\n---\n")?;
    write(root, "skills/big/SKILL.md", &"x".repeat(200))?;
    let options = ScanOptions {
        max_file_bytes: 100,
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(root, &options)?;
    let codes: Vec<_> = report
        .artifacts
        .iter()
        .flat_map(|a| a.diagnostics.iter())
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["skill.invalid-frontmatter", "walk.file-too-large"]);
    assert!(report.diagnostics.is_empty());
    Ok(())
}

#[test]
fn missing_folder_is_an_error() {
    let result = ClaudeAdapter.scan(Path::new("/definitely/not/here"), &ScanOptions::default());
    assert!(matches!(result, Err(Error::NotAFolder { .. })));
}

#[test]
fn same_folder_gives_the_same_report() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    write(
        dir.path(),
        "skills/a/SKILL.md",
        "---\ndescription: d\n---\n",
    )?;
    let first = ClaudeAdapter.scan(dir.path(), &ScanOptions::default())?;
    let second = ClaudeAdapter.scan(dir.path(), &ScanOptions::default())?;
    assert_eq!(first, second);
    Ok(())
}
