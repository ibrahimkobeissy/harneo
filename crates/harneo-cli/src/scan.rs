//! `harneo scan`: read a folder and print the artifacts and diagnostics found in it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use harneo_core::artifact::{
    AgentAdapter, ClaudeAdapter, Diagnostic, ScanOptions, ScanReport, Severity,
};
use serde::Serialize;

/// The camelCase name a type has in the JSON output, reused so text and JSON agree.
fn label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

pub fn run(path: &Path, json: bool) -> anyhow::Result<()> {
    let report = ClaudeAdapter.scan(path, &ScanOptions::default())?;
    if json {
        let text = serde_json::to_string_pretty(&report).context("cannot serialize the scan")?;
        println!("{text}");
    } else {
        print!("{}", render(&report));
    }
    Ok(())
}

fn diagnostic_line(indent: &str, d: &Diagnostic) -> String {
    format!(
        "{indent}{} {}: {} ({})\n",
        label(&d.severity),
        d.code,
        d.message,
        d.file
    )
}

fn render(report: &ScanReport) -> String {
    let mut out = String::new();
    for a in &report.artifacts {
        let _ = writeln!(
            out,
            "{} {}\n  path: {}\n  files: {}",
            label(&a.kind),
            a.name,
            a.path,
            a.files.len()
        );
        for r in &a.references {
            let state = if r.resolved { "resolved" } else { "unresolved" };
            let _ = writeln!(
                out,
                "  reference: {} {} ({state})",
                label(&r.kind),
                r.target
            );
        }
        for d in &a.diagnostics {
            out.push_str(&diagnostic_line("  ", d));
        }
        out.push('\n');
    }
    if !report.diagnostics.is_empty() {
        out.push_str("root diagnostics:\n");
        for d in &report.diagnostics {
            out.push_str(&diagnostic_line("  ", d));
        }
        out.push('\n');
    }
    out.push_str(&summary(report));
    out
}

fn summary(report: &ScanReport) -> String {
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for a in &report.artifacts {
        *kinds.entry(label(&a.kind)).or_default() += 1;
    }
    let all = || {
        report
            .artifacts
            .iter()
            .flat_map(|a| a.diagnostics.iter())
            .chain(report.diagnostics.iter())
    };
    let errors = all().filter(|d| d.severity == Severity::Error).count();
    let warnings = all().filter(|d| d.severity == Severity::Warning).count();
    let counts = if kinds.is_empty() {
        "none".to_owned()
    } else {
        kinds
            .iter()
            .map(|(k, n)| format!("{n} {k}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{} artifacts ({counts}); {errors} errors, {warnings} warnings ({} at the root)\n",
        report.artifacts.len(),
        report.diagnostics.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use harneo_core::artifact::{Artifact, ArtifactKind, Reference};

    #[test]
    fn render_shows_references_diagnostics_and_root_findings() {
        let report = ScanReport {
            artifacts: vec![Artifact {
                kind: ArtifactKind::Skill,
                name: "alpha".into(),
                description: None,
                path: "skills/alpha".into(),
                files: vec!["skills/alpha/SKILL.md".into()],
                metadata: serde_json::Map::new(),
                references: vec![
                    Reference {
                        kind: ArtifactKind::Rule,
                        target: "style".into(),
                        resolved: true,
                    },
                    Reference {
                        kind: ArtifactKind::Agent,
                        target: "ghost".into(),
                        resolved: false,
                    },
                ],
                diagnostics: vec![Diagnostic::warning(
                    "skill.missing-description",
                    Path::new("skills/alpha/SKILL.md"),
                    "needs a description",
                )],
                digest: None,
            }],
            diagnostics: vec![Diagnostic::error(
                "skill.file-too-large",
                Path::new("skills/big/SKILL.md"),
                "too big",
            )],
        };
        let text = render(&report);
        assert!(text.contains("  reference: rule style (resolved)\n"));
        assert!(text.contains("  reference: agent ghost (unresolved)\n"));
        assert!(text.contains(
            "  warning skill.missing-description: needs a description (skills/alpha/SKILL.md)\n"
        ));
        assert!(text.contains("root diagnostics:\n  error skill.file-too-large: too big"));
        assert!(text.ends_with("1 artifacts (1 skill); 1 errors, 1 warnings (1 at the root)\n"));
    }
}
