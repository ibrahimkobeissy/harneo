//! The runner against fixture scripts standing in for `claude`. The real CLI is never started.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::claude_cli::{cli_status, run};
use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

/// Writes an executable shell script and returns its path.
pub(super) fn script(dir: &Path, name: &str, body: &str) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

pub(super) fn config(program: &Path) -> ClaudeConfig {
    ClaudeConfig {
        program: program.into(),
        timeout: Duration::from_secs(20),
        ..ClaudeConfig::default()
    }
}

const OK: &str = r#"{"is_error":false,"structured_output":{"links":[],"tags":[]}}"#;

#[test]
fn success_reads_the_structured_output_and_gives_claude_no_tools() -> TestResult {
    let dir = tempfile::tempdir()?;
    let d = dir.path().display();
    let program = script(
        dir.path(),
        "claude",
        &format!(
            "cat > '{d}/stdin.txt'\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\npwd > '{d}/cwd.txt'\nprintf '%s' '{OK}'"
        ),
    )?;
    let got = run(
        &config(&program),
        "the prompt text",
        "{\"type\":\"object\"}",
    )?;
    assert_eq!(got["links"], serde_json::json!([]));
    assert_eq!(
        fs::read_to_string(dir.path().join("stdin.txt"))?,
        "the prompt text"
    );
    let args: Vec<String> = fs::read_to_string(dir.path().join("args.txt"))?
        .lines()
        .map(str::to_owned)
        .collect();
    for flag in [
        "-p",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--json-schema",
    ] {
        assert!(args.iter().any(|a| a == flag), "{flag} in {args:?}");
    }
    let at = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].clone())
    };
    assert_eq!(at("--output-format").as_deref(), Some("json"));
    assert_eq!(at("--model").as_deref(), Some("haiku"));
    // `--tools ""` is the empty list: no tools.
    assert_eq!(at("--tools").as_deref(), Some(""));
    // The prompt never travels on the command line.
    assert!(!args.iter().any(|a| a.contains("the prompt text")));
    // Claude runs in an empty temporary folder, not where the app was started.
    let cwd = fs::read_to_string(dir.path().join("cwd.txt"))?;
    assert!(cwd.contains("harneo-claude-"), "{cwd}");
    assert!(!cwd.trim().is_empty() && Path::new(cwd.trim()) != std::env::current_dir()?);
    Ok(())
}

#[test]
fn a_result_text_with_a_json_fence_is_accepted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = script(
        dir.path(),
        "claude",
        r#"cat >/dev/null
printf '%s' '{"result":"```json\n{\"links\":[],\"tags\":[]}\n```"}'"#,
    )?;
    let got = run(&config(&program), "p", "{}")?;
    assert_eq!(got["tags"], serde_json::json!([]));
    Ok(())
}

#[test]
fn a_slow_claude_is_stopped_at_the_timeout() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = script(dir.path(), "claude", "sleep 30")?;
    let mut cfg = config(&program);
    cfg.timeout = Duration::from_millis(300);
    let err = run(&cfg, "p", "{}").err().map(|e| e.to_string());
    assert_eq!(
        err.as_deref(),
        Some("claude did not finish within 0 s and was stopped")
    );
    Ok(())
}

#[test]
fn a_failing_claude_reports_its_stderr_but_never_the_prompt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let long = "x".repeat(900);
    let program = script(
        dir.path(),
        "claude",
        &format!("cat >/dev/null\necho 'first' >&2\necho 'not logged in {long}' >&2\nexit 3"),
    )?;
    let Err(Error::ClaudeFailed { message }) = run(&config(&program), "SECRET-PROMPT", "{}") else {
        return Err("expected ClaudeFailed".into());
    };
    assert!(message.contains("not logged in"));
    assert_eq!(message.chars().count(), 500);
    assert!(!message.contains("SECRET-PROMPT"));
    Ok(())
}

#[test]
fn an_error_envelope_is_a_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = script(
        dir.path(),
        "claude",
        r#"cat >/dev/null
printf '%s' '{"is_error":true,"result":"Credit balance is too low"}'"#,
    )?;
    let Err(Error::ClaudeFailed { message }) = run(&config(&program), "p", "{}") else {
        return Err("expected ClaudeFailed".into());
    };
    assert_eq!(message, "Credit balance is too low");
    Ok(())
}

#[test]
fn answers_that_are_not_the_asked_for_json_are_bad_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    for (i, out) in [
        "garbage, not json",
        "[1,2]",
        r#"{"is_error":false}"#,
        r#"{"result":"just words"}"#,
    ]
    .iter()
    .enumerate()
    {
        let program = script(
            dir.path(),
            &format!("claude{i}"),
            &format!("cat >/dev/null\nprintf '%s' '{out}'"),
        )?;
        let err = run(&config(&program), "p", "{}");
        assert!(
            matches!(err, Err(Error::ClaudeBadOutput { .. })),
            "{out}: {err:?}"
        );
    }
    Ok(())
}

#[test]
fn an_oversized_answer_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let program = script(
        dir.path(),
        "claude",
        "cat >/dev/null\nhead -c 3000000 /dev/zero | tr '\\0' 'a'",
    )?;
    let err = run(&config(&program), "p", "{}");
    assert!(matches!(err, Err(Error::ClaudeBadOutput { .. })), "{err:?}");
    Ok(())
}

#[test]
fn a_missing_program_is_reported_as_missing() {
    let cfg = ClaudeConfig {
        program: "harneo-no-such-claude".into(),
        path: Some("/nonexistent-dir".into()),
        ..ClaudeConfig::default()
    };
    assert!(matches!(run(&cfg, "p", "{}"), Err(Error::ClaudeMissing)));
    assert_eq!(
        cli_status(&cfg),
        CliStatus {
            found: false,
            path: None,
            version: None
        }
    );
}

#[test]
fn status_reports_the_path_and_the_version() -> TestResult {
    let dir = tempfile::tempdir()?;
    script(
        dir.path(),
        "claude",
        "echo '9.9.9 (Claude Code)'\necho second line",
    )?;
    let by_path = ClaudeConfig {
        program: "claude".into(),
        path: Some(dir.path().into()),
        ..ClaudeConfig::default()
    };
    let status = cli_status(&by_path);
    assert!(status.found);
    assert_eq!(status.version.as_deref(), Some("9.9.9 (Claude Code)"));
    assert_eq!(
        status.path,
        Some(dir.path().join("claude").display().to_string())
    );
    // A program given as a path works too, and one that fails --version is still found.
    let broken = script(dir.path(), "broken", "exit 1")?;
    let status = cli_status(&config(&broken));
    assert!(status.found);
    assert_eq!(status.version, None);
    Ok(())
}
