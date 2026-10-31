//! The security audit: how far to trust an artifact and what it can reach. Everything is static;
//! an artifact's files are read through the safe walk and never run.
//!
//! - `mod.rs` (this file): the result types, the score and tiers, the rule registry and [`audit`].
//! - `input.rs`: [`AuditInput`], an artifact's readable files with line indexes and code spans.
//! - `rules/`: one file per rule family; add a rule by adding one line to [`rules`].
//! - `reach.rs`: what the artifact can touch (files, network, shell, environment variables).
//!
//! Rule ids are stable once released (the library stores them), so never rename one.

pub mod confirm;
#[cfg(test)]
mod confirm_tests;
pub mod input;
mod protect;
mod reach;
mod redact;
mod rules;

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::{Artifact, ScanOptions};
pub use input::{AuditInput, FileRole, InputFile};
pub use redact::redact;
pub use rules::leaks::contains_token_shape;

/// How serious a finding is; decides how many points it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "AuditSeverity", rename_all = "camelCase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl Severity {
    /// Points taken off the score by the first finding of a rule.
    pub const fn deduction(self) -> u32 {
        match self {
            Self::Critical => 25,
            Self::High => 15,
            Self::Medium => 8,
            Self::Low => 3,
        }
    }
}

/// The reader-facing band of a trust score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Tier {
    /// 80 to 100.
    Trusted,
    /// 60 to 79.
    Review,
    /// 40 to 59.
    Risky,
    /// 0 to 39.
    Dangerous,
}

impl Tier {
    pub const fn from_score(score: u8) -> Self {
        match score {
            80..=u8::MAX => Self::Trusted,
            60..=79 => Self::Review,
            40..=59 => Self::Risky,
            _ => Self::Dangerous,
        }
    }
}

/// One thing a rule found. `evidence` is a short excerpt and never holds a secret value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Finding {
    /// The rule's stable id (kebab-case).
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    /// Relative to the scan root, forward slashes (an MCP server is `.mcp.json#<name>`).
    pub file: String,
    /// 1-based; `None` when the finding belongs to the file as a whole.
    pub line: Option<u32>,
    pub evidence: String,
}

/// What an artifact can reach, as sorted lists without duplicates. Names only, never values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Reach {
    pub files: Vec<String>,
    pub network: Vec<String>,
    pub shell: Vec<String>,
    pub env: Vec<String>,
    /// How many credential paths the artifact only guards (a "never read" list, a
    /// `permissions.deny` rule, a blocking hook). Info, not reach: never part of `files`.
    #[serde(default)]
    pub protects: u32,
}

impl Reach {
    /// Sorts every list and drops duplicates.
    pub fn normalize(&mut self) {
        for list in [
            &mut self.files,
            &mut self.network,
            &mut self.shell,
            &mut self.env,
        ] {
            list.sort();
            list.dedup();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
            && self.network.is_empty()
            && self.shell.is_empty()
            && self.env.is_empty()
    }
}

/// The audit of one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Audit {
    /// 0 to 100; see [`score`].
    pub score: u8,
    pub tier: Tier,
    /// The findings that count: every phase 1 finding that phase 2 did not dismiss. The score
    /// is computed from these only.
    pub findings: Vec<Finding>,
    pub reach: Reach,
    /// Findings Claude judged not to be a threat, with its reason. They stay auditable here
    /// and count for nothing: not in the score, not in the findings counts.
    #[serde(default)]
    pub dismissed: Vec<DismissedFinding>,
    /// Where phase 2 (Claude's confirmation) stands; `None` for a phase 1 result that was never
    /// stored (an import preview).
    #[serde(default)]
    pub confirmation: Option<Confirmation>,
    /// Files over the per-file limit, copied but never read by any rule: not audited.
    #[serde(default)]
    pub unaudited: Vec<String>,
}

/// A finding Claude judged not to be a threat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DismissedFinding {
    pub finding: Finding,
    /// Claude's one-sentence reason.
    pub reason: String,
}

/// How far phase 2 got for one artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ConfirmStatus {
    /// Phase 2 is off in Settings: the findings are phase 1's.
    Off,
    /// Waiting for Claude.
    Pending,
    /// The `claude` CLI is missing: every finding is kept.
    Unavailable,
    /// A call failed, timed out or answered badly: the findings without a verdict are kept.
    Failed,
    /// Every finding has a verdict (or needed none).
    Confirmed,
}

/// The phase 2 state stored with an audit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Confirmation {
    pub status: ConfirmStatus,
    /// The model that judged; `None` when Claude was not asked.
    pub model: Option<String>,
    /// Unix seconds of the run; `None` while pending or off.
    pub at: Option<i64>,
    /// Why a run was `unavailable` or `failed`, cut short.
    pub detail: Option<String>,
    /// What sort of failure a `failed` run was; `None` for other statuses and for results stored
    /// before this was kept.
    #[serde(default)]
    pub kind: Option<crate::bundles::FailureKind>,
}

impl Confirmation {
    pub fn of(status: ConfirmStatus) -> Self {
        Self {
            status,
            model: None,
            at: None,
            detail: None,
            kind: None,
        }
    }
}

/// A check over an artifact's files. Pure: it reads only the [`AuditInput`], never panics and
/// never touches the file system.
pub trait AuditRule: Send + Sync {
    /// Stable, kebab-case.
    fn id(&self) -> &'static str;
    /// The rule's usual severity (a rule may raise or lower it per finding).
    fn severity(&self) -> Severity;
    fn check(&self, input: &AuditInput) -> Vec<Finding>;
}

/// Every rule, in the order their findings are reported. A new rule is one line here.
pub fn rules() -> Vec<Box<dyn AuditRule>> {
    vec![
        // rules/injection.rs
        Box::new(rules::injection::PromptInjection),
        Box::new(rules::injection::HiddenUnicode),
        // rules/leaks.rs
        Box::new(rules::leaks::PlaintextSecrets),
        Box::new(rules::leaks::CredentialTheft),
        // rules/commands.rs
        Box::new(rules::commands::Rce),
        Box::new(rules::commands::DangerousCommands),
        Box::new(rules::commands::SafetyBypass),
        // rules/mcp.rs
        Box::new(rules::mcp::BroadPermissions),
        Box::new(rules::mcp::McpCommandInjection),
        // rules/supply_chain.rs
        Box::new(rules::supply_chain::SupplyChain),
        // rules/combo.rs
        Box::new(rules::combo::PermissionCombo),
    ]
}

/// The trust score: 100, minus the severity's points for the first finding of each rule and 1
/// for every further finding of the same rule, never below 0.
pub fn score(findings: &[Finding]) -> u8 {
    let mut seen = HashSet::new();
    let mut deducted: u32 = 0;
    for f in findings {
        deducted += if seen.insert(f.rule.as_str()) {
            f.severity.deduction()
        } else {
            1
        };
    }
    u8::try_from(100u32.saturating_sub(deducted)).unwrap_or(0)
}

/// Audits an already read input: every rule, then the score, tier and reach.
pub fn audit_input(input: &AuditInput) -> Audit {
    let order: Vec<&'static str> = rules().iter().map(|r| r.id()).collect();
    let mut findings: Vec<Finding> = rules().iter().flat_map(|r| r.check(input)).collect();
    // Registry order first, then where in the artifact: stable output for the same input.
    findings.sort_by_cached_key(|f| {
        let rank = order
            .iter()
            .position(|id| *id == f.rule)
            .unwrap_or(order.len());
        (rank, f.file.clone(), f.line)
    });
    let score = score(&findings);
    Audit {
        score,
        tier: Tier::from_score(score),
        findings,
        reach: reach::infer(input),
        dismissed: Vec::new(),
        confirmation: None,
        unaudited: input.oversize.clone(),
    }
}

/// Audits one scanned artifact: reads its files under `root` within `options`' limits (never a
/// symlink or a secret-named file), runs every rule and returns the result.
pub fn audit(artifact: &Artifact, root: &Path, options: &ScanOptions) -> Audit {
    audit_input(&AuditInput::read(artifact, root, options))
}

/// Audits an artifact's library copy in `root` (see [`AuditInput::read_stored`]): the same as
/// [`audit`], except an MCP server is judged by the stored `server.json` install builds
/// `.mcp.json` from.
pub fn audit_stored(artifact: &Artifact, root: &Path, options: &ScanOptions) -> Audit {
    audit_input(&AuditInput::read_stored(artifact, root, options))
}

/// A short, printable excerpt for `Finding::evidence`: secrets replaced by `<redacted>`,
/// invisible and control characters shown as `<U+XXXX>`, cut to `max` characters.
pub fn excerpt(text: &str, max: usize) -> String {
    let clean = redact(text.trim());
    let mut out = String::new();
    for (n, c) in clean.chars().enumerate() {
        if n >= max {
            out.push('…');
            break;
        }
        if input::is_invisible(c) || (c.is_control() && c != '\t') {
            out.push_str(&format!("<U+{:04X}>", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(rule: &str, severity: Severity) -> Finding {
        Finding {
            rule: rule.into(),
            severity,
            message: String::new(),
            file: "a".into(),
            line: Some(1),
            evidence: String::new(),
        }
    }

    #[test]
    fn deductions_follow_the_spec() {
        assert_eq!(score(&[]), 100);
        assert_eq!(score(&[f("a", Severity::Critical)]), 75);
        assert_eq!(score(&[f("a", Severity::High)]), 85);
        assert_eq!(score(&[f("a", Severity::Medium)]), 92);
        assert_eq!(score(&[f("a", Severity::Low)]), 97);
    }

    #[test]
    fn repeats_of_a_rule_cost_one_each() {
        let two = [f("a", Severity::Critical), f("a", Severity::Critical)];
        assert_eq!(score(&two), 74);
        // The repeat costs 1 whatever its own severity.
        assert_eq!(
            score(&[f("a", Severity::Low), f("a", Severity::Critical)]),
            96
        );
    }

    #[test]
    fn the_score_never_goes_below_zero() {
        let many: Vec<Finding> = (0..8)
            .map(|i| f(&format!("r{i}"), Severity::Critical))
            .collect();
        assert_eq!(score(&many), 0);
        let repeats: Vec<Finding> = (0..500).map(|_| f("a", Severity::Critical)).collect();
        assert_eq!(score(&repeats), 0);
    }

    /// Four distinct lows and a medium (20 points), then one point at a time.
    fn base_80() -> Vec<Finding> {
        let mut v: Vec<Finding> = (0..4).map(|i| f(&format!("l{i}"), Severity::Low)).collect();
        v.push(f("m", Severity::Medium));
        v
    }

    #[test]
    fn the_trusted_review_boundary_is_80_79() {
        let mut v = base_80();
        assert_eq!(
            (score(&v), Tier::from_score(score(&v))),
            (80, Tier::Trusted)
        );
        v.push(f("l0", Severity::Low));
        assert_eq!((score(&v), Tier::from_score(score(&v))), (79, Tier::Review));
    }

    #[test]
    fn the_review_risky_boundary_is_60_59() {
        let mut v = vec![f("c", Severity::Critical), f("h", Severity::High)];
        assert_eq!((score(&v), Tier::from_score(score(&v))), (60, Tier::Review));
        v.push(f("h", Severity::High));
        assert_eq!((score(&v), Tier::from_score(score(&v))), (59, Tier::Risky));
    }

    #[test]
    fn the_risky_dangerous_boundary_is_40_39() {
        let mut v = vec![
            f("c1", Severity::Critical),
            f("c2", Severity::Critical),
            f("m", Severity::Medium),
            f("m", Severity::Medium),
            f("m", Severity::Medium),
        ];
        assert_eq!((score(&v), Tier::from_score(score(&v))), (40, Tier::Risky));
        v.push(f("m", Severity::Medium));
        assert_eq!(
            (score(&v), Tier::from_score(score(&v))),
            (39, Tier::Dangerous)
        );
    }

    #[test]
    fn tiers_cover_every_score() {
        for (s, t) in [
            (100, Tier::Trusted),
            (80, Tier::Trusted),
            (79, Tier::Review),
            (60, Tier::Review),
            (59, Tier::Risky),
            (40, Tier::Risky),
            (39, Tier::Dangerous),
            (0, Tier::Dangerous),
        ] {
            assert_eq!(Tier::from_score(s), t, "{s}");
        }
    }

    #[test]
    fn rule_ids_are_unique_kebab_case() {
        let ids: Vec<&str> = rules().iter().map(|r| r.id()).collect();
        let unique: HashSet<&&str> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        for id in ids {
            assert!(
                !id.is_empty()
                    && id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{id}"
            );
        }
    }

    #[test]
    fn reach_normalize_sorts_and_dedups() {
        let mut r = Reach {
            files: vec!["b".into(), "a".into(), "b".into()],
            env: vec!["Z".into(), "Z".into()],
            ..Reach::default()
        };
        assert!(!r.is_empty());
        r.normalize();
        assert_eq!(
            (r.files, r.env),
            (vec!["a".into(), "b".into()], vec!["Z".into()])
        );
        assert!(Reach::default().is_empty());
    }

    #[test]
    fn serialises_camel_case() -> Result<(), serde_json::Error> {
        let a = Audit {
            score: 90,
            tier: Tier::Trusted,
            findings: vec![f("x", Severity::High)],
            reach: Reach::default(),
            dismissed: Vec::new(),
            confirmation: None,
            unaudited: Vec::new(),
        };
        let v = serde_json::to_value(&a)?;
        assert_eq!(
            (v["tier"].as_str(), v["findings"][0]["severity"].as_str()),
            (Some("trusted"), Some("high"))
        );
        Ok(())
    }

    #[test]
    fn excerpt_redacts_escapes_and_cuts() {
        let token = ["gh", "p_", &"a".repeat(36)].concat();
        let shown = excerpt(&format!("  token {token} \u{202E}x"), 200);
        assert_eq!(shown, "token <redacted> <U+202E>x");
        let long = excerpt(&"x".repeat(50), 10);
        assert_eq!(long.chars().count(), 11);
        assert!(long.ends_with('…'));
    }
}
