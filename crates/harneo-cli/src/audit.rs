//! `harneo audit`: scan a folder, audit every artifact in it and print the result.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use harneo_core::artifact::{AgentAdapter, Artifact, ClaudeAdapter, ScanOptions};
use harneo_core::audit::{Audit, Finding, Severity, Tier};
use serde::Serialize;

/// The camelCase name a type has in the JSON output, reused so text and JSON agree.
fn label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[derive(Serialize)]
struct Who<'a> {
    kind: String,
    name: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct Entry<'a> {
    artifact: Who<'a>,
    audit: Audit,
}

const SEVERITIES: [Severity; 4] = [
    Severity::Critical,
    Severity::High,
    Severity::Medium,
    Severity::Low,
];
const TIERS: [Tier; 4] = [Tier::Trusted, Tier::Review, Tier::Risky, Tier::Dangerous];

pub fn run(path: &Path, json: bool, include_managed: bool) -> anyhow::Result<()> {
    let options = ScanOptions {
        include_managed,
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(path, &options)?;
    let entries: Vec<Entry> = report
        .artifacts
        .iter()
        .map(|a: &Artifact| Entry {
            artifact: Who {
                kind: label(&a.kind),
                name: &a.name,
                path: &a.path,
            },
            audit: ClaudeAdapter.audit(a, path, &options),
        })
        .collect();
    if json {
        let text = serde_json::to_string_pretty(&entries).context("cannot serialize the audit")?;
        println!("{text}");
    } else {
        print!("{}", render(&entries));
    }
    Ok(())
}

fn finding_lines(out: &mut String, f: &Finding) {
    let place = f
        .line
        .map_or_else(|| f.file.clone(), |l| format!("{}:{l}", f.file));
    let _ = writeln!(
        out,
        "    {} {} {place}: {}",
        label(&f.severity),
        f.rule,
        f.message
    );
    if !f.evidence.is_empty() {
        let _ = writeln!(out, "      {}", f.evidence);
    }
}

fn render(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        let a = &e.audit;
        let _ = writeln!(
            out,
            "{} {}\n  path: {}\n  score: {} ({})",
            e.artifact.kind,
            e.artifact.name,
            e.artifact.path,
            a.score,
            label(&a.tier)
        );
        for sev in SEVERITIES {
            let group: Vec<&Finding> = a.findings.iter().filter(|f| f.severity == sev).collect();
            if group.is_empty() {
                continue;
            }
            let _ = writeln!(out, "  {} findings:", label(&sev));
            for f in group {
                finding_lines(&mut out, f);
            }
        }
        for (name, list) in [
            ("files", &a.reach.files),
            ("network", &a.reach.network),
            ("shell", &a.reach.shell),
            ("env", &a.reach.env),
        ] {
            if !list.is_empty() {
                let _ = writeln!(out, "  reach {name}: {}", list.join(", "));
            }
        }
        out.push('\n');
    }
    out.push_str(&summary(entries));
    out
}

fn summary(entries: &[Entry]) -> String {
    let mut tiers: BTreeMap<Tier, usize> = BTreeMap::new();
    let mut severities: BTreeMap<Severity, usize> = BTreeMap::new();
    for e in entries {
        *tiers.entry(e.audit.tier).or_default() += 1;
        for f in &e.audit.findings {
            *severities.entry(f.severity).or_default() += 1;
        }
    }
    let by_tier = TIERS
        .iter()
        .map(|t| format!("{} {}", tiers.get(t).copied().unwrap_or(0), label(t)))
        .collect::<Vec<_>>()
        .join(", ");
    let by_severity = SEVERITIES
        .iter()
        .map(|s| format!("{} {}", severities.get(s).copied().unwrap_or(0), label(s)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} artifacts ({by_tier}); findings: {by_severity}\n",
        entries.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use harneo_core::audit::Reach;

    #[test]
    fn render_groups_findings_and_prints_reach_and_summary() {
        let entries = vec![Entry {
            artifact: Who {
                kind: "skill".into(),
                name: "alpha",
                path: "skills/alpha",
            },
            audit: Audit {
                score: 75,
                tier: Tier::Review,
                findings: vec![Finding {
                    rule: "rce".into(),
                    severity: Severity::Critical,
                    message: "pipes a download to a shell".into(),
                    file: "skills/alpha/SKILL.md".into(),
                    line: Some(4),
                    evidence: "curl x | sh".into(),
                }],
                reach: Reach {
                    network: vec!["x".into()],
                    ..Reach::default()
                },
            },
        }];
        let text = render(&entries);
        assert!(text.contains("  score: 75 (review)\n  critical findings:\n"));
        assert!(text.contains(
            "    critical rce skills/alpha/SKILL.md:4: pipes a download to a shell\n      curl x | sh\n"
        ));
        assert!(text.contains("  reach network: x\n"));
        assert!(!text.contains("reach files"));
        assert!(text.ends_with(
            "1 artifacts (0 trusted, 1 review, 0 risky, 0 dangerous); findings: 1 critical, 0 high, 0 medium, 0 low\n"
        ));
    }
}
