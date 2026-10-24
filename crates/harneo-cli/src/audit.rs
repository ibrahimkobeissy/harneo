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

/// Audits the folder. Phase 2 (Claude confirms each finding) follows the library's
/// `audit.confirmWithClaude` setting, and `claude` false skips it for this run. Without a
/// library, phase 2 is on by default and nothing is cached.
pub fn run(path: &Path, json: bool, include_managed: bool, claude: bool) -> anyhow::Result<()> {
    let options = ScanOptions {
        include_managed,
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(path, &options)?;
    let library = if claude {
        crate::profiles::open_library()
            .ok()
            .map(|(library, _)| library)
    } else {
        None
    };
    let entries: Vec<Entry> = report
        .artifacts
        .iter()
        .map(|a: &Artifact| {
            let audit = match &library {
                Some(library) => library.audit_confirmed(a, path, &options, true)?,
                None if claude => {
                    harneo_core::audit::confirm::audit_without_library(a, path, &options)
                }
                None => harneo_core::audit::confirm::off(ClaudeAdapter.audit(a, path, &options)),
            };
            Ok(Entry {
                artifact: Who {
                    kind: label(&a.kind),
                    name: &a.name,
                    path: &a.path,
                },
                audit,
            })
        })
        .collect::<anyhow::Result<_>>()?;
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
        if a.reach.protects > 0 {
            let _ = writeln!(out, "  protects {} credential paths", a.reach.protects);
        }
        if !a.dismissed.is_empty() {
            let _ = writeln!(out, "  dismissed by Claude: {}", a.dismissed.len());
            for d in &a.dismissed {
                let place = d.finding.line.map_or_else(
                    || d.finding.file.clone(),
                    |l| format!("{}:{l}", d.finding.file),
                );
                let _ = writeln!(out, "    {} {place}: {}", d.finding.rule, d.reason);
            }
        }
        if let Some(c) = &a.confirmation {
            let _ = write!(out, "  phase 2: {}", label(&c.status));
            if let Some(detail) = &c.detail {
                let _ = write!(out, " ({detail})");
            }
            out.push('\n');
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
                dismissed: Vec::new(),
                confirmation: None,
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

    #[test]
    fn render_lists_what_claude_dismissed_and_the_phase_two_status() {
        use harneo_core::audit::{ConfirmStatus, Confirmation, DismissedFinding};
        let finding = Finding {
            rule: "prompt-injection".into(),
            severity: Severity::Critical,
            message: "m".into(),
            file: "SKILL.md".into(),
            line: Some(7),
            evidence: String::new(),
        };
        let entries = vec![Entry {
            artifact: Who {
                kind: "skill".into(),
                name: "alpha",
                path: "skills/alpha",
            },
            audit: Audit {
                score: 100,
                tier: Tier::Trusted,
                findings: Vec::new(),
                reach: Reach::default(),
                dismissed: vec![DismissedFinding {
                    finding,
                    reason: "it only warns".into(),
                }],
                confirmation: Some(Confirmation {
                    detail: Some("no verdict".into()),
                    ..Confirmation::of(ConfirmStatus::Failed)
                }),
            },
        }];
        let text = render(&entries);
        assert!(text.contains(
            "  dismissed by Claude: 1\n    prompt-injection SKILL.md:7: it only warns\n"
        ));
        assert!(text.contains("  phase 2: failed (no verdict)\n"));
    }
}
