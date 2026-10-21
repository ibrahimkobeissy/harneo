//! MCP servers: the `mcpServers` map of the project's `.mcp.json`. One artifact per server,
//! named by its key, with path `.mcp.json#<name>` and every field kept in `metadata`
//! (transport `type`, `command`/`args`/`env`, or `url`/`headers`). Nothing is ever run:
//! `headersHelper` is a shell command, so only a project-relative script it points to joins
//! `files`. Literal secrets in `env` and `headers` are replaced by `<redacted>` in `metadata`
//! and reported by key name only.

use std::path::Path;

use serde_json::{Map, Value};

use crate::artifact::literal_secret::{is_literal_secret, REDACTED};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanReport};

const FILE: &str = ".mcp.json";
/// Commands that run the script given as their first argument.
const INTERPRETERS: [&str; 9] = [
    "bash", "sh", "zsh", "node", "python", "python3", "deno", "bun", "ruby",
];

/// Pure: reads one `.mcp.json`. `script_exists` says whether a root-relative path is a regular
/// file. Returns the servers and the findings that belong to the file as a whole.
pub(crate) fn parse(
    contents: &str,
    script_exists: &dyn Fn(&str) -> bool,
) -> (Vec<Artifact>, Vec<Diagnostic>) {
    let file = Path::new(FILE);
    let root = match serde_json::from_str::<Value>(contents) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            return (
                Vec::new(),
                vec![Diagnostic::error(
                    "mcp.invalid-json",
                    file,
                    ".mcp.json must hold a JSON object",
                )],
            )
        }
        Err(e) => {
            return (
                Vec::new(),
                vec![Diagnostic::error(
                    "mcp.invalid-json",
                    file,
                    &format!("not valid JSON: {e}"),
                )],
            )
        }
    };
    let artifacts = match root.get("mcpServers") {
        Some(Value::Object(servers)) => servers
            .iter()
            .map(|(name, def)| server(name, def, script_exists))
            .collect(),
        _ => Vec::new(),
    };
    (artifacts, Vec::new())
}

fn server(name: &str, def: &Value, script_exists: &dyn Fn(&str) -> bool) -> Artifact {
    let file = Path::new(FILE);
    let mut metadata = match def {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    let mut artifact = Artifact {
        kind: ArtifactKind::Mcp,
        name: name.to_owned(),
        description: None,
        path: format!("{FILE}#{name}"),
        files: vec![FILE.to_owned()],
        metadata: Map::new(),
        references: Vec::new(),
        diagnostics: Vec::new(),
        digest: None,
    };
    let has =
        |key: &str| matches!(metadata.get(key), Some(Value::String(s)) if !s.trim().is_empty());
    if !has("command") && !has("url") {
        artifact.diagnostics.push(Diagnostic::error(
            "mcp.missing-command-or-url",
            file,
            &format!("the server `{name}` needs a `command` (stdio) or a `url` (http, sse, ws)"),
        ));
    }
    redact(&mut metadata, "env", file, &mut artifact.diagnostics);
    redact(&mut metadata, "headers", file, &mut artifact.diagnostics);

    if let Some(Value::String(helper)) = metadata.get("headersHelper") {
        if let Some(script) = helper_script(helper) {
            if script_exists(&script) && !artifact.files.contains(&script) {
                artifact.files.push(script);
            }
        }
    }
    artifact.files.sort();
    artifact.metadata = metadata;
    artifact
}

/// Replaces literal secrets in the `field` map with a marker and warns by key name.
fn redact(metadata: &mut Map<String, Value>, field: &str, file: &Path, out: &mut Vec<Diagnostic>) {
    let Some(Value::Object(map)) = metadata.get_mut(field) else {
        return;
    };
    for (key, value) in map.iter_mut() {
        if value.as_str().is_some_and(|v| is_literal(key, v)) {
            *value = Value::String(REDACTED.into());
            out.push(Diagnostic::warning(
                "mcp.literal-secret",
                file,
                &format!(
                    "`{field}.{key}` holds a literal secret; use a `${{VAR}}` placeholder instead"
                ),
            ));
        }
    }
}

/// The shared check, after dropping an auth scheme (`Bearer x`) and treating any value with a
/// `${VAR}` expansion inside as a placeholder.
fn is_literal(key: &str, value: &str) -> bool {
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

/// The project-relative script a `headersHelper` command points to, if any.
fn helper_script(command: &str) -> Option<String> {
    let tokens: Vec<String> = command
        .split_whitespace()
        .map(|t| t.replace(['"', '\''], ""))
        .collect();
    let first = tokens.first()?;
    let candidate = if INTERPRETERS.contains(&first.as_str()) {
        tokens.iter().skip(1).find(|t| !t.starts_with('-'))?
    } else {
        first
    };
    let rel = candidate
        .strip_prefix("${CLAUDE_PROJECT_DIR}/")
        .or_else(|| candidate.strip_prefix("$CLAUDE_PROJECT_DIR/"))
        .unwrap_or(candidate);
    resolve_reference(Path::new(""), rel)
        .ok()
        .map(|p| rel_string(&p))
        .filter(|p| !p.is_empty())
}

/// Finds `.mcp.json` at the scan root, through the walk.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    let rel = Path::new(FILE);
    if !walker.exists(rel) {
        return;
    }
    let mut walk_diagnostics = Vec::new();
    let Some(contents) = walker.read_text(rel, &mut walk_diagnostics) else {
        report.diagnostics.extend(walk_diagnostics);
        return;
    };
    let exists = |script: &str| walker.is_file(Path::new(script));
    let (artifacts, diagnostics) = parse(&contents, &exists);
    report.artifacts.extend(artifacts);
    report.diagnostics.extend(diagnostics);
    report.diagnostics.extend(walk_diagnostics);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ScanOptions, Severity};
    use std::fs;

    const SECRET: &str = "harneo-test-literal-value-123";

    fn read(json: &str, existing: &[&str]) -> (Vec<Artifact>, Vec<Diagnostic>) {
        let exists = |p: &str| existing.contains(&p);
        parse(json, &exists)
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    const SERVERS: &str = r#"{"mcpServers":{
      "db":{"command":"npx","args":["-y","dbhub"],"env":{"LOG_LEVEL":"${LOG_LEVEL:-info}"}},
      "api":{"type":"http","url":"https://x.example/mcp","headers":{"Authorization":"Bearer ${API_KEY}"},"headersHelper":"bash ./scripts/auth.sh"}
    }}"#;

    #[test]
    fn each_server_is_its_own_artifact_with_its_fields() {
        let (a, d) = read(SERVERS, &["scripts/auth.sh"]);
        assert!(d.is_empty());
        assert_eq!(a.len(), 2);
        let api = &a[0];
        assert_eq!(
            (api.name.as_str(), api.path.as_str()),
            ("api", ".mcp.json#api")
        );
        assert_eq!(api.metadata["type"], "http");
        assert_eq!(api.files, [".mcp.json", "scripts/auth.sh"]);
        assert!(api.diagnostics.is_empty());
        let db = &a[1];
        assert_eq!(db.metadata["command"], "npx");
        assert_eq!(db.files, [".mcp.json"]);
        assert!(db.diagnostics.is_empty());
    }

    #[test]
    fn missing_helper_script_is_not_listed_and_unsafe_one_is_ignored() {
        let (a, _) = read(SERVERS, &[]);
        assert_eq!(a[0].files, [".mcp.json"]);
        let json = r#"{"mcpServers":{"a":{"url":"u","headersHelper":"../evil.sh"},"b":{"url":"u","headersHelper":"/opt/x.sh"}}}"#;
        let (a, _) = read(json, &["evil.sh"]);
        assert!(a.iter().all(|x| x.files == [".mcp.json"]));
    }

    #[test]
    fn invalid_json_is_a_report_level_error_and_valid_is_not() {
        for bad in ["{oops", "[]"] {
            let (a, d) = read(bad, &[]);
            assert!(a.is_empty());
            assert_eq!(d[0].code, "mcp.invalid-json");
            assert_eq!(d[0].severity, Severity::Error);
        }
        let (_, d) = read(r#"{"mcpServers":{}}"#, &[]);
        assert!(d.is_empty());
    }

    #[test]
    fn neither_command_nor_url_is_an_error_and_either_is_not() {
        let (a, _) = read(
            r#"{"mcpServers":{"x":{"type":"stdio"},"y":{"command":"c"},"z":{"url":"u"}}}"#,
            &[],
        );
        assert_eq!(codes(&a[0]), ["mcp.missing-command-or-url"]);
        assert!(a[1].diagnostics.is_empty() && a[2].diagnostics.is_empty());
    }

    #[test]
    fn literal_secrets_warn_by_key_and_never_leak() {
        let json = format!(
            r#"{{"mcpServers":{{"s":{{"command":"c","env":{{"API_TOKEN":"{SECRET}","LOG":"debug"}}}},
            "r":{{"url":"u","headers":{{"Authorization":"Bearer {SECRET}","X-Other":"fine"}}}}}}}}"#
        );
        let (a, d) = read(&json, &[]);
        let by = |n: &str| a.iter().find(|x| x.name == n).cloned();
        let (Some(s), Some(r)) = (by("s"), by("r")) else {
            panic!("both servers are expected");
        };
        assert_eq!(codes(&s), ["mcp.literal-secret"]);
        assert!(s.diagnostics[0].message.contains("env.API_TOKEN"));
        assert_eq!(s.metadata["env"]["API_TOKEN"], REDACTED);
        assert_eq!(s.metadata["env"]["LOG"], "debug");
        assert_eq!(codes(&r), ["mcp.literal-secret"]);
        assert!(r.diagnostics[0].message.contains("headers.Authorization"));
        assert_eq!(r.metadata["headers"]["Authorization"], REDACTED);
        assert_eq!(r.metadata["headers"]["X-Other"], "fine");
        let report = ScanReport {
            artifacts: a,
            diagnostics: d,
        };
        assert!(!format!("{report:?}").contains(SECRET));
        assert!(!serde_json::to_string(&report)
            .unwrap_or_default()
            .contains(SECRET));
    }

    #[test]
    fn placeholders_do_not_warn() {
        let json = r#"{"mcpServers":{"s":{"url":"u","headers":{"Authorization":"Bearer ${API_KEY}","X-Api-Key":"${K:-}"},"env":{"API_TOKEN":"${T}"}}}}"#;
        let (a, _) = read(json, &[]);
        assert!(a[0].diagnostics.is_empty());
    }

    #[test]
    fn scan_reads_the_file_through_the_walk() {
        let dir = std::env::temp_dir().join(format!("harneo-mcp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(fs::create_dir_all(&dir).is_ok());
        assert!(fs::write(dir.join(".mcp.json"), SERVERS).is_ok());
        let options = ScanOptions::default();
        let mut report = ScanReport::default();
        let mut walker = Walker::new(&dir, &options);
        scan(&mut walker, &mut report);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(report.artifacts.len(), 2);
    }
}
