//! Settings: `.claude/settings.json` (project) or `settings.json` at the root (pack or
//! `~/.claude`), never `settings.local.json`. One artifact named `settings` per file, with every
//! key kept whole in `metadata`. The scripts its hooks and status line point to travel with it
//! (`files`). Nothing in it is ever run.
//!
//! Scripts are listed in `files` only: there is no artifact type for a loose script, so they are
//! not `references`. `outputStyle` and `enabledPlugins` keys are references (unresolved here).
//! Literal values in `env` that look like secrets are replaced by `<redacted>` in `metadata`.

use std::path::Path;

use serde_json::{Map, Value};

use crate::artifact::literal_secret::{is_literal_secret, REDACTED};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, Reference, ScanReport};

/// Where a Settings file can sit, relative to the scan root.
const LOCATIONS: [&str; 2] = [".claude/settings.json", "settings.json"];
/// Commands that run the script given as their first argument.
const INTERPRETERS: [&str; 9] = [
    "bash", "sh", "zsh", "node", "python", "python3", "deno", "bun", "ruby",
];

/// Pure: reads one Settings file. `path` is relative to the scan root; `script_exists` says
/// whether a root-relative script path is a regular file.
pub(crate) fn parse(path: &Path, contents: &str, script_exists: &dyn Fn(&str) -> bool) -> Artifact {
    let mut artifact = Artifact {
        kind: ArtifactKind::Settings,
        name: "settings".into(),
        description: None,
        path: rel_string(path),
        files: vec![rel_string(path)],
        metadata: Map::new(),
        references: Vec::new(),
        diagnostics: Vec::new(),
        digest: None,
        origin: crate::artifact::Origin::default(),
    };
    let mut metadata = match serde_json::from_str::<Value>(contents) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            artifact.diagnostics.push(Diagnostic::error(
                "settings.invalid-json",
                path,
                "settings.json must hold a JSON object",
            ));
            return artifact;
        }
        Err(e) => {
            artifact.diagnostics.push(Diagnostic::error(
                "settings.invalid-json",
                path,
                &format!("not valid JSON: {e}"),
            ));
            return artifact;
        }
    };

    redact_env(&mut metadata, path, &mut artifact.diagnostics);
    references(&metadata, &mut artifact.references);

    for command in commands(&metadata) {
        for script in script_paths(&command) {
            let Ok(resolved) = resolve_reference(Path::new(""), &script) else {
                artifact.diagnostics.push(Diagnostic::warning(
                    "settings.unsafe-script-path",
                    path,
                    &format!("`{script}` is absolute or leaves the folder, so it was not followed"),
                ));
                continue;
            };
            let script = rel_string(&resolved);
            if script_exists(&script) {
                if !artifact.files.contains(&script) {
                    artifact.files.push(script);
                }
            } else if !artifact
                .diagnostics
                .iter()
                .any(|d| d.code == "settings.missing-script" && d.message.contains(&script))
            {
                artifact.diagnostics.push(Diagnostic::error(
                    "settings.missing-script",
                    path,
                    &format!("the script `{script}` is missing (or is a symlink, which is never followed)"),
                ));
            }
        }
    }
    artifact.files.sort();
    artifact.metadata = metadata;
    artifact
}

/// Replaces literal secrets in `env` with a marker and warns by key name.
fn redact_env(metadata: &mut Map<String, Value>, path: &Path, out: &mut Vec<Diagnostic>) {
    let Some(Value::Object(env)) = metadata.get_mut("env") else {
        return;
    };
    for (key, value) in env.iter_mut() {
        let literal = value.as_str().is_some_and(|v| is_literal_secret(key, v));
        if literal {
            *value = Value::String(REDACTED.into());
            out.push(Diagnostic::warning(
                "settings.literal-secret",
                path,
                &format!(
                    "`env.{key}` holds a literal secret; use a `${{VAR}}` placeholder instead"
                ),
            ));
        }
    }
}

fn references(metadata: &Map<String, Value>, out: &mut Vec<Reference>) {
    if let Some(Value::String(style)) = metadata.get("outputStyle") {
        out.push(Reference {
            kind: ArtifactKind::OutputStyle,
            target: style.clone(),
            resolved: false,
        });
    }
    if let Some(Value::Object(plugins)) = metadata.get("enabledPlugins") {
        out.extend(plugins.keys().map(|k| Reference {
            kind: ArtifactKind::Plugin,
            target: k.clone(),
            resolved: false,
        }));
    }
}

/// Every command string in `hooks` (event, matcher groups, `hooks[]`) and `statusLine`.
fn commands(metadata: &Map<String, Value>) -> Vec<String> {
    let mut out = Vec::new();
    let mut take = |v: &Value| {
        let is_command = v.get("type").is_none_or(|t| t == "command");
        if let (true, Some(Value::String(c))) = (is_command, v.get("command")) {
            // Exec form: `command` is the executable and `args` its argument vector.
            let args = v
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten();
            let words: Vec<&str> = std::iter::once(c.as_str())
                .chain(args.filter_map(Value::as_str))
                .collect();
            out.push(words.join(" "));
        }
    };
    if let Some(Value::Object(events)) = metadata.get("hooks") {
        for groups in events.values().filter_map(Value::as_array) {
            for hook in groups
                .iter()
                .filter_map(|g| g.get("hooks")?.as_array())
                .flatten()
            {
                take(hook);
            }
        }
    }
    if let Some(status) = metadata.get("statusLine") {
        take(status);
    }
    out
}

/// The script paths a shell command points to, relative to the project root. Plain binaries
/// on PATH (`npx prettier`) and absolute paths are not scripts.
fn script_paths(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    for segment in command.split(['&', '|', ';', '\n']) {
        let tokens: Vec<String> = segment
            .split_whitespace()
            .map(|t| t.replace(['"', '\''], ""))
            .collect();
        let Some(first) = tokens.first() else {
            continue;
        };
        let candidate = if INTERPRETERS.contains(&first.as_str()) {
            tokens.iter().skip(1).find(|t| !t.starts_with('-'))
        } else {
            Some(first)
        };
        if let Some(path) = candidate.and_then(|t| script_candidate(t)) {
            out.push(path);
        }
    }
    out
}

/// `Some(root-relative path)` when the token looks like a path inside the project.
fn script_candidate(token: &str) -> Option<String> {
    let mut t = token;
    let mut explicit = token.starts_with("./");
    for prefix in ["${CLAUDE_PROJECT_DIR}/", "$CLAUDE_PROJECT_DIR/"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest;
            explicit = true;
        }
    }
    let t = t.strip_prefix("./").unwrap_or(t);
    if t.is_empty()
        || t.starts_with(['$', '~', '-'])
        || t.contains("://")
        || t.contains(['=', '$', '`', '(', ')', '<', '>'])
        || !(explicit || t.contains('/'))
    {
        return None;
    }
    // An absolute path outside `$CLAUDE_PROJECT_DIR` is not a project script.
    if t.starts_with('/') && t.len() == token.len() {
        return None;
    }
    Some(t.to_owned())
}

/// Finds `.claude/settings.json` and `settings.json` under the scan root, through the walk.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    for location in LOCATIONS {
        let rel = Path::new(location);
        if !walker.exists(rel) {
            continue;
        }
        let mut walk_diagnostics = Vec::new();
        let Some(contents) = walker.read_text(rel, &mut walk_diagnostics) else {
            // Not read (a symlink, too large…): the walk's diagnostic says why.
            report.diagnostics.extend(walk_diagnostics);
            continue;
        };
        let exists = |script: &str| walker.is_file(Path::new(script));
        let mut artifact = parse(rel, &contents, &exists);
        artifact.diagnostics.extend(walk_diagnostics);
        report.artifacts.push(artifact);
    }
}

/// `CONTEXT.md` §2: Settings install to `.claude/settings.json`, wherever they were found.
pub(crate) fn install_path(_artifact: &crate::artifact::Artifact) -> std::path::PathBuf {
    use std::path::PathBuf;
    PathBuf::from(".claude/settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ScanOptions;
    use std::fs;

    fn scan_dir(root: &Path, options: &ScanOptions, report: &mut ScanReport) {
        let mut walker = Walker::new(root, options);
        scan(&mut walker, report);
        report.diagnostics.extend(walker.finish());
    }

    const SECRET: &str = "harneo-test-literal-value-123";

    fn read(json: &str, existing: &[&str]) -> Artifact {
        let exists = |p: &str| existing.contains(&p);
        parse(Path::new(".claude/settings.json"), json, &exists)
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    const HOOKS: &str = r#"{
      "hooks": {
        "PreToolUse": [
          {"matcher": "Bash", "hooks": [
            {"type": "command", "command": "python3 \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/guard.py --x"}
          ]}
        ],
        "PostToolUse": [
          {"hooks": [{"type": "command", "command": "npx prettier --write src/a.ts"}]}
        ]
      },
      "statusLine": {"type": "command", "command": "./scripts/status.sh"}
    }"#;

    #[test]
    fn keeps_every_key_whole_and_names_the_artifact_settings() {
        let a = read(
            r#"{"model":"opus","permissions":{"allow":["Bash(ls)"]}}"#,
            &[],
        );
        assert_eq!(a.kind, ArtifactKind::Settings);
        assert_eq!(a.name, "settings");
        assert_eq!(a.path, ".claude/settings.json");
        assert_eq!(a.metadata["model"], "opus");
        assert_eq!(a.metadata["permissions"]["allow"][0], "Bash(ls)");
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn hook_and_status_line_scripts_join_files() {
        let a = read(HOOKS, &[".claude/hooks/guard.py", "scripts/status.sh"]);
        assert_eq!(
            a.files,
            [
                ".claude/hooks/guard.py",
                ".claude/settings.json",
                "scripts/status.sh"
            ]
        );
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
    }

    #[test]
    fn exec_form_args_point_to_the_script() {
        let json = r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"node","args":["${CLAUDE_PROJECT_DIR}/scripts/format.js","--fix"]}]}]}}"#;
        let a = read(json, &["scripts/format.js"]);
        assert!(a.files.contains(&"scripts/format.js".to_owned()));
        assert!(a.diagnostics.is_empty());
        let plain = read(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"npx","args":["prettier","--check"]}]}]}}"#,
            &[],
        );
        assert_eq!(plain.files, [".claude/settings.json"]);
    }

    #[test]
    fn braced_project_dir_form_is_recognised() {
        let json = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"\"${CLAUDE_PROJECT_DIR}\"/.claude/hooks/x.sh"}]}]}}"#;
        let a = read(json, &[".claude/hooks/x.sh"]);
        assert!(a.files.contains(&".claude/hooks/x.sh".to_owned()));
    }

    #[test]
    fn missing_script_is_an_error_and_present_one_is_not() {
        let a = read(HOOKS, &["scripts/status.sh"]);
        assert_eq!(codes(&a), ["settings.missing-script"]);
        assert!(a.diagnostics[0].message.contains(".claude/hooks/guard.py"));
        assert!(codes(&read(
            HOOKS,
            &[".claude/hooks/guard.py", "scripts/status.sh"]
        ))
        .is_empty());
    }

    #[test]
    fn plain_binaries_and_absolute_paths_are_not_scripts() {
        let json = r#"{"hooks":{"Stop":[{"hooks":[
          {"type":"command","command":"npx prettier --write src/a.ts"},
          {"type":"command","command":"/usr/bin/true"},
          {"type":"command","command":"echo done && git status"}]}]}}"#;
        let a = read(json, &[]);
        assert_eq!(a.files, [".claude/settings.json"]);
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn dotdot_script_is_flagged_not_followed() {
        let json = r#"{"statusLine":{"type":"command","command":"../evil/x.sh"}}"#;
        // Even if the checker says it exists, a `..` path is never followed.
        let a = parse(Path::new("settings.json"), json, &|_| true);
        assert_eq!(codes(&a), ["settings.unsafe-script-path"]);
        assert_eq!(a.files, ["settings.json"]);
        assert!(codes(&read(
            r#"{"statusLine":{"type":"command","command":"./a/b.sh"}}"#,
            &["a/b.sh"]
        ))
        .is_empty());
    }

    #[test]
    fn output_style_and_plugins_become_references() {
        let a = read(
            r#"{"outputStyle":"concise","enabledPlugins":{"fmt@acme":true,"old@acme":false}}"#,
            &[],
        );
        let got: Vec<_> = a
            .references
            .iter()
            .map(|r| (r.kind, r.target.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (ArtifactKind::OutputStyle, "concise"),
                (ArtifactKind::Plugin, "fmt@acme"),
                (ArtifactKind::Plugin, "old@acme"),
            ]
        );
        assert!(a.references.iter().all(|r| !r.resolved));
        assert!(read("{}", &[]).references.is_empty());
    }

    #[test]
    fn invalid_json_triggers_and_valid_does_not() {
        assert_eq!(codes(&read("{ nope", &[])), ["settings.invalid-json"]);
        assert_eq!(codes(&read("[1]", &[])), ["settings.invalid-json"]);
        assert!(codes(&read("{}", &[])).is_empty());
    }

    #[test]
    fn literal_secret_warns_by_key_and_never_leaks_the_value() -> Result<(), serde_json::Error> {
        let json = format!(r#"{{"env":{{"API_TOKEN":"{SECRET}","LOG_LEVEL":"debug"}}}}"#);
        let a = read(&json, &[]);
        assert_eq!(codes(&a), ["settings.literal-secret"]);
        assert!(a.diagnostics[0].message.contains("API_TOKEN"));
        assert_eq!(a.metadata["env"]["API_TOKEN"], REDACTED);
        assert_eq!(a.metadata["env"]["LOG_LEVEL"], "debug");
        let report = ScanReport {
            artifacts: vec![a.clone()],
            diagnostics: Vec::new(),
        };
        for text in [
            format!("{a:?}"),
            format!("{report:?}"),
            serde_json::to_string(&a)?,
            serde_json::to_string(&report)?,
        ] {
            assert!(!text.contains(SECRET));
        }
        Ok(())
    }

    #[test]
    fn placeholder_values_give_no_warning() {
        let a = read(
            r#"{"env":{"API_TOKEN":"${API_TOKEN}","DB_PASSWORD":"$DB_PASSWORD"}}"#,
            &[],
        );
        assert!(a.diagnostics.is_empty());
        assert_eq!(a.metadata["env"]["API_TOKEN"], "${API_TOKEN}");
    }

    #[test]
    fn scan_finds_both_locations_and_skips_local_and_symlinks() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        fs::create_dir_all(root.join(".claude/hooks"))?;
        fs::write(root.join(".claude/hooks/h.sh"), "#!/bin/sh\n")?;
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":".claude/hooks/h.sh"},{"type":"command","command":".claude/hooks/gone.sh"}]}]}}"#,
        )?;
        fs::write(root.join("settings.json"), "{}")?;
        fs::write(root.join(".claude/settings.local.json"), "{}")?;
        let mut report = ScanReport::default();
        scan_dir(root, &ScanOptions::default(), &mut report);
        let paths: Vec<_> = report.artifacts.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths, [".claude/settings.json", "settings.json"]);
        let first = &report.artifacts[0];
        assert!(first.files.contains(&".claude/hooks/h.sh".to_owned()));
        assert_eq!(codes(first), ["settings.missing-script"]);
        Ok(())
    }

    #[test]
    fn oversized_file_warns_and_normal_size_does_not() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("settings.json"), r#"{"model":"opus"}"#)?;
        let tiny = ScanOptions {
            max_file_bytes: 4,
            ..ScanOptions::default()
        };
        let mut report = ScanReport::default();
        scan_dir(dir.path(), &tiny, &mut report);
        assert!(report.artifacts.is_empty());
        assert_eq!(report.diagnostics[0].code, "walk.file-too-large");
        let mut ok = ScanReport::default();
        scan_dir(dir.path(), &ScanOptions::default(), &mut ok);
        assert_eq!(ok.artifacts.len(), 1);
        assert!(ok.diagnostics.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_settings_and_scripts_are_never_followed() -> std::io::Result<()> {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        fs::write(root.join("real.json"), "{}")?;
        symlink(root.join("real.json"), root.join("settings.json"))?;
        let mut report = ScanReport::default();
        scan_dir(root, &ScanOptions::default(), &mut report);
        assert!(report.artifacts.is_empty());
        assert_eq!(report.diagnostics[0].code, "walk.symlink");

        fs::create_dir_all(root.join(".claude"))?;
        fs::write(root.join("target.sh"), "")?;
        symlink(root.join("target.sh"), root.join("link.sh"))?;
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"statusLine":{"type":"command","command":"./link.sh"}}"#,
        )?;
        let mut r2 = ScanReport::default();
        scan_dir(root, &ScanOptions::default(), &mut r2);
        assert!(r2.artifacts[0].files.iter().all(|f| f != "link.sh"));
        assert_eq!(codes(&r2.artifacts[0]), ["settings.missing-script"]);
        Ok(())
    }
}

#[cfg(test)]
mod install_tests {
    use crate::artifact::{Artifact, ArtifactKind};

    fn install(kind: ArtifactKind, path: &str) -> std::path::PathBuf {
        let artifact = Artifact {
            kind,
            name: "x".into(),
            description: None,
            path: path.into(),
            files: Vec::new(),
            metadata: Default::default(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        };
        super::install_path(&artifact)
    }

    #[test]
    fn install_path_matches_the_context_table() {
        assert_eq!(
            install(ArtifactKind::Settings, "settings.json"),
            std::path::Path::new(".claude/settings.json")
        );
        assert_eq!(
            install(ArtifactKind::Settings, ".claude/settings.json"),
            std::path::Path::new(".claude/settings.json")
        );
    }
}
