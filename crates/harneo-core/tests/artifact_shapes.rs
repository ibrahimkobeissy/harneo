//! Scans each committed fixture shape and checks exactly what is found.

use std::collections::BTreeSet;
use std::path::PathBuf;

use harneo_core::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions, ScanReport};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn scan(shape: &str) -> Result<ScanReport, harneo_core::Error> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/artifact")
        .join(shape);
    ClaudeAdapter.scan(&root, &ScanOptions::default())
}

/// One line per artifact: kind, name, path, files, references (`!` marks unresolved), codes.
fn projection(report: &ScanReport) -> Vec<String> {
    report
        .artifacts
        .iter()
        .map(|a| {
            let refs: Vec<String> = a
                .references
                .iter()
                .map(|r| format!("{}{}", r.target, if r.resolved { "" } else { "!" }))
                .collect();
            let codes: Vec<&str> = a.diagnostics.iter().map(|d| d.code.as_str()).collect();
            format!(
                "{:?} {} @ {} files={:?} refs={:?} codes={:?}",
                a.kind, a.name, a.path, a.files, refs, codes
            )
        })
        .collect()
}

fn check(shape: &str, expected: &[&str]) -> TestResult {
    let report = scan(shape)?;
    assert_eq!(projection(&report), expected);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    Ok(())
}

#[test]
fn project_shape() -> TestResult {
    check(
        "project",
        &[
            r#"Agent reviewer @ .claude/agents/reviewer.md files=[".claude/agents/reviewer.md"] refs=["tdd"] codes=[]"#,
            r#"Command git:ship @ .claude/commands/git/ship.md files=[".claude/commands/git/ship.md"] refs=[] codes=[]"#,
            r#"OutputStyle terse @ .claude/output-styles/terse.md files=[".claude/output-styles/terse.md"] refs=[] codes=[]"#,
            r#"Rule react @ .claude/rules/frontend/react.md files=[".claude/rules/frontend/react.md"] refs=[] codes=[]"#,
            r#"Rule style @ .claude/rules/style.md files=[".claude/rules/style.md"] refs=[] codes=[]"#,
            r#"Settings settings @ .claude/settings.json files=[".claude/hooks/guard.sh", ".claude/settings.json"] refs=["terse"] codes=[]"#,
            r#"Skill tdd @ .claude/skills/tdd files=[".claude/skills/tdd/SKILL.md", ".claude/skills/tdd/scripts/check.sh"] refs=[] codes=[]"#,
            r#"Workflow audit @ .claude/workflows/audit.js files=[".claude/workflows/audit.js"] refs=[] codes=[]"#,
            r#"Mcp github @ .mcp.json#github files=[".mcp.json"] refs=[] codes=[]"#,
            r#"ClaudeMd CLAUDE.md @ CLAUDE.md files=["CLAUDE.md", "docs/architecture.md"] refs=[] codes=["claude-md.missing-import"]"#,
        ],
    )
}

#[test]
fn claude_home_shape() -> TestResult {
    check(
        "claude-home",
        &[
            r#"ClaudeMd CLAUDE.md @ CLAUDE.md files=["CLAUDE.md"] refs=[] codes=[]"#,
            r#"Agent scout @ agents/scout.md files=["agents/scout.md"] refs=[] codes=[]"#,
            r#"Command standup @ commands/standup.md files=["commands/standup.md"] refs=[] codes=[]"#,
            r#"OutputStyle plain @ output-styles/plain.md files=["output-styles/plain.md"] refs=[] codes=[]"#,
            r#"Settings settings @ settings.json files=["settings.json"] refs=[] codes=[]"#,
            r#"Skill notes @ skills/notes files=["skills/notes/SKILL.md"] refs=[] codes=[]"#,
        ],
    )
}

#[test]
fn pack_shape() -> TestResult {
    check(
        "pack",
        &[
            r#"Agent tester @ agents/tester.md files=["agents/tester.md"] refs=["review", "ghost-skill!"] codes=[]"#,
            r#"Command review @ commands/review.md files=["commands/review.md"] refs=[] codes=["scan.skill-command-clash"]"#,
            r#"OutputStyle explain @ output-styles/explain.md files=["output-styles/explain.md"] refs=[] codes=[]"#,
            r#"Rule testing @ rules/testing.md files=["rules/testing.md"] refs=[] codes=[]"#,
            r#"Skill review @ skills/review files=["skills/review/SKILL.md", "skills/review/checklist.md"] refs=[] codes=[]"#,
            r#"Workflow triage @ workflows/triage.js files=["workflows/triage.js"] refs=[] codes=[]"#,
        ],
    )
}

#[test]
fn plugin_shape_is_one_whole_plugin() -> TestResult {
    check(
        "plugin",
        &[
            r#"Plugin lint-helper @  files=[".claude-plugin/plugin.json", "agents/fixer.md", "skills/lint/SKILL.md"] refs=[] codes=[]"#,
        ],
    )
}

#[test]
fn marketplace_shape_has_one_plugin_per_entry() -> TestResult {
    check(
        "marketplace",
        &[
            r#"Plugin remote-tool @ .claude-plugin/marketplace.json#remote-tool files=[".claude-plugin/marketplace.json"] refs=[] codes=[]"#,
            r#"Plugin formatter @ plugins/formatter files=["plugins/formatter/.claude-plugin/plugin.json", "plugins/formatter/skills/fmt/SKILL.md"] refs=[] codes=[]"#,
        ],
    )
}

#[test]
fn the_fixtures_together_hold_all_ten_types() -> TestResult {
    let mut kinds = BTreeSet::new();
    for shape in ["project", "claude-home", "pack", "plugin", "marketplace"] {
        kinds.extend(
            scan(shape)?
                .artifacts
                .iter()
                .map(|a| format!("{:?}", a.kind)),
        );
    }
    assert_eq!(kinds.len(), 10, "{kinds:?}");
    assert!(kinds.contains(&format!("{:?}", ArtifactKind::Plugin)));
    Ok(())
}

#[test]
fn scanning_a_fixture_twice_gives_the_same_report_and_digests() -> TestResult {
    for shape in ["project", "claude-home", "pack", "plugin", "marketplace"] {
        let first = serde_json::to_string(&scan(shape)?)?;
        let second = serde_json::to_string(&scan(shape)?)?;
        assert_eq!(first, second, "{shape}");
        assert!(first.contains("\"digest\":\""), "{shape} has no digests");
    }
    Ok(())
}

#[test]
fn every_artifact_has_an_install_path_under_the_project_root() -> TestResult {
    let report = scan("project")?;
    let paths: Vec<String> = report
        .artifacts
        .iter()
        .map(|a| ClaudeAdapter.install_path(a).display().to_string())
        .collect();
    assert_eq!(
        paths,
        [
            ".claude/agents/reviewer.md",
            ".claude/commands/git/ship.md",
            ".claude/output-styles/terse.md",
            ".claude/rules/frontend/react.md",
            ".claude/rules/style.md",
            ".claude/settings.json",
            ".claude/skills/tdd",
            ".claude/workflows/audit.js",
            ".mcp.json",
            "CLAUDE.md",
        ]
    );
    Ok(())
}
