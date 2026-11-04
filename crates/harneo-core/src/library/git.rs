//! Shallow git clones for git sources.
//!
//! Git is run with an argument list (never a shell), no terminal and no credential prompt, and
//! is stopped when it runs past the timeout. A clone lives in a temp folder under the library
//! folder (same filesystem as the store) and is deleted when its [`Checkout`] is dropped.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::source::redact;
use crate::error::{Error, Result};

/// The longest a single git command may run.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// How often a running git is checked on.
const POLL: Duration = Duration::from_millis(20);
/// The largest checkout a clone may produce (bytes), measured after the clone finishes.
const MAX_CHECKOUT_BYTES: u64 = 500 * 1024 * 1024;
/// How much of git's output is kept.
const OUTPUT_LIMIT: u64 = 64 * 1024;

/// How to run git. The default is `git` from the PATH with a 120 s limit; tests swap in a stub.
#[derive(Debug, Clone)]
pub(crate) struct GitConfig {
    /// The program to start.
    pub program: OsString,
    /// Replaces `PATH` for the child (and its lookup) when set.
    pub path: Option<OsString>,
    /// The limit for each git command.
    pub timeout: Duration,
    /// The largest checkout a clone may leave behind.
    pub max_checkout_bytes: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            program: OsString::from("git"),
            path: None,
            timeout: DEFAULT_TIMEOUT,
            max_checkout_bytes: MAX_CHECKOUT_BYTES,
        }
    }
}

/// A finished clone. Dropping it removes the folder.
#[derive(Debug)]
pub(crate) struct Checkout {
    dir: TempDir,
    /// The full SHA of the checked-out commit.
    pub commit: String,
    /// The branch asked for, or the default branch git picked; `None` on a detached HEAD.
    pub branch: Option<String>,
}

impl Checkout {
    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// Shallow-clones `url` (at `branch`, or its default branch) into a new folder under `parent`.
pub(crate) fn clone(
    config: &GitConfig,
    parent: &Path,
    url: &str,
    branch: Option<&str>,
) -> Result<Checkout> {
    let dir = tempfile::Builder::new()
        .prefix(".clone-")
        .tempdir_in(parent)
        .map_err(|source| Error::Io {
            path: parent.display().to_string(),
            source,
        })?;
    let flags = protocol_flags(url);
    let mut args: Vec<&OsStr> = flags.iter().map(OsStr::new).collect();
    args.extend(
        ["clone", "--depth", "1", "--single-branch"]
            .iter()
            .map(OsStr::new),
    );
    if let Some(branch) = branch {
        args.extend([OsStr::new("--branch"), OsStr::new(branch)]);
    }
    args.extend([OsStr::new("--"), OsStr::new(url), dir.path().as_os_str()]);
    let output = run(config, &args)?;
    if !output.success {
        return Err(clone_error(&output.stderr, url, branch));
    }
    if dir_size(dir.path(), config.max_checkout_bytes) > config.max_checkout_bytes {
        // Dropping `dir` deletes the oversized clone.
        return Err(failed(&format!(
            "the repository is larger than {} MiB",
            config.max_checkout_bytes / (1024 * 1024)
        )));
    }

    let at = |rest: &[&str]| -> Result<String> {
        let mut args: Vec<&OsStr> = vec![OsStr::new("-C"), dir.path().as_os_str()];
        args.extend(rest.iter().map(OsStr::new));
        let output = run(config, &args)?;
        if output.success {
            Ok(output.stdout.trim().to_owned())
        } else {
            Err(failed(&output.stderr))
        }
    };
    let commit = at(&["rev-parse", "HEAD"])?;
    let branch = match branch {
        Some(given) => Some(given.to_owned()),
        None => Some(at(&["rev-parse", "--abbrev-ref", "HEAD"])?).filter(|b| b != "HEAD"),
    };
    Ok(Checkout {
        dir,
        commit,
        branch,
    })
}

/// The commit `branch` (a branch or tag, or the remote's HEAD when `None`) points to on the
/// remote, asked with `git ls-remote` so nothing is downloaded. A tag resolves to the commit
/// it points at, the same one a clone would check out.
pub(crate) fn ls_remote(config: &GitConfig, url: &str, branch: Option<&str>) -> Result<String> {
    let patterns: Vec<String> = match branch {
        Some(b) => vec![
            format!("refs/heads/{b}"),
            format!("refs/tags/{b}"),
            format!("refs/tags/{b}^{{}}"),
        ],
        None => vec!["HEAD".to_owned()],
    };
    let flags = protocol_flags(url);
    let mut args: Vec<&OsStr> = flags.iter().map(OsStr::new).collect();
    args.extend(["ls-remote", "--", url].into_iter().map(OsStr::new));
    args.extend(patterns.iter().map(OsStr::new));
    let output = run(config, &args)?;
    if !output.success {
        return Err(clone_error(&output.stderr, url, branch));
    }
    let listed: Vec<(&str, &str)> = output
        .stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect();
    let find = |name: &str| listed.iter().find(|(_, r)| *r == name).map(|(sha, _)| *sha);
    let sha = match branch {
        None => find("HEAD"),
        Some(b) => find(&format!("refs/heads/{b}"))
            .or_else(|| find(&format!("refs/tags/{b}^{{}}")))
            .or_else(|| find(&format!("refs/tags/{b}"))),
    };
    match (sha, branch) {
        (Some(sha), _) => Ok(sha.to_owned()),
        (None, Some(branch)) => Err(Error::GitBranchNotFound {
            branch: branch.to_owned(),
        }),
        (None, None) => Err(failed("the remote has no HEAD")),
    }
}

/// True for a local path or `file://` URL, the only input allowed to use the file protocol.
fn is_local(url: &str) -> bool {
    if url.starts_with("file://") {
        return true;
    }
    if url.contains("://") {
        return false;
    }
    // `host:path` (scp-like) is remote; a path has no colon before its first slash.
    match (url.find(':'), url.find('/')) {
        (Some(colon), Some(slash)) => slash < colon,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// `-c` flags for every git command: only https, ssh and git (and file for a local source),
/// no LFS filters, no hooks.
fn protocol_flags(url: &str) -> Vec<String> {
    let mut settings: Vec<&str> = vec![
        "protocol.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=always",
        "protocol.git.allow=always",
        "protocol.ext.allow=never",
        "filter.lfs.smudge=",
        "filter.lfs.process=",
        "filter.lfs.required=false",
        "core.hooksPath=/dev/null",
    ];
    if is_local(url) {
        settings.push("protocol.file.allow=always");
    }
    settings
        .iter()
        .flat_map(|s| ["-c".to_owned(), (*s).to_owned()])
        .collect()
}

/// Removes the variables that redirect git elsewhere and pins the safe ones.
fn harden(command: &mut Command) {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_EXEC_PATH",
        "GIT_TEMPLATE_DIR",
        "GIT_PROXY_COMMAND",
        "GIT_ALLOW_PROTOCOL",
    ] {
        command.env_remove(name);
    }
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_CONFIG") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1");
}

/// Total size of the files under `dir`; counting stops once it passes `limit`.
fn dir_size(dir: &Path, limit: u64) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
            if total > limit {
                return total;
            }
        }
    }
    total
}

struct Output {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run(config: &GitConfig, args: &[&OsStr]) -> Result<Output> {
    let io_error = |source| Error::Io {
        path: "git".to_owned(),
        source,
    };
    let mut stdout = tempfile::tempfile().map_err(io_error)?;
    let mut stderr = tempfile::tempfile().map_err(io_error)?;
    let mut command = Command::new(&config.program);
    command.args(args);
    harden(&mut command);
    crate::host_env::for_host(&mut command);
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().map_err(io_error)?))
        .stderr(Stdio::from(stderr.try_clone().map_err(io_error)?));
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        // ssh would otherwise ask for a password or host-key confirmation on /dev/tty.
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    if let Some(path) = &config.path {
        command.env("PATH", path);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::GitNotInstalled),
        Err(e) => return Err(io_error(e)),
    };
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(io_error)? {
            break status;
        }
        if started.elapsed() >= config.timeout {
            // Best effort: the timeout is what gets reported.
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::GitTimeout {
                seconds: config.timeout.as_secs(),
            });
        }
        thread::sleep(POLL);
    };
    Ok(Output {
        success: status.success(),
        stdout: read_all(&mut stdout).map_err(io_error)?,
        stderr: read_all(&mut stderr).map_err(io_error)?,
    })
}

fn read_all(file: &mut File) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(OUTPUT_LIMIT).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Turns a failed clone's stderr into the error the user should see.
fn clone_error(stderr: &str, url: &str, branch: Option<&str>) -> Error {
    let has = |needle: &str| stderr.contains(needle);
    if let Some(branch) = branch {
        if (has("Remote branch") && has("not found")) || has("Could not find remote branch") {
            return Error::GitBranchNotFound {
                branch: branch.to_owned(),
            };
        }
    }
    let url = redact(url);
    if has("Authentication failed")
        || has("terminal prompts disabled")
        || has("could not read Username")
        || has("could not read Password")
        || has("Permission denied")
    {
        return Error::GitAuthRequired { url };
    }
    if has("not found")
        || has("does not appear to be a git repository")
        || has("does not exist")
        || has("Could not resolve host")
        || has("unable to access")
        || has("Connection")
        || has("timed out")
        || has("Host key verification failed")
    {
        return Error::GitRepoUnreachable { url };
    }
    failed(stderr)
}

fn failed(stderr: &str) -> Error {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let tail = lines[lines.len().saturating_sub(2)..].join(" ");
    Error::GitFailed {
        message: redact(&tail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_maps_to_the_right_error() {
        let url = "https://tok@github.com/o/r";
        let cases = [
            "fatal: Remote branch dev not found in upstream origin",
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
            "fatal: repository 'https://github.com/o/r/' not found",
            "fatal: unable to access 'https://x/': Could not resolve host: x",
            "boom",
        ];
        let got: Vec<String> = cases
            .iter()
            .map(|c| clone_error(c, url, Some("dev")).to_string())
            .collect();
        assert!(got[0].contains("no branch or tag named dev"));
        assert!(got[1].contains("needs credentials"));
        assert!(got[2].contains("cannot reach"));
        assert!(got[3].contains("cannot reach"));
        assert_eq!(got[4], "git failed: boom");
        assert!(got.iter().all(|m| !m.contains("tok@")));
    }

    #[test]
    fn failure_messages_never_echo_credentials() {
        let err = failed("fatal: weird https://user:pw@host/o/r exploded");
        assert!(!err.to_string().contains("pw"));
    }
}
