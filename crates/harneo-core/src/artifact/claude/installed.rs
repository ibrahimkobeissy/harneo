//! Plugins of a `~/.claude`-like root, read from `plugins/installed_plugins.json` (what is
//! really installed) instead of from the catalogue clones under `plugins/marketplaces/`.
//! `plugins/known_marketplaces.json` says where each marketplace comes from.
//!
//! Diagnostic codes: `installed.invalid-json`, `installed.path-outside-cache`,
//! `installed.not-found`, `installed.invalid-key`, `marketplaces.invalid-json`.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

use super::kinds::plugin;
use crate::artifact::walk::{Walker, INSTALLED_PLUGINS};
use crate::artifact::{rel_string, Artifact, Diagnostic, Origin, ScanReport};

const KNOWN_MARKETPLACES: &str = "plugins/known_marketplaces.json";
const CACHE: &str = "plugins/cache";

/// Pure: `owner/repo` from a `known_marketplaces.json` entry. The source may be nested under
/// `source` (`{"source": "github", "repo": "o/r"}` or `{"source": "git", "url": "..."}`).
fn repo_of(entry: &Value) -> Option<String> {
    let src = entry.get("source").unwrap_or(entry);
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    if let Some(r) = text(src, "repo").or_else(|| text(entry, "repo")) {
        return Some(r.trim_end_matches(".git").to_owned());
    }
    let url = text(src, "url").or_else(|| src.as_str().map(str::to_owned))?;
    let url = url.trim_end_matches('/').trim_end_matches(".git");
    // `https://host/owner/repo` or `git@host:owner/repo`.
    let tail = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map(|(_, path)| path)?,
        None => url.split_once(':').map(|(_, path)| path)?,
    };
    let parts: Vec<&str> = tail.split('/').filter(|p| !p.is_empty()).collect();
    (parts.len() == 2).then(|| parts.join("/"))
}

/// True for a repo under the `anthropics` organisation.
fn is_official(repo: &str) -> bool {
    repo.split('/')
        .next()
        .is_some_and(|org| org.eq_ignore_ascii_case("anthropics"))
}

/// Pure: the marketplace origin for `name` given the parsed `known_marketplaces.json`, and the
/// marketplace's source as written.
fn origin_for(name: &str, known: &Map<String, Value>) -> (Origin, Option<Value>) {
    let entry = known.get(name);
    let repo = entry.and_then(repo_of);
    let official = repo.as_deref().is_some_and(is_official);
    let source = entry.map(|e| e.get("source").unwrap_or(e).clone());
    (
        Origin::Marketplace {
            name: name.to_owned(),
            repo,
            official,
        },
        source,
    )
}

/// Maps the absolute `install_path` to a path relative to the scan root, only when it lies
/// under `<root>/plugins/cache/` and no folder on the way is a symlink or `..`.
fn cache_relative(walker: &Walker, install_path: &str) -> Option<PathBuf> {
    let given = Path::new(install_path);
    let rel = if given.is_absolute() {
        let root = walker.root();
        let canonical = root.canonicalize().ok();
        given
            .strip_prefix(root)
            .ok()
            .or_else(|| {
                canonical
                    .as_deref()
                    .and_then(|c| given.strip_prefix(c).ok())
            })?
            .to_path_buf()
    } else {
        given.to_path_buf()
    };
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let under = rel.starts_with(CACHE) && rel.components().count() > 2;
    // Every folder on the way must be real: a symlink could lead out of the root.
    let real = (1..=rel.components().count())
        .all(|n| walker.is_dir(&rel.components().take(n).collect::<PathBuf>()));
    (under && real).then_some(rel)
}

/// One installed plugin: its key `name@marketplace` and its install entries.
fn read_one(
    walker: &mut Walker,
    key: &str,
    entries: &[Value],
    known: &Map<String, Value>,
    report: &mut ScanReport,
) {
    let list = Path::new(INSTALLED_PLUGINS);
    let (plugin_name, market) = key.rsplit_once('@').unwrap_or((key, ""));
    if plugin_name.is_empty() || market.is_empty() {
        report.diagnostics.push(Diagnostic::warning(
            "installed.invalid-key",
            list,
            &format!("`{key}` is not of the form name@marketplace"),
        ));
        return;
    }
    let mut folder = None;
    let mut scopes = Vec::new();
    let mut version = None;
    for entry in entries {
        let text = |k: &str| entry.get(k).and_then(Value::as_str);
        let mut scope = Map::new();
        for k in ["scope", "projectPath", "version"] {
            if let Some(v) = text(k) {
                scope.insert(k.into(), v.into());
            }
        }
        scopes.push(Value::Object(scope));
        let Some(path) = text("installPath") else {
            continue;
        };
        match cache_relative(walker, path) {
            Some(rel) if folder.is_none() => {
                folder = Some(rel);
                version = text("version").map(str::to_owned);
            }
            Some(_) => {}
            None => report.diagnostics.push(Diagnostic::error(
                "installed.path-outside-cache",
                list,
                &format!("`{key}`: installPath `{path}` is not a real folder under plugins/cache/ of this root; it was not read"),
            )),
        }
    }
    let Some(folder) = folder else {
        return;
    };
    let (origin, source) = origin_for(market, known);
    let mut artifact = installed_artifact(walker, &folder, plugin_name);
    if let Some(v) = version {
        artifact.metadata.insert("version".into(), v.into());
    }
    artifact.metadata.insert("pluginId".into(), key.into());
    artifact
        .metadata
        .insert("marketplace".into(), market.into());
    artifact
        .metadata
        .insert("scopes".into(), Value::Array(scopes));
    if let Some(s) = source {
        artifact.metadata.insert("marketplaceSource".into(), s);
    }
    artifact.origin = origin;
    report.artifacts.push(artifact);
}

/// The plugin in `folder`; without a `plugin.json` it is named after the installed key.
fn installed_artifact(walker: &mut Walker, folder: &Path, plugin_name: &str) -> Artifact {
    if walker.exists(&plugin::manifest_path(folder)) {
        return plugin::read_plugin(walker, folder);
    }
    let mut diags = Vec::new();
    let files = walker.list_files(folder, &mut diags);
    let mut a = plugin::remote_artifact(Path::new(INSTALLED_PLUGINS), Some(plugin_name));
    a.path = rel_string(folder);
    a.files = files;
    a.diagnostics = diags;
    a.diagnostics.push(Diagnostic::warning(
        "installed.not-found",
        folder,
        "the installed plugin folder has no .claude-plugin/plugin.json",
    ));
    a
}

/// Parses a JSON object file; a malformed one is reported under `code` and gives `None`.
fn read_object(
    walker: &mut Walker,
    rel: &str,
    code: &str,
    report: &mut ScanReport,
) -> Option<Map<String, Value>> {
    let path = Path::new(rel);
    let mut diags = Vec::new();
    let text = walker.read_text(path, &mut diags);
    report.diagnostics.extend(diags);
    match serde_json::from_str::<Value>(&text?) {
        Ok(Value::Object(m)) => Some(m),
        Ok(_) => {
            report
                .diagnostics
                .push(Diagnostic::error(code, path, "must hold a JSON object"));
            None
        }
        Err(e) => {
            report
                .diagnostics
                .push(Diagnostic::error(code, path, &e.to_string()));
            None
        }
    }
}

/// Adds one Plugin per `name@marketplace` of `plugins/installed_plugins.json`, when the root has
/// one. A malformed list gives a diagnostic and no plugins.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    if !walker.installed_mode() {
        return;
    }
    let Some(list) = read_object(walker, INSTALLED_PLUGINS, "installed.invalid-json", report)
    else {
        return;
    };
    let known = if walker.is_file(Path::new(KNOWN_MARKETPLACES)) {
        read_object(
            walker,
            KNOWN_MARKETPLACES,
            "marketplaces.invalid-json",
            report,
        )
        .unwrap_or_default()
    } else {
        Map::new()
    };
    let Some(Value::Object(plugins)) = list.get("plugins") else {
        report.diagnostics.push(Diagnostic::error(
            "installed.invalid-json",
            Path::new(INSTALLED_PLUGINS),
            "`plugins` must be an object",
        ));
        return;
    };
    for (key, value) in plugins {
        let entries = match value {
            Value::Array(items) => items.clone(),
            Value::Object(_) => vec![value.clone()],
            _ => Vec::new(),
        };
        read_one(walker, key, &entries, &known, report);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn repo_from_nested_github_and_git_url() {
        let gh = json!({"source": {"source": "github", "repo": "anthropics/x"}});
        assert_eq!(repo_of(&gh).as_deref(), Some("anthropics/x"));
        let git = json!({"source": {"source": "git", "url": "https://github.com/addyosmani/agent-skills.git"}});
        assert_eq!(repo_of(&git).as_deref(), Some("addyosmani/agent-skills"));
        let ssh = json!({"source": {"url": "git@github.com:o/r.git"}});
        assert_eq!(repo_of(&ssh).as_deref(), Some("o/r"));
        assert_eq!(repo_of(&json!({"repo": "o/r"})).as_deref(), Some("o/r"));
        assert_eq!(
            repo_of(&json!({"source": {"source": "directory", "path": "/x"}})),
            None
        );
    }

    #[test]
    fn official_only_for_anthropics() {
        assert!(is_official("anthropics/claude-plugins-official"));
        assert!(!is_official("anthropics-fake/x"));
        assert!(!is_official("addyosmani/agent-skills"));
    }
}
