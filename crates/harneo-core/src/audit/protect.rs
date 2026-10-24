//! Protective mentions of credential paths. A `CLAUDE.md` that forbids the agent to read the SSH
//! folder, a `permissions.deny` rule on it and a guard hook that blocks reads of it all name
//! credential paths, but they defend them. They are neither `credential-theft` findings nor
//! reach; they are counted and shown as an info line ("Protects N credential paths").
//!
//! The decision is per line (or per list, through its intro paragraph and heading), never per
//! file: a real read, and above all a read with a network send, stays a finding in a file that
//! also has protective text.

use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::input::{FileRole, InputFile};

/// Credential stores, keychain tools and environment dumps (case-insensitive). The first
/// `PATH_TARGETS` are paths; the rest are commands.
pub(crate) const CRED_TARGETS: [&str; 12] = [
    r"\.ssh/(?:id_[a-z0-9]+|authorized_keys|known_hosts|config)\b",
    r"\b(?:id_rsa|id_ed25519|id_ecdsa|id_dsa)\b",
    r"\.(?:aws/(?:credentials|config)|gcloud/(?:credentials|application_default)|config/gcloud)",
    r#"(?:^|[\s/"'=<(])\.env(?:\.(?:local|production|prod|development|dev|staging|test))?(?:$|[\s/"'):;|&>])"#,
    r"(?:\bcredentials\.json|\.netrc|\.pgpass|\.git-credentials|\.npmrc|\.pypirc)\b",
    r"\.config/gh\b",
    r"\.docker/config\.json",
    r"\.kube(?:/|\b)",
    r"\.claude/\.credentials\.json",
    r"\bsecurity\s+find-(?:generic|internet)-password\b|\bsecret-tool\s+(?:lookup|search)\b|\bkeyctl\s+(?:read|print|pipe)\b|\bgit\s+credential\s+fill\b|\bcmdkey\s+/list\b|\b(?:kwallet-query|gnome-keyring)\b",
    r"\bprintenv\b|\bexport\s+-p\b|\bcompgen\s+-e\b",
    r"(?:^|[;&|(`]\s*|\$\(\s*)env\s*(?:$|[|>;)`])",
];
const PATH_TARGETS: usize = 9;

/// Network sends (case-insensitive).
pub(crate) const SEND_PATTERNS: [&str; 3] = [
    r"\b(?:curl|wget|nc|ncat|netcat|socat|scp|rsync|sftp|ftp|telnet)\b",
    r"/dev/tcp/|\bfetch\s*\(|\brequests\.(?:post|put)\b|\baxios\.(?:post|put)\b|XMLHttpRequest|\bInvoke-(?:WebRequest|RestMethod)\b|\burlopen\s*\(|\bhttp\.client\b|\bgh\s+gist\s+create\b",
    r"\b(?:post|upload|send|exfil\w*)\b.*https?://",
];

/// A token that names a credential path or file, folders included (`~/.ssh`, `.gnupg/`, `*.pem`).
const MENTION: &str = r"(?i)\.ssh\b|\.aws\b|\.gnupg\b|\.azure\b|\.kube\b|\.docker/config|\.oci\b|\.config/(?:gcloud|gh|anthropic)|\.netrc|\.npmrc|\.pypirc|\.pgpass|\.git-credentials|\.claude/\.credentials|\.codex\b|\.gemini\b|\.env\b|\.pem\b|\.key\b|\.p12\b|\.pfx\b|\.jks\b|\.keystore\b|\bid_(?:rsa|ed25519|ecdsa|dsa)|credentials\S*\.json|(?:^|/)secrets\.[\w*]+|\.secret\b|\.tfstate|\.vault-token|\.my\.cnf|_history\b";

const PROHIBITION: &str = r"(?i)\bnever\b|\bdo\s+not\b|\bdon'?t\b|\bmust\s+not\b|\bmustn'?t\b|\bprohibit\w*|\bforbid\w*|\bforbidden\b|off[\s-]limits|\bdeny\b|\bdenied\b|\bdenies\b|\bblock(?:s|ed)?\b|\bguardrails?\b|\bnot\s+allowed\b|\brefuse\b";

const READ_VERB: &str = r"(?i)\b(?:cat|less|more|head|tail|read|reads|reading|open|opens|cp|copy|upload|curl|wget|scp|rsync|sftp|tar|zip|base64|grep|rg|source|dump|exfiltrate|send|post|get-content|sed|awk|xxd|strings)\b";

const BLOCK_MARKER: &str = r#"(?i)\bexit\s*\(?\s*2\b|["']decision["']\s*:\s*["']block|permissionDecision["']?\s*[:=]\s*["']deny"#;

/// A script that guards: it returns a deny decision, checks for one, or writes deny rules.
const GUARD_MARKER: &str = r#"(?i)permissionDecision\W{0,24}deny|\bexit\s*\(?\s*2\b|["']decision["']\s*:\s*["']block|permissions\W{0,40}deny|["']deny["']\s*[,)\]:]"#;

const START_WITH_VERB: &str = r"(?i)^\s*(?:sudo\s+)?(?:cat|less|more|head|tail|cp|scp|rsync|curl|wget|tar|zip|base64|grep|source|\.)\s";

fn compile(p: &str) -> Option<Regex> {
    Regex::new(p).ok()
}

pub(crate) static CRED_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    CRED_TARGETS
        .iter()
        .filter_map(|p| compile(&format!("(?i){p}")))
        .collect()
});
pub(crate) static SEND_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    SEND_PATTERNS
        .iter()
        .filter_map(|p| compile(&format!("(?i){p}")))
        .collect()
});
static MENTION_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(MENTION));
static PROHIBITION_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(PROHIBITION));
static VERB_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(READ_VERB));
static BLOCK_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(BLOCK_MARKER));
static START_VERB_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(START_WITH_VERB));
static GUARD_RE: LazyLock<Option<Regex>> = LazyLock::new(|| compile(GUARD_MARKER));

fn is_match(re: &LazyLock<Option<Regex>>, text: &str) -> bool {
    re.as_ref().is_some_and(|r| r.is_match(text))
}

/// True when `line` matches a `credential-theft` target.
pub(crate) fn names_credential(line: &str) -> bool {
    CRED_RES.iter().any(|r| r.is_match(line))
}

fn sends(line: &str) -> bool {
    SEND_RES.iter().any(|r| r.is_match(line))
}

/// True when the line names a credential path or command in any form.
fn mentions(line: &str) -> bool {
    names_credential(line) || is_match(&MENTION_RE, line)
}

/// The tokens of `line` that name a credential path, as shown to the reader.
fn mentioned_paths(line: &str, out: &mut BTreeSet<String>) {
    let split = |c: char| c.is_whitespace() || "\"'`(),;|<>[]{}".contains(c);
    for token in line.split(split) {
        let token = token.trim_end_matches(['.', ':', '!', '?']);
        if !token.is_empty() && token.len() < 120 && mentions(token) {
            out.insert(token.to_owned());
        }
    }
}

/// Which lines of a file are protective mentions, and which credential paths they protect.
#[derive(Debug, Default)]
pub(crate) struct Protection {
    pub lines: HashSet<u32>,
    pub paths: BTreeSet<String>,
}

pub(crate) fn analyze(file: &InputFile) -> Protection {
    let mut p = Protection::default();
    match file.role {
        FileRole::Settings => settings(file, &mut p),
        FileRole::Markdown => markdown(file, &mut p),
        FileRole::HookScript => hook_script(file, &mut p),
        FileRole::Script => guard_script(file, &mut p),
        _ => {}
    }
    p
}

/// Entries of `permissions.deny` and `permissions.ask`.
fn settings(file: &InputFile, p: &mut Protection) {
    let Ok(root) = serde_json::from_str::<Value>(&file.text) else {
        return;
    };
    for key in ["deny", "ask"] {
        let Some(entries) = root
            .pointer(&format!("/permissions/{key}"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let from = file.text.find(&format!("\"{key}\"")).unwrap_or(0);
        for entry in entries.iter().filter_map(Value::as_str) {
            let Ok(escaped) = serde_json::to_string(entry) else {
                continue;
            };
            if let Some(at) = file.text[from..].find(&escaped) {
                let line = file.line_of(from + at);
                if mentions(entry) {
                    p.lines.insert(line);
                    mentioned_paths(entry, &mut p.paths);
                }
            }
        }
    }
}

/// How a markdown line stands: a real read, or protective.
fn markdown(file: &InputFile, p: &mut Protection) {
    let lines: Vec<&str> = file.lines().map(|l| l.text).collect();
    for line in file.lines() {
        if !mentions(line.text) {
            continue;
        }
        let i = (line.number - 1) as usize;
        let own = is_match(&PROHIBITION_RE, line.text);
        let verb = is_match(&VERB_RE, line.text);
        let fenced = file.in_fenced(line.offset);
        // In a fence the line is code: a read verb there is a command, whatever a comment says.
        let protective = if (verb && sends(line.text) && !own) || (verb && fenced) {
            false
        } else if own || (!verb && !is_command_target(line.text)) {
            true
        } else if fenced {
            false
        } else {
            is_match(&PROHIBITION_RE, &context(&lines, i))
        };
        if protective {
            p.lines.insert(line.number);
            mentioned_paths(line.text, &mut p.paths);
        }
    }
}

/// True when the credential match on `line` is a command (keychain tool, environment dump), not
/// a path: a plain mention of those is still a use.
fn is_command_target(line: &str) -> bool {
    !CRED_RES.iter().take(PATH_TARGETS).any(|r| r.is_match(line)) && names_credential(line)
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn is_item(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with(['-', '*', '+'])
        || t.split_once(['.', ')'])
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn is_indented(s: &str) -> bool {
    s.starts_with([' ', '\t'])
}

/// The paragraph that introduces line `i` (the paragraph itself, or for a list item the one
/// before the list) and the nearest heading above it.
fn context(lines: &[&str], i: usize) -> String {
    let mut end = i;
    if is_item(lines[i]) || is_indented(lines[i]) {
        let mut j = i;
        let mut found = None;
        while j > 0 {
            j -= 1;
            let s = lines[j];
            if is_blank(s) || is_item(s) || is_indented(s) {
                continue;
            }
            found = Some(j);
            break;
        }
        match found {
            Some(j) => end = j,
            None => return String::new(),
        }
    }
    let mut start = end;
    while start > 0 && !is_blank(lines[start - 1]) {
        start -= 1;
    }
    let mut text = lines[start..=end].join("\n");
    if let Some(h) = lines[..start]
        .iter()
        .rev()
        .find(|l| l.trim_start().starts_with('#'))
    {
        text.push('\n');
        text.push_str(h);
    }
    text
}

/// A hook script that can block: its credential paths are the ones it guards.
fn hook_script(file: &InputFile, p: &mut Protection) {
    if !is_match(&BLOCK_RE, &file.text) {
        return;
    }
    for line in file.lines() {
        if !mentions(line.text) {
            continue;
        }
        let exfil = is_match(&VERB_RE, line.text) && sends(line.text);
        if exfil || is_match(&START_VERB_RE, line.text) {
            continue;
        }
        p.lines.insert(line.number);
        mentioned_paths(line.text, &mut p.paths);
    }
}

/// `line` with the contents of its string literals and its trailing comment blanked out: what is
/// left is the code that runs.
fn code_of(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                    out.push(c);
                }
            }
            None if c == '"' || c == '\'' || c == '`' => {
                quote = Some(c);
                out.push(c);
            }
            None if c == '#' => break,
            None if c == '/' && out.ends_with('/') => {
                out.pop();
                break;
            }
            None => out.push(c),
        }
    }
    out
}

/// Any other script (a guard shipped in a skill, its installer, its self-test): when it returns,
/// checks or writes deny decisions and sends nothing over the network, the credential paths in
/// its strings, comments and docstrings are the ones it guards. A path in the code itself, or a
/// read verb next to it, stays a finding.
fn guard_script(file: &InputFile, p: &mut Protection) {
    if !is_match(&GUARD_RE, &file.text) || file.lines().any(|l| sends(&code_of(l.text))) {
        return;
    }
    let mut in_docstring = false;
    for line in file.lines() {
        // A line that opens or closes a `"""` block is part of it; a block never runs.
        let fences = line.text.matches("\"\"\"").count() + line.text.matches("'''").count();
        let docstring = in_docstring || fences > 0;
        if fences % 2 == 1 {
            in_docstring = !in_docstring;
        }
        if !mentions(line.text) {
            continue;
        }
        let code = if docstring {
            String::new()
        } else {
            code_of(line.text)
        };
        if mentions(&code) || is_match(&VERB_RE, &code) {
            continue;
        }
        p.lines.insert(line.number);
        mentioned_paths(line.text, &mut p.paths);
    }
}

#[cfg(test)]
#[path = "protect_tests.rs"]
mod tests;
