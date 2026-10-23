// Ported from HarnessKit (crates/hk-core/src/scanner.rs, `infer_skill_permissions`,
// `infer_hook_permissions`, `mcp_stdio_profile`, `mcp_remote_profile` and the plugin inference,
// commit 9098728d22fd5c2ea6fb4864da19183b55cb33c4, Apache-2.0).
// Modified: databases are dropped; reach is read per file role for all ten artifact kinds (not
// only skills, hooks, MCP servers and plugins); Settings are read as JSON (hook commands, `env`
// keys, `permissions.allow` Bash entries, hook URLs); a subagent's `tools` granting Bash is shell;
// `*` network for npx, uvx, bunx and pipx; a remote MCP host loses its `user:pass@`; a URL host
// is read without userinfo; a path inside a URL is not a file; env is names only, never values;
// every item remembers the file and line that produced it, so rules can point at them.

//! What an artifact can reach: files, network hosts, shell commands and environment variable
//! names, read statically from its files. [`trace`] keeps the line behind every item;
//! [`infer`] is the plain [`Reach`].
//!
//! An item is *incidental* when it is not a capability the artifact adds by itself: a URL in
//! prose, a package runner's `*`, an MCP server's own launch command, host and header names.
//! Those still show in the reach; the `permission-combo` rule ignores them.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::input::{FileRole, InputFile};
use super::{excerpt, AuditInput, Reach};
use crate::artifact::ArtifactKind;

const EVIDENCE_CHARS: usize = 120;
const RUNNERS: [&str; 4] = ["npx", "uvx", "bunx", "pipx"];
const SHELL_FENCES: [&str; 4] = ["bash", "shell", "sh", "zsh"];

/// One reach item and where it came from. `evidence` never holds a secret or an env value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    pub value: String,
    pub file: String,
    /// 1-based; `None` when the item belongs to the file (or the MCP definition) as a whole.
    pub line: Option<u32>,
    pub evidence: String,
    /// See the module docs.
    pub incidental: bool,
}

/// The reach and the sources behind its network, shell and env lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Trace {
    pub reach: Reach,
    pub network: Vec<Source>,
    pub shell: Vec<Source>,
    pub env: Vec<Source>,
}

/// The reach of the artifact behind `input`, with sorted lists.
pub(crate) fn infer(input: &AuditInput) -> Reach {
    input.trace().reach.clone()
}

/// Reads the whole input once. Use [`AuditInput::trace`] to share the result.
pub(super) fn trace(input: &AuditInput) -> Trace {
    let mut t = Tracer::default();
    for file in &input.files {
        t.file(input, file);
    }
    if input.kind == ArtifactKind::Agent {
        t.agent_tools(input);
    }
    let mut out = Trace {
        reach: Reach {
            files: t.files,
            network: t.network.iter().map(|s| s.value.clone()).collect(),
            shell: t.shell.iter().map(|s| s.value.clone()).collect(),
            env: t.env.iter().map(|s| s.value.clone()).collect(),
        },
        network: t.network,
        shell: t.shell,
        env: t.env,
    };
    out.reach.normalize();
    out
}

fn compile(pattern: &str) -> Option<Regex> {
    Regex::new(pattern).ok()
}

static SENSITIVE_PATHS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    compile(concat!(
        r"(?:",
        // Unix: ~/foo, /etc/foo, /home/user/foo, etc.
        r"(?:~|/(?:etc|home/\w+|tmp|var|opt|usr/local|Library|Applications))/[\w.\-/]+",
        r"|",
        // Windows: C:\Users\foo, D:\Projects\bar
        r"[A-Za-z]:\\[\w.\-\\]+",
        r"|",
        // Windows env vars: %APPDATA%\foo, %USERPROFILE%\bar
        r"%[A-Za-z_]+%[\\/][\w.\-\\/]+",
        r")",
    ))
});

/// The host of a URL, without any `user:pass@` in front of it.
static URL_HOST: LazyLock<Option<Regex>> = LazyLock::new(|| {
    compile(r#"(?i)https?://(?:[^\s/@:"'<>()\[\]]*(?::[^\s/@"'<>]*)?@)?([\w.\-]+)"#)
});

static ENV_ASSIGNMENT: LazyLock<Option<Regex>> =
    LazyLock::new(|| compile(r"^[A-Za-z_][A-Za-z0-9_]*="));

#[derive(Default)]
struct Tracer {
    files: Vec<String>,
    network: Vec<Source>,
    shell: Vec<Source>,
    env: Vec<Source>,
}

/// Where the items being added come from.
struct At<'a> {
    file: &'a str,
    line: Option<u32>,
    evidence: &'a str,
}

impl Tracer {
    fn net(&mut self, value: &str, at: &At<'_>, incidental: bool) {
        push(&mut self.network, value, at, incidental);
    }

    fn sh(&mut self, value: &str, at: &At<'_>, incidental: bool) {
        push(&mut self.shell, value, at, incidental);
    }

    fn env(&mut self, value: &str, at: &At<'_>, incidental: bool) {
        push(&mut self.env, value, at, incidental);
    }

    fn file(&mut self, input: &AuditInput, file: &InputFile) {
        match file.role {
            FileRole::Settings if self.settings(file) => {}
            FileRole::McpDefinition if self.mcp(input, file) => {}
            _ => self.text(file),
        }
    }

    /// Items of a text file: URLs and paths on every line, shell for fenced shell blocks and
    /// hook scripts.
    fn text(&mut self, file: &InputFile) {
        let shell_spans: Vec<_> = file
            .fenced_spans()
            .iter()
            .filter(|r| is_shell_fence(file, r.start))
            .collect();
        for line in file.lines() {
            let evidence = excerpt(line.text, EVIDENCE_CHARS);
            let at = At {
                file: &file.path,
                line: Some(line.number),
                evidence: &evidence,
            };
            let in_code = file.in_fenced(line.offset);
            let incidental = match file.role {
                FileRole::Markdown => !in_code,
                FileRole::Json | FileRole::Other => true,
                _ => false,
            };
            self.urls_and_paths(line.text, &at, incidental);
            if shell_spans.iter().any(|r| r.contains(&line.offset)) {
                self.commands(line.text, &at);
            }
        }
        if file.role == FileRole::HookScript {
            let name = file.path.rsplit('/').next().unwrap_or(&file.path);
            let at = At {
                file: &file.path,
                line: None,
                evidence: &excerpt(&format!("runs {name}"), EVIDENCE_CHARS),
            };
            self.sh(name, &at, false);
        }
    }

    fn urls_and_paths(&mut self, text: &str, at: &At<'_>, incidental: bool) {
        if let Some(re) = URL_HOST.as_ref() {
            for caps in re.captures_iter(text) {
                if let Some(host) = caps.get(1) {
                    let host = host.as_str().trim_end_matches('.').to_ascii_lowercase();
                    if !host.is_empty() {
                        self.net(&host, at, incidental);
                    }
                }
            }
        }
        if let Some(re) = SENSITIVE_PATHS.as_ref() {
            for m in re.find_iter(text) {
                // A path inside a URL or a longer word is not a file on this machine.
                let glued = text[..m.start()].chars().next_back().is_some_and(|c| {
                    m.as_str().starts_with('/') && (c.is_alphanumeric() || ".:-_".contains(c))
                });
                if !glued {
                    self.files.push(m.as_str().trim_end_matches('.').to_owned());
                }
            }
        }
    }

    /// Shell commands of one line or command string: the first word of each pipeline step.
    fn commands(&mut self, text: &str, at: &At<'_>) {
        let trimmed = text.trim();
        if trimmed.starts_with('#') {
            return;
        }
        let steps = trimmed
            .replace("&&", ";")
            .replace("||", ";")
            .replace('|', ";");
        for step in steps.split(';') {
            if let Some(command) = first_command(step) {
                if RUNNERS.contains(&command.as_str()) {
                    self.net("*", at, true);
                }
                self.sh(&command, at, false);
            }
        }
    }

    /// A Settings or hooks JSON file. Returns false when it does not parse (read as text then).
    fn settings(&mut self, file: &InputFile) -> bool {
        let Ok(root) = serde_json::from_str::<Value>(&file.text) else {
            return false;
        };
        let mut items = Vec::new();
        settings_items(&root, &mut items);
        for item in items {
            let (text, is_command) = match &item {
                SettingsItem::Command(c) => (c.as_str(), true),
                SettingsItem::Url(u) => (u.as_str(), false),
            };
            let first = text.lines().next().unwrap_or(text);
            let evidence = excerpt(first, EVIDENCE_CHARS);
            let at = At {
                file: &file.path,
                line: json_line(file, text),
                evidence: &evidence,
            };
            self.urls_and_paths(text, &at, false);
            if is_command {
                for part in text.lines() {
                    self.commands(part, &at);
                }
            }
        }
        if let Some(Value::Object(env)) = root.get("env") {
            for key in env.keys() {
                let at = At {
                    file: &file.path,
                    line: key_line(file, key),
                    evidence: &excerpt(&format!("env {key}"), EVIDENCE_CHARS),
                };
                self.env(key, &at, false);
            }
        }
        let allow = root.pointer("/permissions/allow").and_then(Value::as_array);
        for entry in allow.into_iter().flatten().filter_map(Value::as_str) {
            if let Some(command) = bash_grant(entry) {
                let at = At {
                    file: &file.path,
                    line: json_line(file, entry),
                    evidence: &excerpt(entry, EVIDENCE_CHARS),
                };
                self.sh(&command, &at, false);
            }
        }
        true
    }

    /// An MCP definition: one server, or a `mcpServers` map, or a bare map of servers. Returns
    /// false when it is not JSON with servers (read as text then).
    fn mcp(&mut self, input: &AuditInput, file: &InputFile) -> bool {
        let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&file.text) else {
            return false;
        };
        let synthetic = input.kind == ArtifactKind::Mcp && file.path == input.path;
        let is_server = |m: &Map<String, Value>| m.contains_key("command") || m.contains_key("url");
        let mut servers: Vec<(&str, &Map<String, Value>)> = Vec::new();
        if let Some(Value::Object(map)) = root.get("mcpServers") {
            servers.extend(
                map.iter()
                    .filter_map(|(k, v)| Some((k.as_str(), v.as_object()?))),
            );
        } else if is_server(&root) {
            servers.push((input.name.as_str(), &root));
        } else {
            servers.extend(
                root.iter()
                    .filter_map(|(k, v)| Some((k.as_str(), v.as_object()?)))
                    .filter(|(_, m)| is_server(m)),
            );
        }
        for (name, server) in &servers {
            let line = if synthetic || root.contains_key("command") || root.contains_key("url") {
                None
            } else {
                key_line(file, name)
            };
            self.server(
                name,
                server,
                &At {
                    file: &file.path,
                    line,
                    evidence: "",
                },
            );
        }
        true
    }

    fn server(&mut self, name: &str, server: &Map<String, Value>, at: &At<'_>) {
        let text = |key: &str| server.get(key).and_then(Value::as_str);
        let evidence = |what: &str| excerpt(&format!("{name}: {what}"), EVIDENCE_CHARS);
        if let Some(command) = text("command").and_then(first_command_word) {
            let e = evidence(&format!("command {command}"));
            let at = At {
                evidence: &e,
                ..*at
            };
            self.sh(&command, &at, true);
            if RUNNERS.contains(&command.as_str()) {
                self.net("*", &at, true);
            }
        }
        for arg in server
            .get("args")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(arg) = arg.as_str() else { continue };
            let e = evidence(&format!("argument {arg}"));
            let at = At {
                evidence: &e,
                ..*at
            };
            if let Some(re) = URL_HOST.as_ref() {
                for host in re.captures_iter(arg).filter_map(|c| c.get(1)) {
                    self.net(&host.as_str().to_ascii_lowercase(), &at, false);
                }
            }
            let absolute = (arg.starts_with('/') && !arg.starts_with("//"))
                || arg.starts_with("~/")
                || is_windows_absolute(arg);
            if absolute && !arg.contains(char::is_whitespace) {
                self.files.push(arg.to_owned());
            }
        }
        if let Some(url) = text("url") {
            let host = remote_host(url);
            let e = evidence(&format!("remote {host}"));
            self.net(
                &host,
                &At {
                    evidence: &e,
                    ..*at
                },
                true,
            );
        }
        for (field, incidental) in [("env", false), ("headers", true)] {
            if let Some(Value::Object(map)) = server.get(field) {
                for key in map.keys() {
                    let e = evidence(&format!("{field} {key}"));
                    self.env(
                        key,
                        &At {
                            evidence: &e,
                            ..*at
                        },
                        incidental,
                    );
                }
            }
        }
    }

    /// A subagent whose `tools` list grants Bash.
    fn agent_tools(&mut self, input: &AuditInput) {
        let tools: Vec<String> = match input.metadata.get("tools") {
            Some(Value::String(s)) => s.split(',').map(|t| t.trim().to_owned()).collect(),
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => return,
        };
        let line = input.file(&input.path).and_then(|f| {
            f.lines()
                .take(60)
                .find(|l| l.text.trim_start().starts_with("tools:"))
                .map(|l| l.number)
        });
        for tool in tools {
            if let Some(command) = bash_grant(&tool) {
                let at = At {
                    file: &input.path,
                    line,
                    evidence: &excerpt(&format!("tools: {tool}"), EVIDENCE_CHARS),
                };
                self.sh(&command, &at, false);
            }
        }
    }
}

fn push(list: &mut Vec<Source>, value: &str, at: &At<'_>, incidental: bool) {
    list.push(Source {
        value: value.to_owned(),
        file: at.file.to_owned(),
        line: at.line,
        evidence: at.evidence.to_owned(),
        incidental,
    });
}

enum SettingsItem {
    Command(String),
    Url(String),
}

/// Hook `command` strings and `url` values, outside `permissions` and `env`.
fn settings_items(value: &Value, out: &mut Vec<SettingsItem>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match (key.as_str(), child) {
                    ("permissions" | "env", _) => {}
                    ("command", Value::String(s)) => out.push(SettingsItem::Command(s.clone())),
                    ("url", Value::String(s)) => out.push(SettingsItem::Url(s.clone())),
                    _ => settings_items(child, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| settings_items(v, out)),
        _ => {}
    }
}

/// The line where a JSON string value starts, found by its escaped form.
fn json_line(file: &InputFile, value: &str) -> Option<u32> {
    let first = value.lines().next().unwrap_or(value);
    let escaped = serde_json::to_string(first).ok()?;
    let needle = escaped.trim_matches('"');
    file.text.find(needle).map(|at| file.line_of(at))
}

/// The line of a JSON key.
fn key_line(file: &InputFile, key: &str) -> Option<u32> {
    let needle = format!("{}:", serde_json::to_string(key).ok()?);
    let at = file.text.find(&needle).or_else(|| {
        let loose = serde_json::to_string(key).ok()?;
        file.text.find(&loose)
    })?;
    Some(file.line_of(at))
}

/// True when the fence that opens the block whose content starts at `content_start` names a
/// shell.
fn is_shell_fence(file: &InputFile, content_start: usize) -> bool {
    let fence = file.line_text(file.line_of(content_start).saturating_sub(1));
    let info = fence.trim_start().trim_start_matches(['`', '~']).trim();
    let word = info.split_whitespace().next().unwrap_or_default();
    SHELL_FENCES.contains(&word.to_ascii_lowercase().as_str())
}

/// `Bash`, `Bash(*)` and `Bash(:*)` grant every command (`*`); `Bash(git status:*)` grants `git`.
fn bash_grant(entry: &str) -> Option<String> {
    let entry = entry.trim();
    let rest = entry.strip_prefix("Bash")?;
    if rest.is_empty() {
        return Some("*".to_owned());
    }
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?.trim();
    let inner = inner.strip_suffix(":*").unwrap_or(inner).trim();
    if inner.is_empty() || inner == "*" {
        return Some("*".to_owned());
    }
    first_command(inner).or_else(|| Some("*".to_owned()))
}

/// The command a shell line starts with: leading `VAR=value` words are skipped, the path is cut
/// to its file name. `None` for variables, subshells and the like.
fn first_command(step: &str) -> Option<String> {
    let step = step.trim().trim_start_matches("$ ");
    let word = step
        .split_whitespace()
        .find(|w| !ENV_ASSIGNMENT.as_ref().is_some_and(|re| re.is_match(w)))?;
    first_command_word(word)
}

fn first_command_word(word: &str) -> Option<String> {
    let word = word.split_whitespace().next()?.trim_matches(['"', '\'']);
    if word.contains(['`', '(']) {
        return None;
    }
    let name = Path::new(word).file_name()?.to_string_lossy().into_owned();
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || "._-+@".contains(c));
    valid.then_some(name)
}

/// `host[:port]` of a remote MCP URL without any `user:pass@`; `*` when there is none.
fn remote_host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default().trim();
    if host.is_empty() {
        "*".to_owned()
    } else {
        host.to_ascii_lowercase()
    }
}

fn is_windows_absolute(arg: &str) -> bool {
    let mut chars = arg.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(d), Some(':'), Some('\\' | '/')) if d.is_ascii_alphabetic()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input(kind: ArtifactKind, files: &[(&str, &str)]) -> AuditInput {
        let path = files.first().map_or("x", |f| f.0);
        AuditInput::with_files(kind, "x", path, files)
    }

    fn reach(kind: ArtifactKind, files: &[(&str, &str)]) -> Reach {
        infer(&input(kind, files))
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_skill_reaches_paths_hosts_and_fenced_shell_commands() {
        let text = "# s\nSee https://docs.example.com/x for more.\nConfig lives in ~/.config/tool/rc.\n\
                    ```bash\n# setup\ncurl -s https://api.example.com/v1 | jq .\nnpx prettier .\n./scripts/run.sh\n```\n\
                    ```python\nprint(1)\n```\n";
        let r = reach(ArtifactKind::Skill, &[("s/SKILL.md", text)]);
        assert_eq!(
            r.network,
            strs(&["*", "api.example.com", "docs.example.com"])
        );
        assert_eq!(r.shell, strs(&["curl", "jq", "npx", "run.sh"]));
        assert_eq!(r.files, strs(&["~/.config/tool/rc"]));
        assert!(r.env.is_empty());
    }

    #[test]
    fn only_fenced_shell_blocks_count_as_shell_and_sources_carry_lines() {
        let text = "```text\nrm -rf x\n```\nuse curl https://a.example.com\n```sh\nwget https://b.example.com/f\n```\n";
        let i = input(ArtifactKind::Skill, &[("s/SKILL.md", text)]);
        let t = i.trace();
        assert_eq!(t.reach.shell, strs(&["wget"]));
        let hosts: Vec<_> = t
            .network
            .iter()
            .map(|s| (s.value.as_str(), s.line, s.incidental))
            .collect();
        assert_eq!(
            hosts,
            [
                ("a.example.com", Some(4), true),
                ("b.example.com", Some(6), false)
            ]
        );
    }

    #[test]
    fn a_url_never_becomes_a_file_and_userinfo_is_never_a_host() {
        let text = "get https://user:pw@host.example.com/etc/passwd and /etc/hosts\n";
        let r = reach(ArtifactKind::Rule, &[("r.md", text)]);
        assert_eq!(r.network, strs(&["host.example.com"]));
        assert_eq!(r.files, strs(&["/etc/hosts"]));
    }

    #[test]
    fn a_hook_script_runs_itself_and_reaches_its_urls() {
        let script = "#!/bin/sh\ncurl -fsS https://hooks.example.com/notify\ncat /home/me/notes\n";
        let r = reach(
            ArtifactKind::Settings,
            &[("s.json", "{}"), ("hooks/notify.sh", script)],
        );
        assert_eq!(r.shell, strs(&["notify.sh"]));
        assert_eq!(r.network, strs(&["hooks.example.com"]));
        assert_eq!(r.files, strs(&["/home/me/notes"]));
    }

    const SETTINGS: &str = r#"{
  "env": {
    "API_TOKEN": "super-secret-value-123",
    "LOG": "debug"
  },
  "hooks": {
    "PostToolUse": [
      { "hooks": [ { "type": "command", "command": "FOO=1 /usr/local/bin/fmt --write && curl https://telemetry.example.com/ping" } ] }
    ]
  },
  "permissions": {
    "allow": ["Bash(git status:*)", "Bash(npm run test)", "Read(./x)", "Bash"],
    "deny": ["Bash(curl:*)"]
  }
}"#;

    #[test]
    fn settings_reach_hooks_allow_entries_and_env_names() {
        let i = input(
            ArtifactKind::Settings,
            &[(".claude/settings.json", SETTINGS)],
        );
        let t = i.trace();
        assert_eq!(t.reach.env, strs(&["API_TOKEN", "LOG"]));
        assert_eq!(t.reach.shell, strs(&["*", "curl", "fmt", "git", "npm"]));
        assert_eq!(t.reach.network, strs(&["telemetry.example.com"]));
        assert_eq!(t.reach.files, strs(&["/usr/local/bin/fmt"]));
        let line_of = |list: &[Source], v: &str| list.iter().find(|s| s.value == v).map(|s| s.line);
        assert_eq!(line_of(&t.env, "API_TOKEN"), Some(Some(3)));
        assert_eq!(line_of(&t.network, "telemetry.example.com"), Some(Some(8)));
        assert_eq!(line_of(&t.shell, "git"), Some(Some(12)));
    }

    #[test]
    fn no_env_value_reaches_the_reach_or_its_evidence() {
        let i = input(
            ArtifactKind::Settings,
            &[(".claude/settings.json", SETTINGS)],
        );
        let t = i.trace();
        let all = format!("{:?}{:?}", t.reach, (&t.network, &t.shell, &t.env));
        assert!(!all.contains("super-secret-value-123"), "{all}");
    }

    fn mcp_input(name: &str, def: &Value) -> AuditInput {
        let text = serde_json::to_string_pretty(def).unwrap_or_default();
        let mut i = AuditInput::with_files(
            ArtifactKind::Mcp,
            name,
            ".mcp.json#x",
            &[(".mcp.json#x", &text)],
        );
        i.files.clear();
        i.files
            .push(InputFile::new(".mcp.json#x", FileRole::McpDefinition, text));
        i
    }

    #[test]
    fn a_local_mcp_server_reaches_its_command_env_names_and_args() {
        let i = mcp_input(
            "db",
            &json!({"command": "npx", "args": ["-y", "pkg", "/srv/data", "https://u:p@feed.example.com/x"],
                    "env": {"DB_PASSWORD": "hunter2hunter2", "MODE": "ro"}}),
        );
        let t = i.trace();
        assert_eq!(t.reach.shell, strs(&["npx"]));
        assert_eq!(t.reach.network, strs(&["*", "feed.example.com"]));
        assert_eq!(t.reach.env, strs(&["DB_PASSWORD", "MODE"]));
        assert_eq!(t.reach.files, strs(&["/srv/data"]));
        assert!(!format!("{t:?}").contains("hunter2"));
        // The launch command and the runner's `*` are incidental, the URL argument is not.
        let loud: Vec<_> = t
            .network
            .iter()
            .filter(|s| !s.incidental)
            .map(|s| s.value.as_str())
            .collect();
        assert_eq!(loud, ["feed.example.com"]);
        assert!(t.shell.iter().all(|s| s.incidental));
        assert!(t.network.iter().all(|s| s.line.is_none()));
    }

    #[test]
    fn a_remote_mcp_server_reaches_its_host_without_credentials_and_its_header_names() {
        let i = mcp_input(
            "api",
            &json!({"type": "http", "url": "https://me:pw@Api.Example.com:8443/mcp?x=1",
                    "headers": {"Authorization": "Bearer abcdefghijkl"}}),
        );
        let t = i.trace();
        assert_eq!(t.reach.network, strs(&["api.example.com:8443"]));
        assert_eq!(t.reach.env, strs(&["Authorization"]));
        assert!(t.reach.shell.is_empty());
        assert!(!format!("{t:?}").contains("abcdefghijkl") && !format!("{t:?}").contains("pw@"));
        assert_eq!(remote_host("https://x"), "x");
        assert_eq!(remote_host("sse://"), "*");
    }

    #[test]
    fn a_plugin_mcp_file_maps_servers_and_points_at_their_lines() {
        let mcp = "{\n  \"mcpServers\": {\n    \"a\": {\"command\": \"node\", \"args\": [\"s.js\"]},\n    \"b\": {\"url\": \"https://b.example.com\", \"headers\": {\"X-Key\": \"v\"}}\n  }\n}\n";
        let hooks = "{\"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"${CLAUDE_PLUGIN_ROOT}/hooks/stop.sh\"}]}]}}";
        let i = input(
            ArtifactKind::Plugin,
            &[
                ("p/.mcp.json", mcp),
                ("p/hooks/hooks.json", hooks),
                ("p/README.md", "docs https://p.example.com\n"),
            ],
        );
        let t = i.trace();
        assert_eq!(t.reach.shell, strs(&["node", "stop.sh"]));
        assert_eq!(t.reach.network, strs(&["b.example.com", "p.example.com"]));
        assert_eq!(t.reach.env, strs(&["X-Key"]));
        let host = t
            .network
            .iter()
            .find(|s| s.value == "b.example.com")
            .map(|s| (s.file.as_str(), s.line));
        assert_eq!(host, Some(("p/.mcp.json", Some(4))));
    }

    #[test]
    fn an_agent_granting_bash_reaches_the_shell() {
        let mut i = input(
            ArtifactKind::Agent,
            &[("a.md", "---\nname: a\ntools: Read, Bash\n---\nbody\n")],
        );
        i.metadata.insert("tools".into(), json!("Read, Bash"));
        let t = i.trace();
        assert_eq!(t.reach.shell, strs(&["*"]));
        assert_eq!(t.shell.first().map(|s| s.line), Some(Some(3)));
        let mut j = input(ArtifactKind::Agent, &[("a.md", "x")]);
        j.metadata
            .insert("tools".into(), json!(["Read", "Bash(git diff:*)"]));
        assert_eq!(j.trace().reach.shell, strs(&["git"]));
        let mut k = input(ArtifactKind::Agent, &[("a.md", "x")]);
        k.metadata.insert("tools".into(), json!("Read, Grep"));
        assert!(k.trace().reach.is_empty());
    }

    #[test]
    fn unparsable_settings_fall_back_to_text_and_an_empty_input_reaches_nothing() {
        let r = reach(
            ArtifactKind::Settings,
            &[("s.json", "{ not json https://t.example.com")],
        );
        assert_eq!(r.network, strs(&["t.example.com"]));
        assert!(reach(ArtifactKind::Skill, &[("s/SKILL.md", "hello")]).is_empty());
    }

    #[test]
    fn grants_and_commands_are_parsed() {
        assert_eq!(bash_grant("Bash(*)").as_deref(), Some("*"));
        assert_eq!(bash_grant("Bash(:*)").as_deref(), Some("*"));
        assert_eq!(bash_grant("Bash(npm run build:*)").as_deref(), Some("npm"));
        assert_eq!(bash_grant("Read"), None);
        assert_eq!(bash_grant("BashOutput"), None);
        assert_eq!(
            first_command("  $ A=1 ./bin/tool x").as_deref(),
            Some("tool")
        );
        assert_eq!(first_command("$HOME/x").as_deref(), Some("x"));
        assert_eq!(first_command("$CMD --x"), None);
        assert_eq!(first_command("A=1"), None);
    }
}
