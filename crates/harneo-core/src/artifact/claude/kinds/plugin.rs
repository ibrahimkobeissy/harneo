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
    }
}

/// Reads the plugin folder `folder`, which holds a `plugin.json`.
fn read_plugin(walker: &mut Walker, folder: &Path) -> Artifact {
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
                let artifact =
                    entry_artifact(walker, folder, &file, market.as_deref(), entry, claimed);
                report.artifacts.push(artifact);
            }
        }
    }
    report.diagnostics.extend(diagnostics);
}

fn entry_artifact(
    walker: &mut Walker,
    mfolder: &Path,
    mfile: &Path,
    market: Option<&str>,
    mut entry: Map<String, Value>,
    claimed: &mut Vec<PathBuf>,
) -> Artifact {
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
    let mut artifact = match &local {
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
    if local.is_none() {
        artifact.path = format!("{}#{}", artifact.path, artifact.name);
    }
    artifact.diagnostics.extend(extra_diag);
    artifact
}

fn remote_artifact(mfile: &Path, name: Option<&str>) -> Artifact {
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
