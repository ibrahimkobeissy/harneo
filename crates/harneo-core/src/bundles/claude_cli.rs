//! Runs the user's `claude` CLI for bundle and tag detection. After `git`, this is the second
//! program core starts.
//!
//! Claude is started with an argument list (never a shell), the prompt on stdin, no tools (it can
//! only read what it is given), no saved session, no MCP servers, an empty temporary folder as
//! its working directory (so no project `CLAUDE.md` or settings load), a timeout and an output
//! cap. Nothing here ever puts artifact text on a command line.

use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::{Error, Result};

/// The longest one claude call may run.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(180);
/// The longest `claude --version` may run.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a running claude is checked on.
const POLL: Duration = Duration::from_millis(20);
/// How much of claude's answer is accepted.
pub(crate) const OUTPUT_LIMIT: u64 = 1024 * 1024;
/// How much of claude's error text is read.
const STDERR_LIMIT: u64 = 16 * 1024;
/// How much of an error is shown.
const MESSAGE_CHARS: usize = 500;
/// The model used when Settings holds none.
pub const DEFAULT_MODEL: &str = "haiku";

/// How to run claude. The default is `claude` from the `PATH`, Haiku and a 180 s limit; tests
/// point `program` at a fixture script.
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// The program to start.
    pub program: OsString,
    /// Replaces `PATH` for the lookup (and for the child) when set.
    pub path: Option<OsString>,
    /// The model alias or name passed to `--model`.
    pub model: String,
    /// The limit for each call.
    pub timeout: Duration,
    /// Where the empty working folder is made; the system temp folder when `None`.
    pub scratch: Option<PathBuf>,
    /// When set to `true`, a running call is killed and fails with [`Error::Cancelled`].
    pub cancel: Option<Arc<AtomicBool>>,
    /// Where each call's log line goes; the log folder of [`crate::paths::log_dir`] when `None`.
    /// Tests point it at a temporary folder.
    pub log_dir: Option<PathBuf>,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            program: OsString::from("claude"),
            path: None,
            model: DEFAULT_MODEL.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            scratch: None,
            cancel: None,
            log_dir: None,
        }
    }
}

/// Whether the `claude` CLI can be used, for Settings and the Review step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct CliStatus {
    pub found: bool,
    /// Where it was found.
    pub path: Option<String>,
    /// What `claude --version` printed (first line).
    pub version: Option<String>,
}

fn io_error(source: std::io::Error) -> Error {
    Error::Io {
        path: "claude".to_owned(),
        source,
    }
}

fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// The program's file, searched on `PATH` unless it already names a path.
fn locate(config: &ClaudeConfig) -> Option<PathBuf> {
    let program = Path::new(&config.program);
    if program.components().count() > 1 {
        return program.is_file().then(|| program.to_path_buf());
    }
    let path = config
        .path
        .clone()
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Finds `claude` and asks it for its version.
pub fn cli_status(config: &ClaudeConfig) -> CliStatus {
    let Some(found) = locate(config) else {
        return CliStatus {
            found: false,
            path: None,
            version: None,
        };
    };
    let version = run_with(config, &["--version"], None, VERSION_TIMEOUT)
        .ok()
        .and_then(|out| out.stdout.lines().next().map(|l| cut(l.trim(), 100)))
        .filter(|v| !v.is_empty());
    CliStatus {
        found: true,
        path: Some(found.display().to_string()),
        version,
    }
}

/// Who is calling Claude and what the call is about, for the log: the feature's label and the
/// paths (or artifact names) that go into it. Never the prompt, never file contents.
pub(crate) struct Call<'a> {
    pub(crate) feature: &'a str,
    pub(crate) paths: &'a [String],
}

pub(crate) struct Run {
    pub(crate) success: bool,
    /// The exit code, when the process ended with one.
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Runs a `claude` management subcommand (`plugin …`) in `cwd`, or in an empty scratch folder
/// when `None`. No model is called and nothing is sent to Claude.
pub(crate) fn run_subcommand(
    config: &ClaudeConfig,
    call: &Call,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<Run> {
    let started = Instant::now();
    let result = run_in(config, args, None, timeout, cwd);
    // Only the subcommand's own words (`plugin install`) are logged: what follows may be a
    // plugin, a marketplace or a path.
    let words = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
    let (exit, outcome) = match &result {
        Ok(run) => (
            run.code,
            if run.success {
                "ok".to_owned()
            } else {
                format!(
                    "error={}",
                    crate::log::quoted(&failure(&run.stderr, &run.stdout).to_string())
                )
            },
        ),
        Err(error) => (
            None,
            format!("error={}", crate::log::quoted(&error.to_string())),
        ),
    };
    log_call(
        config,
        &format!(
            "claude-subcommand feature={} command={}",
            call.feature,
            crate::log::quoted(&words)
        ),
        call,
        started,
        exit,
        &outcome,
    );
    result
}

/// One log line for a finished call.
fn log_call(
    config: &ClaudeConfig,
    head: &str,
    call: &Call,
    started: Instant,
    exit: Option<i32>,
    outcome: &str,
) {
    let exit = exit.map_or_else(|| "none".to_owned(), |c| c.to_string());
    crate::log::record(
        config.log_dir.as_deref(),
        &format!(
            "{head} {} duration_ms={} exit={exit} {outcome}",
            crate::log::paths_field(call.paths),
            started.elapsed().as_millis(),
        ),
    );
}

fn is_cancelled(config: &ClaudeConfig) -> bool {
    config
        .cancel
        .as_ref()
        .is_some_and(|c| c.load(Ordering::SeqCst))
}

fn read_all(file: &mut File, limit: u64) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Starts claude with `args`, feeds `stdin`, and waits at most `timeout`. Output beyond
/// [`OUTPUT_LIMIT`] stops the process and is reported as a bad answer.
fn run_with(
    config: &ClaudeConfig,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Duration,
) -> Result<Run> {
    run_in(config, args, stdin, timeout, None)
}

/// [`run_with`] with `cwd` as the working directory instead of an empty scratch folder: only
/// for `claude plugin …` management commands, which never call a model.
fn run_in(
    config: &ClaudeConfig,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Duration,
    cwd: Option<&Path>,
) -> Result<Run> {
    let mut stdout = tempfile::tempfile().map_err(io_error)?;
    let mut stderr = tempfile::tempfile().map_err(io_error)?;
    let scratch = match &config.scratch {
        Some(parent) => tempfile::Builder::new()
            .prefix("harneo-claude-")
            .tempdir_in(parent),
        None => tempfile::Builder::new().prefix("harneo-claude-").tempdir(),
    }
    .map_err(io_error)?;
    let mut command = Command::new(&config.program);
    crate::host_env::for_host(&mut command);
    command
        .args(args)
        .current_dir(cwd.unwrap_or(scratch.path()))
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(stdout.try_clone().map_err(io_error)?))
        .stderr(Stdio::from(stderr.try_clone().map_err(io_error)?));
    if let Some(path) = &config.path {
        command.env("PATH", path);
    }
    if is_cancelled(config) {
        return Err(Error::Cancelled);
    }
    let mut attempts = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::ClaudeMissing),
            // The program file was being written a moment ago (an update replacing it).
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 5 => {
                attempts += 1;
                thread::sleep(POLL);
            }
            Err(e) => {
                return Err(Error::ClaudeFailed {
                    message: cut(&e.to_string(), MESSAGE_CHARS),
                    kind: FailureKind::Other,
                })
            }
        }
    };
    // The prompt is written from its own thread so a full pipe can never block the timeout.
    let writer = stdin.and_then(|text| {
        let mut pipe = child.stdin.take()?;
        let text = text.to_owned();
        Some(thread::spawn(move || {
            // A claude that exits early closes the pipe; its exit status tells the story.
            let _ = pipe.write_all(text.as_bytes());
        }))
    });
    let started = Instant::now();
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(e) => break Err(io_error(e)),
        }
        let written = stdout.metadata().map(|m| m.len()).unwrap_or(0);
        if written > OUTPUT_LIMIT {
            // Best effort: the oversized answer is what gets reported.
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::ClaudeBadOutput {
                message: "the answer is larger than 1 MiB".to_owned(),
            });
        }
        if is_cancelled(config) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::ClaudeTimeout {
                seconds: timeout.as_secs(),
            });
        }
        thread::sleep(POLL);
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let status = outcome?;
    if stdout.metadata().map(|m| m.len()).unwrap_or(0) > OUTPUT_LIMIT {
        return Err(Error::ClaudeBadOutput {
            message: "the answer is larger than 1 MiB".to_owned(),
        });
    }
    Ok(Run {
        success: status.success(),
        code: status.code(),
        stdout: read_all(&mut stdout, OUTPUT_LIMIT).map_err(io_error)?,
        stderr: read_all(&mut stderr, STDERR_LIMIT).map_err(io_error)?,
    })
}

/// Why a claude call failed, stored so screens need not read text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename_all = "snake_case")]
pub enum FailureKind {
    /// Claude is not signed in.
    NotLoggedIn,
    /// A usage or rate limit was reached.
    Limit,
    /// The chosen model is not available to this account.
    ModelUnavailable,
    /// The call ran past its time limit.
    Timeout,
    /// Anything else.
    Other,
}

impl FailureKind {
    /// Whether the account itself is the problem, so another call would fail the same way.
    pub fn is_account_level(self) -> bool {
        matches!(self, Self::NotLoggedIn | Self::Limit)
    }
}

/// Sorts a failure into a kind from claude's own words and the API status when it gave one.
fn classify(text: &str, status: Option<&str>) -> FailureKind {
    let t = text.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    if status == Some("401")
        || has(&[
            "not logged in",
            "/login",
            "invalid api key",
            "authentication",
            "unauthorized",
        ])
    {
        FailureKind::NotLoggedIn
    } else if status == Some("429")
        || has(&[
            "usage limit",
            "hit your limit",
            "rate limit",
            "limit reached",
            "quota",
            "credit balance",
            "too many requests",
        ])
    {
        FailureKind::Limit
    } else if status == Some("404")
        || (t.contains("model")
            && has(&[
                "not found",
                "does not exist",
                "not available",
                "unavailable",
                "issue with",
            ]))
    {
        FailureKind::ModelUnavailable
    } else {
        FailureKind::Other
    }
}

/// The short sentence for a recognised kind.
fn sentence(kind: FailureKind) -> Option<&'static str> {
    match kind {
        FailureKind::NotLoggedIn => {
            Some("Claude is not logged in. Run `claude` in a terminal to sign in.")
        }
        FailureKind::Limit => Some("Claude's usage limit is reached. Try again later."),
        FailureKind::ModelUnavailable => {
            Some("The chosen model is not available. Pick another in Settings.")
        }
        FailureKind::Timeout | FailureKind::Other => None,
    }
}

/// Claude's `--output-format json` envelope when it is one: its `result` text and API status.
fn envelope(stdout: &str) -> Option<(String, Option<String>)> {
    let Value::Object(map) = serde_json::from_str::<Value>(stdout.trim()).ok()? else {
        return None;
    };
    let result = map
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let status = match map.get("api_error_status") {
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    (!result.is_empty() || status.is_some()).then_some((result, status))
}

/// Why claude failed. A JSON envelope on stdout (its `result`, and the API status) comes
/// first; otherwise the last lines of stderr, or of stdout when stderr is empty.
fn failure(stderr: &str, stdout: &str) -> Error {
    let (raw, status) = match envelope(stdout) {
        Some((result, status)) => {
            let mut raw = result;
            if let Some(code) = &status {
                if raw.is_empty() {
                    raw = format!("API error {code}");
                } else {
                    raw = format!("{raw} (API error {code})");
                }
            }
            (raw, status)
        }
        None => {
            let text = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            let lines: Vec<&str> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            (lines[lines.len().saturating_sub(2)..].join(" "), None)
        }
    };
    let kind = classify(&raw, status.as_deref());
    let message = match sentence(kind) {
        Some(known) if raw.is_empty() => known.to_owned(),
        Some(known) => cut(&format!("{known} ({raw})"), MESSAGE_CHARS),
        None => cut(&raw, MESSAGE_CHARS),
    };
    Error::ClaudeFailed { message, kind }
}

/// The JSON a `--output-format json` run answered with: the structured output when there is
/// one, otherwise the `result` text parsed as JSON (fences removed).
fn payload(stdout: &str) -> Result<Value> {
    let bad = |message: &str| Error::ClaudeBadOutput {
        message: message.to_owned(),
    };
    let envelope: Value =
        serde_json::from_str(stdout.trim()).map_err(|_| bad("the answer is not JSON"))?;
    let Value::Object(map) = &envelope else {
        return Err(bad("the answer is not a JSON object"));
    };
    if map.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(failure("", stdout));
    }
    if let Some(structured) = map.get("structured_output").filter(|v| v.is_object()) {
        return Ok(structured.clone());
    }
    if let Some(text) = map.get("result").and_then(Value::as_str) {
        let text = text.trim();
        let text = text
            .strip_prefix("```json")
            .or_else(|| text.strip_prefix("```"))
            .map_or(text, |rest| rest.trim_end_matches("```"))
            .trim();
        return serde_json::from_str(text).map_err(|_| bad("the result is not JSON"));
    }
    Err(bad("the answer has no result"))
}

/// Runs one prompt and returns the JSON claude answered with. `schema` is the JSON Schema the
/// answer must follow. The call is logged: who asked, what was sent (paths only), how long it
/// took, the exit code, the error text and whether the answer parsed.
pub(crate) fn run(config: &ClaudeConfig, call: &Call, prompt: &str, schema: &str) -> Result<Value> {
    let args = [
        "-p",
        "--output-format",
        "json",
        "--json-schema",
        schema,
        "--model",
        &config.model,
        // No tools at all: the pass can only read what the prompt holds.
        "--tools",
        "",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--disable-slash-commands",
        // No settings files at all: the user's and the project's hooks must not act on the run.
        "--setting-sources",
        "",
    ];
    let started = Instant::now();
    let mut exit = None;
    let result = run_with(config, &args, Some(prompt), config.timeout).and_then(|out| {
        exit = out.code;
        if out.success {
            payload(&out.stdout)
        } else {
            Err(failure(&out.stderr, &out.stdout))
        }
    });
    let outcome = match &result {
        Ok(_) => "parsed=true".to_owned(),
        Err(error) => format!(
            "parsed=false error={}",
            crate::log::quoted(&error.to_string())
        ),
    };
    log_call(
        config,
        &format!(
            "claude feature={} model={}",
            call.feature,
            crate::log::quoted(&config.model)
        ),
        call,
        started,
        exit,
        &outcome,
    );
    result
}
