// Ported from HarnessKit (crates/hk-core/src/auditor/rules/mcp.rs and permissions.rs, commit
// 9098728d22fd5c2ea6fb4864da19183b55cb33c4, Apache-2.0).
// Modified: an MCP server is read from its definition JSON (`command`, `args`, `env`, `url`) so
// plugin `.mcp.json` files with several servers work too; `broad-permissions` also covers a
// filesystem server rooted at `/` or `~`, Settings that allow every tool and a subagent granted
// every tool; `mcp-command-injection` covers `$(` and backticks in the command and its
// arguments; evidence is redacted and never includes environment values.

//! Rules `broad-permissions` and `mcp-command-injection`, and the MCP server reader that
//! `supply_chain.rs` shares.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::artifact::ArtifactKind;
use crate::audit::input::InputFile;
use crate::audit::{excerpt, AuditInput, AuditRule, Finding, Severity};

const EVIDENCE_CHARS: usize = 160;

/// One MCP server definition, as read from a definition file.
pub(super) struct Server {
    /// `.mcp.json#<name>` for a server inside a `mcpServers` map, else the file's path.
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub url: String,
}

impl Server {
    /// The command line as one string, for evidence.
    pub fn command_line(&self) -> String {
        std::iter::once(self.command.as_str())
            .chain(self.args.iter().map(String::as_str))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn server_from(label: String, def: &Map<String, Value>) -> Server {
    let text = |key: &str| {
        def.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let args = def
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let env = def
        .get("env")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
        .collect();
    Server {
        label,
        command: text("command"),
        args,
        env,
        url: text("url"),
    }
}

/// The servers a definition file holds: one per entry of a `mcpServers` map, or the file itself
/// when it is a single server's definition.
pub(super) fn servers(file: &InputFile) -> Vec<Server> {
    if !file.is_mcp_definition() {
        return Vec::new();
    }
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&file.text) else {
        return Vec::new();
    };
    match root.get("mcpServers").and_then(Value::as_object) {
        Some(map) => map
            .iter()
            .filter_map(|(name, def)| {
                def.as_object()
                    .map(|d| server_from(format!("{}#{name}", file.path), d))
            })
            .collect(),
        None => vec![server_from(file.path.clone(), &root)],
    }
}

/// The line where a JSON key or string value first appears (`value` is quoted for the search).
pub(super) fn json_line(file: &InputFile, value: &str) -> Option<u32> {
    let needle = serde_json::to_string(value).ok()?;
    file.text.find(&needle).map(|at| file.line_of(at))
}

pub(super) fn finding(
    rule: &'static str,
    severity: Severity,
    message: String,
    file: &str,
    line: Option<u32>,
    evidence: &str,
) -> Finding {
    Finding {
        rule: rule.into(),
        severity,
        message,
        file: file.to_owned(),
        line,
        evidence: excerpt(evidence, EVIDENCE_CHARS),
    }
}

// ---------------------------------------------------------------------------------------------
// broad-permissions
// ---------------------------------------------------------------------------------------------

const BIND_FLAGS: [&str; 6] = [
    "--host",
    "--bind",
    "--listen",
    "--address",
    "--hostname",
    "-H",
];

fn is_all_interfaces(value: &str) -> bool {
    let v = value.trim();
    v == "*" || v == "[::]" || v.starts_with("0.0.0.0")
}

/// Whether a server's arguments or environment bind every network interface.
fn binds_everywhere(server: &Server) -> bool {
    let args_bind = server.args.iter().enumerate().any(|(i, arg)| {
        if arg.contains("0.0.0.0") || arg.contains("[::]") {
            return true;
        }
        if let Some((flag, value)) = arg.split_once('=') {
            return BIND_FLAGS.contains(&flag) && is_all_interfaces(value);
        }
        BIND_FLAGS.contains(&arg.as_str())
            && server
                .args
                .get(i + 1)
                .is_some_and(|next| is_all_interfaces(next))
    });
    args_bind
        || server.env.iter().any(|(key, value)| {
            let key = key.to_ascii_uppercase();
            ["HOST", "BIND", "ADDR", "LISTEN"]
                .iter()
                .any(|part| key.contains(part))
                && is_all_interfaces(value)
        })
}

fn is_filesystem_server(server: &Server) -> bool {
    let all = format!("{} {}", server.command, server.args.join(" ")).to_ascii_lowercase();
    all.contains("server-filesystem") || all.contains("mcp-server-filesystem")
}

fn is_root_path(arg: &str) -> bool {
    matches!(
        arg.trim_end_matches('/'),
        "" | "~" | "$HOME" | "${HOME}" | "%USERPROFILE%"
    ) && !arg.is_empty()
}

fn filesystem_root(server: &Server) -> Option<&str> {
    if !is_filesystem_server(server) {
        return None;
    }
    server
        .args
        .iter()
        .map(String::as_str)
        .find(|a| !a.starts_with('-') && is_root_path(a))
}

static ALL_TOOLS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*tools\s*:\s*(?:["']?\*["']?|\[\s*["']?\*["']?\s*\])\s*(?:#.*)?$"#).ok()
});

/// The frontmatter line of a subagent that grants every tool.
fn agent_every_tool(file: &InputFile) -> Option<(u32, String)> {
    let re = ALL_TOOLS.as_ref()?;
    let mut lines = file.lines();
    if lines.next()?.text.trim_end() != "---" {
        return None;
    }
    for line in lines {
        if line.text.trim_end() == "---" {
            return None;
        }
        if re.is_match(line.text) {
            return Some((line.number, line.text.to_owned()));
        }
    }
    None
}

fn is_subagent_file(input: &AuditInput, file: &InputFile) -> bool {
    file.is_markdown()
        && (input.kind == ArtifactKind::Agent || file.path.split('/').any(|c| c == "agents"))
}

/// A server that binds every interface or can read the whole disk, Settings that allow every
/// tool, and a subagent that is granted every tool.
pub(crate) struct BroadPermissions;

impl AuditRule for BroadPermissions {
    fn id(&self) -> &'static str {
        "broad-permissions"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<Finding> {
        let mut out = Vec::new();
        let mut push = |message: &str, file: &str, line: Option<u32>, evidence: &str| {
            out.push(finding(
                "broad-permissions",
                Severity::High,
                format!("Broad permissions: {message}"),
                file,
                line,
                evidence,
            ));
        };
        for file in &input.files {
            for server in servers(file) {
                if binds_everywhere(&server) {
                    push(
                        "the MCP server listens on every network interface (0.0.0.0 or *)",
                        &server.label,
                        None,
                        &server.command_line(),
                    );
                }
                if let Some(root) = filesystem_root(&server) {
                    push(
                        &format!("the filesystem MCP server is given `{root}`, the whole disk or home folder"),
                        &server.label,
                        None,
                        &server.command_line(),
                    );
                }
            }
            if file.is_settings() {
                if let Some(entry) = every_tool_entry(file) {
                    push(
                        "permissions.allow grants every tool",
                        &file.path,
                        json_line(file, &entry),
                        &format!("allow: {entry}"),
                    );
                }
            }
            if is_subagent_file(input, file) {
                if let Some((line, text)) = agent_every_tool(file) {
                    push(
                        "the subagent is granted every tool",
                        &file.path,
                        Some(line),
                        &text,
                    );
                }
            }
        }
        out
    }
}

/// The `permissions.allow` entry that allows every tool, if there is one.
fn every_tool_entry(file: &InputFile) -> Option<String> {
    let json = serde_json::from_str::<Value>(&file.text).ok()?;
    json.pointer("/permissions/allow")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|e| matches!(e.trim(), "*" | "**"))
        .map(str::to_owned)
}

// ---------------------------------------------------------------------------------------------
// mcp-command-injection
// ---------------------------------------------------------------------------------------------

/// Command substitution (`$(` or backticks) in an MCP server's command or arguments.
pub(crate) struct McpCommandInjection;

impl AuditRule for McpCommandInjection {
    fn id(&self) -> &'static str {
        "mcp-command-injection"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<Finding> {
        let mut out = Vec::new();
        for file in &input.files {
            for server in servers(file) {
                let injected = std::iter::once(&server.command)
                    .chain(&server.args)
                    .find(|a| a.contains("$(") || a.contains('`'));
                if let Some(arg) = injected {
                    out.push(finding(
                        "mcp-command-injection",
                        Severity::High,
                        "MCP command injection: the server's command or arguments run a command substitution ($( ) or backticks)".into(),
                        &server.label,
                        None,
                        arg,
                    ));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp(json: &str) -> AuditInput {
        AuditInput::with_files(
            ArtifactKind::Mcp,
            "db",
            ".mcp.json#db",
            &[(".mcp.json", json)],
        )
    }

    fn settings(json: &str) -> AuditInput {
        AuditInput::with_files(
            ArtifactKind::Settings,
            "settings",
            ".claude/settings.json",
            &[(".claude/settings.json", json)],
        )
    }

    fn agent(text: &str) -> AuditInput {
        AuditInput::with_files(
            ArtifactKind::Agent,
            "a",
            "agents/a.md",
            &[("agents/a.md", text)],
        )
    }

    fn broad(input: &AuditInput) -> Vec<Finding> {
        BroadPermissions.check(input)
    }

    #[test]
    fn a_server_binding_every_interface_triggers() {
        for json in [
            r#"{"command":"node","args":["s.js","--host","0.0.0.0"]}"#,
            r#"{"command":"node","args":["s.js","--host=*"]}"#,
            r#"{"command":"node","args":["s.js","0.0.0.0:8080"]}"#,
            r#"{"command":"node","args":["s.js"],"env":{"BIND_ADDRESS":"0.0.0.0"}}"#,
        ] {
            let f = broad(&mcp(json));
            assert_eq!(f.len(), 1, "{json}");
            assert_eq!(f[0].rule, "broad-permissions");
            assert_eq!(f[0].severity, Severity::High);
            assert_eq!(f[0].file, ".mcp.json");
        }
    }

    #[test]
    fn a_loopback_server_does_not_trigger() {
        for json in [
            r#"{"command":"node","args":["s.js","--host","127.0.0.1"]}"#,
            r#"{"command":"node","args":["s.js","--host=localhost"]}"#,
            r#"{"command":"node","args":["s.js"],"env":{"NAME":"*"}}"#,
        ] {
            assert!(broad(&mcp(json)).is_empty(), "{json}");
        }
    }

    #[test]
    fn a_filesystem_server_on_the_whole_disk_or_home_triggers() {
        for root in ["/", "~", "~/", "$HOME"] {
            let json = format!(
                r#"{{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","{root}"]}}"#
            );
            let f = broad(&mcp(&json));
            assert_eq!(f.len(), 1, "{root}");
            assert!(f[0].message.contains("filesystem"));
        }
    }

    #[test]
    fn a_filesystem_server_on_a_project_folder_does_not_trigger() {
        let json = r#"{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/home/me/project","./docs"]}"#;
        assert!(broad(&mcp(json)).is_empty());
        // `/` as an argument of another server is not a filesystem root.
        assert!(broad(&mcp(r#"{"command":"node","args":["s.js","/"]}"#)).is_empty());
    }

    #[test]
    fn servers_in_an_mcp_servers_map_are_read_and_named() {
        let json = r#"{"mcpServers":{"ok":{"command":"node","args":["a.js"]},
            "open":{"command":"node","args":["b.js","--bind","0.0.0.0"]}}}"#;
        let f = broad(&mcp(json));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].file, ".mcp.json#open");
    }

    #[test]
    fn settings_allowing_every_tool_trigger_with_their_line() {
        let f = broad(&settings(
            "{\n  \"permissions\": {\n    \"allow\": [\"*\"]\n  }\n}",
        ));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].line, Some(3));
        assert!(broad(&settings(
            r#"{"permissions":{"allow":["Read","Bash(npm test:*)"]}}"#
        ))
        .is_empty());
        assert!(broad(&settings(r#"{"permissions":{"deny":["*"]}}"#)).is_empty());
    }

    #[test]
    fn a_subagent_with_every_tool_triggers() {
        for tools in ["tools: \"*\"", "tools: *", "tools: [\"*\"]"] {
            let f = broad(&agent(&format!("---\nname: a\n{tools}\n---\nbody\n")));
            assert_eq!(f.len(), 1, "{tools}");
            assert_eq!(f[0].line, Some(3));
        }
    }

    #[test]
    fn a_subagent_with_listed_or_default_tools_does_not_trigger() {
        assert!(broad(&agent("---\nname: a\ntools: Read, Grep\n---\n")).is_empty());
        assert!(broad(&agent("---\nname: a\n---\ntools: *\n")).is_empty());
        assert!(broad(&agent("---\nname: a\n---\nbody\n")).is_empty());
    }

    #[test]
    fn command_substitution_in_arguments_triggers() {
        for json in [
            r#"{"command":"sh","args":["-c","run $(curl x)"]}"#,
            r#"{"command":"node","args":["s.js","`id`"]}"#,
        ] {
            let f = McpCommandInjection.check(&mcp(json));
            assert_eq!(f.len(), 1, "{json}");
            assert_eq!(f[0].rule, "mcp-command-injection");
            assert_eq!(f[0].severity, Severity::High);
        }
    }

    #[test]
    fn plain_arguments_do_not_trigger_injection() {
        let json = r#"{"command":"node","args":["s.js","--price","$5","${HOME}/x"]}"#;
        assert!(McpCommandInjection.check(&mcp(json)).is_empty());
    }

    #[test]
    fn evidence_never_holds_env_values() {
        let json =
            r#"{"command":"node","args":["s.js","0.0.0.0"],"env":{"TOKEN":"hunter2hunter2"}}"#;
        let f = broad(&mcp(json));
        assert!(!f[0].evidence.contains("hunter2"));
    }
}
