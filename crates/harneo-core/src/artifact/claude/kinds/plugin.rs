//! Plugins: a folder holding `.claude-plugin/plugin.json`, kept whole, and the entries of a
//! `.claude-plugin/marketplace.json`, each of which is one Plugin as well.
//!
//! Diagnostic codes: `plugin.invalid-json`, `plugin.missing-name`, `marketplace.invalid-json`,
//! `marketplace.source-not-found`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanReport};

const DIR: &str = ".claude-plugin";
const MANIFEST: &str = "plugin.json";
const MARKETPLACE: &str = "marketplace.json";

/// The `plugin.json` of a plugin folder (relative to the scan root).
pub(crate) fn manifest_path(folder: &Path) -> PathBuf {
    folder.join(DIR).join(MANIFEST)
}

/// The `marketplace.json` of a marketplace folder (relative to the scan root).
pub(crate) fn marketplace_path(folder: &Path) -> PathBuf {
    folder.join(DIR).join(MARKETPLACE)
}

fn text(v: Option<Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

fn folder_name(folder: &Path) -> Option<String> {
    folder.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Pure: reads one plugin from its `plugin.json` text. `folder` is the plugin folder relative to
/// the scan root, `files` everything inside it.
pub(crate) fn parse(folder: &Path, files: Vec<String>, contents: &str) -> Artifact {
    let manifest = manifest_path(folder);
    let mut diagnostics = Vec::new();
    let mut metadata = match serde_json::from_str::<Value>(contents) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            diagnostics.push(Diagnostic::error(
                "plugin.invalid-json",
                &manifest,
                "plugin.json must hold a JSON object",
            ));
            Map::new()
        }
        Err(e) => {
            diagnostics.push(Diagnostic::error(
                "plugin.invalid-json",
                &manifest,
                &e.to_string(),
            ));
            Map::new()
        }
    };
    let declared = text(metadata.remove("name"));
    let description = text(metadata.remove("description"));
    if declared.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(
            "plugin.missing-name",
            &manifest,
            "a plugin needs a `name` in plugin.json",
        ));
    }
    Artifact {
        kind: ArtifactKind::Plugin,
        name: declared
            .or_else(|| folder_name(folder))
            .unwrap_or_else(|| "unnamed".into()),
        description,
        path: rel_string(folder),
        files,
        metadata,
        references: Vec::new(),
        diagnostics,
        digest: None,
        origin: crate::artifact::Origin::default(),
    }
}

/// Reads the plugin folder `folder`, which holds a `plugin.json`.
pub(crate) fn read_plugin(walker: &mut Walker, folder: &Path) -> Artifact {
    let mut walk_diagnostics = Vec::new();
    let files = walker.list_files(folder, &mut walk_diagnostics);
    let text = walker.read_text(&manifest_path(folder), &mut walk_diagnostics);
    let read = text.is_some();
    let mut artifact = parse(folder, files, &text.unwrap_or_default());
    if !read {
        // Not parsed: the walk's diagnostic says why.
        artifact.diagnostics.clear();
    }
    artifact.diagnostics.extend(walk_diagnostics);
    artifact
}

/// Adds the plugin in `folder` to the report.
pub(crate) fn scan_plugin(walker: &mut Walker, folder: &Path, report: &mut ScanReport) {
    report.artifacts.push(read_plugin(walker, folder));
}

/// A marketplace's own name and its plugin entries.
type Marketplace = (Option<String>, Vec<Map<String, Value>>);

/// Pure: the entries of a `marketplace.json` and its own name.
pub(crate) fn parse_marketplace(file: &Path, contents: &str) -> Result<Marketplace, Diagnostic> {
    let invalid = |msg: &str| Diagnostic::error("marketplace.invalid-json", file, msg);
    let Value::Object(mut root) =
        serde_json::from_str(contents).map_err(|e| invalid(&e.to_string()))?
    else {
        return Err(invalid("marketplace.json must hold a JSON object"));
    };
    let name = text(root.remove("name"));
    let entries = match root.remove("plugins") {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| match v {
                Value::Object(m) => Some(m),
                _ => None,
            })
            .collect(),
        Some(_) => return Err(invalid("`plugins` must be an array")),
        None => Vec::new(),
    };
    Ok((name, entries))
}

/// Adds one Plugin per entry of the marketplace in `folder`. Local sources are scanned as plugin
/// folders and pushed to `claimed`; remote sources are recorded as written, never fetched.
pub(crate) fn scan_marketplace(
    walker: &mut Walker,
    folder: &Path,
    repo: Option<&str>,
    claimed: &mut Vec<PathBuf>,
    report: &mut ScanReport,
) {
    let file = marketplace_path(folder);
    let mut diagnostics = Vec::new();
    let Some(contents) = walker.read_text(&file, &mut diagnostics) else {
        report.diagnostics.extend(diagnostics);
        return;
    };
    match parse_marketplace(&file, &contents) {
        Err(d) => report.diagnostics.push(d),
        Ok((market, entries)) => {
            for entry in entries {
                let ctx = Ctx {
                    mfolder: folder,
                    mfile: &file,
                    market: market.as_deref(),
                    repo,
                };
                let artifact = entry_artifact(walker, &ctx, entry, claimed);
                report.artifacts.push(artifact);
            }
        }
    }
    report.diagnostics.extend(diagnostics);
}

/// Where a marketplace entry is read from.
struct Ctx<'a> {
    mfolder: &'a Path,
    mfile: &'a Path,
    market: Option<&'a str>,
    /// The GitHub `owner/repo` being imported, when known.
    repo: Option<&'a str>,
}

/// `owner/repo` (lowercase) of a GitHub URL in any common spelling: `github.com/o/r`,
/// `https://github.com/o/r(.git)`, `git@github.com:o/r.git`, `ssh://git@github.com/o/r`.
/// Also accepts a bare `owner/repo`, which is how a `github` source writes it.
pub(crate) fn github_repo(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let path = match s.find("github.com") {
        Some(i) => s[i + "github.com".len()..].trim_start_matches([':', '/']),
        None if s.contains("://") || s.contains('@') || s.contains(':') => return None,
        None => s,
    };
    let mut parts = path.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    let bare = !s.contains("github.com");
    if owner.is_empty() || name.is_empty() || (bare && parts.next().is_some()) {
        return None;
    }
    Some(format!("{owner}/{name}").to_lowercase())
}

/// The repo a marketplace entry's remote source names (no sub-path), if it is on GitHub.
fn remote_repo(source: &Value) -> Option<String> {
    let obj = source.as_object()?;
    if obj.contains_key("path") {
        return None;
    }
    match obj.get("source")?.as_str()? {
        "github" => github_repo(obj.get("repo")?.as_str()?),
        "url" | "git" => github_repo(obj.get("url")?.as_str()?),
        _ => None,
    }
}

/// The GitHub repos a plugin's own `repository` / `homepage` fields name.
fn declared_repos(plugin: &Artifact) -> Vec<String> {
    let url = |v: &Value| match v {
        Value::String(s) => github_repo(s),
        Value::Object(o) => o.get("url").and_then(Value::as_str).and_then(github_repo),
        _ => None,
    };
    ["repository", "homepage"]
        .iter()
        .filter_map(|k| plugin.metadata.get(*k).and_then(url))
        .collect()
}

/// The plugin folder a remote entry designates when it names the repo being imported: the
/// plugin at the marketplace's own folder, with the same name.
fn same_repo_plugin(
    walker: &mut Walker,
    ctx: &Ctx<'_>,
    source: &Value,
    name: Option<&str>,
) -> Option<Artifact> {
    let target = remote_repo(source)?;
    if !walker.exists(&manifest_path(ctx.mfolder)) {
        return None;
    }
    let plugin = read_plugin(walker, ctx.mfolder);
    let same_repo = ctx.repo.is_some_and(|r| r.eq_ignore_ascii_case(&target))
        || declared_repos(&plugin).contains(&target);
    (same_repo && name == Some(plugin.name.as_str())).then_some(plugin)
}

fn entry_artifact(
    walker: &mut Walker,
    ctx: &Ctx<'_>,
    mut entry: Map<String, Value>,
    claimed: &mut Vec<PathBuf>,
) -> Artifact {
    let Ctx {
        mfolder,
        mfile,
        market,
        ..
    } = *ctx;
    let name = text(entry.remove("name"));
    let description = text(entry.remove("description"));
    let source = entry.remove("source").unwrap_or(Value::Null);
    let mut extra_diag = Vec::new();
    if name.is_none() {
        extra_diag.push(Diagnostic::error(
            "plugin.missing-name",
            mfile,
            "a marketplace entry needs a `name`",
        ));
    }

    let local = source.as_str().map(str::to_owned);
    let merged = if local.is_none() {
        same_repo_plugin(walker, ctx, &source, name.as_deref())
    } else {
        None
    };
    let is_local = local.is_some() || merged.is_some();
    let mut artifact = match &local {
        None if merged.is_some() => {
            claimed.push(mfolder.to_path_buf());
            merged.unwrap_or_else(|| remote_artifact(mfile, name.as_deref()))
        }
        Some(s) => match resolve_reference(mfolder, s) {
            Ok(folder) => {
                claimed.push(folder.clone());
                local_plugin(walker, &folder, mfile, &mut extra_diag)
            }
            Err(d) => {
                extra_diag.push(d);
                remote_artifact(mfile, name.as_deref())
            }
        },
        None => remote_artifact(mfile, name.as_deref()),
    };
    if let Some(n) = &name {
        artifact.name.clone_from(n);
    }
    if artifact.description.is_none() {
        artifact.description = description;
    }
    artifact.metadata.insert("source".into(), source);
    if let Some(m) = market {
        artifact.metadata.insert("marketplace".into(), m.into());
    }
    for (k, v) in entry {
        artifact.metadata.entry(k).or_insert(v);
    }
    if !is_local {
        artifact.path = format!("{}#{}", artifact.path, artifact.name);
    }
    artifact.diagnostics.extend(extra_diag);
    artifact
}

pub(crate) fn remote_artifact(mfile: &Path, name: Option<&str>) -> Artifact {
    Artifact {
        kind: ArtifactKind::Plugin,
        name: name.unwrap_or("unnamed").to_owned(),
        description: None,
        path: rel_string(mfile),
        files: vec![rel_string(mfile)],
        metadata: Map::new(),
        references: Vec::new(),
        diagnostics: Vec::new(),
        digest: None,
        origin: crate::artifact::Origin::default(),
    }
}

fn local_plugin(
    walker: &mut Walker,
    folder: &Path,
    mfile: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Artifact {
    if walker.exists(&manifest_path(folder)) {
        return read_plugin(walker, folder);
    }
    let mut walk_diagnostics = Vec::new();
    let files = walker.list_files(folder, &mut walk_diagnostics);
    if !walker.exists(folder) {
        diags.push(Diagnostic::warning(
            "marketplace.source-not-found",
            mfile,
            &format!("the local source `{}` does not exist", rel_string(folder)),
        ));
    }
    let mut artifact = remote_artifact(mfile, folder_name(folder).as_deref());
    artifact.path = rel_string(folder);
    artifact.files = files;
    artifact.diagnostics = walk_diagnostics;
    artifact
}

/// `CONTEXT.md` §2: A plugin is never unpacked: it is enabled through `enabledPlugins` in
/// `.claude/settings.json`, so that is the file it installs to.
pub(crate) fn install_path(_artifact: &crate::artifact::Artifact) -> std::path::PathBuf {
    use std::path::PathBuf;
    PathBuf::from(".claude/settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    fn plugin(contents: &str) -> Artifact {
        parse(
            Path::new("p/tool"),
            vec!["p/tool/.claude-plugin/plugin.json".into()],
            contents,
        )
    }

    #[test]
    fn reads_name_description_and_version() {
        let a = plugin(r#"{"name":"tool","description":"d","version":"1.2.0"}"#);
        assert_eq!(a.name, "tool");
        assert_eq!(a.description.as_deref(), Some("d"));
        assert_eq!(a.metadata["version"], "1.2.0");
        assert!(!a.metadata.contains_key("name"));
        assert_eq!(a.path, "p/tool");
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn invalid_json_triggers_and_valid_does_not() {
        let bad = plugin("{nope");
        assert_eq!(codes(&bad), ["plugin.invalid-json"]);
        assert_eq!(bad.name, "tool");
        assert_eq!(codes(&plugin("[1]")), ["plugin.invalid-json"]);
        assert!(!codes(&plugin(r#"{"name":"x"}"#)).contains(&"plugin.invalid-json"));
    }

    #[test]
    fn missing_name_triggers_and_present_does_not() {
        let a = plugin(r#"{"version":"1"}"#);
        assert_eq!(codes(&a), ["plugin.missing-name"]);
        assert_eq!(a.name, "tool");
        assert_eq!(codes(&plugin(r#"{"name":"  "}"#)), ["plugin.missing-name"]);
        assert!(plugin(r#"{"name":"x"}"#).diagnostics.is_empty());
    }

    #[test]
    fn marketplace_parsing() {
        let f = Path::new(".claude-plugin/marketplace.json");
        let err = parse_marketplace(f, "{").err().map(|d| d.code);
        assert_eq!(err.as_deref(), Some("marketplace.invalid-json"));
        let ok = parse_marketplace(f, r#"{"name":"m","plugins":[{"name":"a"},3]}"#);
        assert_eq!(
            ok.map(|(n, e)| (n, e.len())).ok(),
            Some((Some("m".into()), 1))
        );
        assert!(parse_marketplace(f, r#"{"plugins":5}"#).is_err());
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
            install(ArtifactKind::Plugin, "plugins/x"),
            std::path::Path::new(".claude/settings.json")
        );
    }
}
