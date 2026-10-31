//! Installs a project's plugins through the user's `claude` CLI, from each plugin's own
//! marketplace, so a scaffolded or refitted project is ready the first time Claude Code opens it.
//!
//! The project's `.claude/settings.json` says what to install: every `enabledPlugins` entry set
//! to `true`, with its marketplace's source from `extraKnownMarketplaces`. A marketplace the
//! machine does not know yet is added (in the user's settings, as Claude Code does when it
//! offers it), then each plugin not installed for this project is installed with
//! `--scope project`. Nothing is copied or repackaged, and no model is called. The settings file
//! is left as it is: the install only records what it already enables.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::bundles::{run_subcommand, ClaudeConfig};
use crate::error::{Error, Result};

/// The marketplace Claude Code knows without being told.
const OFFICIAL: &str = "claude-plugins-official";
const OFFICIAL_REPO: &str = "anthropics/claude-plugins-official";
/// The longest a listing may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest adding a marketplace (a clone) or installing one plugin may take.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(180);
/// How much of a failure is shown.
const MESSAGE_CHARS: usize = 300;

/// One plugin the project's settings enable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTarget {
    /// `name@marketplace`, as in `enabledPlugins`.
    pub id: String,
    pub name: String,
    pub marketplace: String,
    /// Where the marketplace comes from (`owner/repo` or a git URL); `None` when the settings
    /// do not say (the official marketplace needs none).
    pub source: Option<String>,
}

/// How one plugin's install went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum PluginInstallStatus {
    /// Installed now, for this project.
    Installed,
    /// Already installed for this project, or for the user (every project).
    AlreadyInstalled,
    /// Not installed; `message` says why.
    Failed,
}

/// One plugin's result, for the Done step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PluginInstall {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub status: PluginInstallStatus,
    /// Why it failed, in one line; `None` otherwise.
    pub message: Option<String>,
}

/// The plugins `project`'s `.claude/settings.json` enables; none when there is no such file.
pub fn targets(project: &Path) -> Result<Vec<PluginTarget>> {
    let path = project.join(".claude").join("settings.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::Io {
                path: path.display().to_string(),
                source,
            })
        }
    };
    let settings: Value =
        serde_json::from_slice(&bytes).map_err(|_| Error::UnreadableSettings {
            path: ".claude/settings.json".to_owned(),
        })?;
    Ok(targets_of(&settings))
}

fn targets_of(settings: &Value) -> Vec<PluginTarget> {
    let Some(enabled) = settings.get("enabledPlugins").and_then(Value::as_object) else {
        return Vec::new();
    };
    let known = settings.get("extraKnownMarketplaces");
    let mut out: Vec<PluginTarget> = enabled
        .iter()
        .filter(|(_, on)| on.as_bool() == Some(true))
        .filter_map(|(id, _)| {
            let (name, marketplace) = id.split_once('@')?;
            if name.is_empty() || marketplace.is_empty() {
                return None;
            }
            let source = known
                .and_then(|k| k.get(marketplace))
                .and_then(|m| m.get("source"))
                .and_then(source_of);
            Some(PluginTarget {
                id: id.clone(),
                name: name.to_owned(),
                marketplace: marketplace.to_owned(),
                source,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// `owner/repo` for a `github` source, the URL for a `git` or `url` one, the path for a
/// `directory` one.
fn source_of(source: &Value) -> Option<String> {
    let field = match source.get("source").and_then(Value::as_str)? {
        "github" => "repo",
        "git" | "url" => "url",
        "directory" => "path",
        _ => return None,
    };
    source
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

/// A marketplace source in one comparable form: `github.com/owner/repo`, lowercased, for a
/// GitHub repo given as `owner/repo`, an https or ssh URL, with or without `.git`.
fn normalized(source: &str) -> String {
    let s = source.trim().trim_end_matches('/').to_lowercase();
    let s = s.strip_suffix(".git").unwrap_or(&s);
    for prefix in ["https://", "http://", "ssh://git@", "git@"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return rest.replacen(':', "/", 1);
        }
    }
    if s.split('/').count() == 2 && !s.starts_with('.') && !s.starts_with('/') {
        return format!("github.com/{s}");
    }
    s.to_owned()
}

/// A marketplace the machine knows: its name and source, from `claude plugin marketplace list`.
#[derive(Debug, Deserialize)]
struct KnownMarketplace {
    name: String,
    repo: Option<String>,
    url: Option<String>,
    path: Option<String>,
}

impl KnownMarketplace {
    fn source(&self) -> Option<&str> {
        self.repo
            .as_deref()
            .or(self.url.as_deref())
            .or(self.path.as_deref())
    }
}

/// An installed plugin, from `claude plugin list`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Installed {
    id: String,
    scope: String,
    project_path: Option<String>,
}

/// The last JSON line a `--json` subcommand printed: `outcome` and `message`.
#[derive(Debug, Deserialize)]
struct Outcome {
    outcome: String,
    message: Option<String>,
}

fn one_line(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = flat.chars().take(MESSAGE_CHARS).collect();
    if flat.chars().count() > MESSAGE_CHARS {
        out.push('…');
    }
    out
}

/// The failure of a subcommand run: its JSON message, else its error output.
fn failure_text(stdout: &str, stderr: &str) -> String {
    let from_json = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Outcome>(l.trim()).ok())
        .and_then(|o| o.message);
    let text = from_json.unwrap_or_else(|| {
        let err = stderr.trim();
        if err.is_empty() {
            stdout.trim().to_owned()
        } else {
            err.to_owned()
        }
    });
    if text.is_empty() {
        "claude reported a failure without a reason".to_owned()
    } else {
        one_line(&text)
    }
}

/// Runs a `--json` subcommand and says whether its outcome was `ok`, with the reason if not.
fn run_json(
    config: &ClaudeConfig,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> std::result::Result<(), String> {
    let run = run_subcommand(config, args, cwd, timeout).map_err(|e| why(&e))?;
    let ok = run
        .stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Outcome>(l.trim()).ok())
        .map_or(run.success, |o| o.outcome == "ok");
    if ok && run.success {
        Ok(())
    } else {
        Err(failure_text(&run.stdout, &run.stderr))
    }
}

fn why(error: &Error) -> String {
    match error {
        Error::ClaudeMissing => {
            "the claude CLI was not found: install Claude Code, then retry".to_owned()
        }
        other => one_line(&other.to_string()),
    }
}

fn list<T: for<'de> Deserialize<'de>>(
    config: &ClaudeConfig,
    args: &[&str],
    cwd: Option<&Path>,
) -> std::result::Result<Vec<T>, String> {
    let run = run_subcommand(config, args, cwd, LIST_TIMEOUT).map_err(|e| why(&e))?;
    if !run.success {
        return Err(failure_text(&run.stdout, &run.stderr));
    }
    serde_json::from_str(run.stdout.trim())
        .map_err(|_| "claude printed a list Harneo could not read".to_owned())
}

/// Installs every plugin `project`'s settings enable, calling `progress` with each result as
/// it lands. A plugin already installed (for this project or the user) is left alone.
pub fn install_plugins(
    config: &ClaudeConfig,
    project: &Path,
    mut progress: impl FnMut(&PluginInstall),
) -> Result<Vec<PluginInstall>> {
    let wanted = targets(project)?;
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let project = project.canonicalize().map_err(|source| Error::Io {
        path: project.display().to_string(),
        source,
    })?;
    let mut results = Vec::new();
    let mut finish = |target: &PluginTarget, status, message: Option<String>| {
        let result = PluginInstall {
            id: target.id.clone(),
            name: target.name.clone(),
            marketplace: target.marketplace.clone(),
            status,
            message,
        };
        progress(&result);
        results.push(result);
    };

    // Without the lists nothing can be done right: every plugin fails with the same reason.
    let lists =
        list::<KnownMarketplace>(config, &["plugin", "marketplace", "list", "--json"], None)
            .and_then(|markets| {
                list::<Installed>(config, &["plugin", "list", "--json"], Some(&project))
                    .map(|installed| (markets, installed))
            });
    let (markets, installed) = match lists {
        Ok(lists) => lists,
        Err(reason) => {
            for target in &wanted {
                finish(target, PluginInstallStatus::Failed, Some(reason.clone()));
            }
            return Ok(results);
        }
    };
    let here = project.display().to_string();
    let present: HashSet<&str> = installed
        .iter()
        .filter(|p| {
            p.scope == "user"
                || (p.scope != "user" && p.project_path.as_deref() == Some(here.as_str()))
        })
        .map(|p| p.id.as_str())
        .collect();

    // Each marketplace once: known with the same source, added, or the reason it cannot be.
    let mut ready: BTreeMap<String, std::result::Result<(), String>> = BTreeMap::new();
    for target in wanted.iter().filter(|t| !present.contains(t.id.as_str())) {
        if ready.contains_key(&target.marketplace) {
            continue;
        }
        let known = markets.iter().find(|m| m.name == target.marketplace);
        let source = target
            .source
            .clone()
            .or_else(|| (target.marketplace == OFFICIAL).then(|| OFFICIAL_REPO.to_owned()));
        let state = match (known, source) {
            (Some(m), Some(wanted_source)) => match m.source() {
                Some(have) if normalized(have) != normalized(&wanted_source) => Err(format!(
                    "a different marketplace named \"{}\" is already added on this machine ({have}, not {wanted_source})",
                    target.marketplace
                )),
                _ => Ok(()),
            },
            (Some(_), None) => Ok(()),
            (None, Some(source)) => run_json(
                config,
                &["plugin", "marketplace", "add", &source, "--json"],
                None,
                NETWORK_TIMEOUT,
            )
            .map_err(|e| format!("its marketplace could not be added: {e}")),
            (None, None) => Err(format!(
                "the settings do not say where the \"{}\" marketplace comes from",
                target.marketplace
            )),
        };
        ready.insert(target.marketplace.clone(), state);
    }

    for target in &wanted {
        if present.contains(target.id.as_str()) {
            finish(target, PluginInstallStatus::AlreadyInstalled, None);
            continue;
        }
        if let Some(Err(reason)) = ready.get(&target.marketplace) {
            finish(target, PluginInstallStatus::Failed, Some(reason.clone()));
            continue;
        }
        match run_json(
            config,
            &[
                "plugin", "install", &target.id, "--scope", "project", "--json",
            ],
            Some(&project),
            NETWORK_TIMEOUT,
        ) {
            Ok(()) => finish(target, PluginInstallStatus::Installed, None),
            Err(reason) => finish(target, PluginInstallStatus::Failed, Some(reason)),
        }
    }
    Ok(results)
}
