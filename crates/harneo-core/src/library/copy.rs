//! Copies one scanned artifact into a destination folder.
//!
//! Every source path comes from the scan (relative, inside the root), but it is treated as
//! untrusted anyway: it is checked again here, joined under the root or the destination only
//! when it is made of plain names, and never read through a symlink. Only regular files are
//! copied; anything the scan refused to read (symlinks, secret-named and local files, files over
//! the size limit) is skipped and reported, never copied.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::artifact::literal_secret::{is_literal_secret, REDACTED};
use crate::artifact::walk::{is_local_name, is_secret_name};
use crate::artifact::{Artifact, ArtifactKind, ScanOptions};
use crate::error::{Error, Result};

/// Name of the stored form of an MCP server.
pub(crate) const SERVER_FILE: &str = "server.json";
/// The file an MCP server is defined in, in the source.
const MCP_FILE: &str = ".mcp.json";

/// Why a listed file was not copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SkipReason {
    /// A symlink, a folder or anything else that is not a regular file, or a file that vanished.
    NotRegularFile,
    /// The name looks like a secret or credential file.
    SecretFile,
    /// A per-machine file such as `settings.local.json`.
    LocalFile,
    /// Larger than the copy's per-file limit: the scan's, or the large-file ceiling when the
    /// import opted in.
    TooLarge,
}

/// A file the scan listed that the copy left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SkippedFile {
    /// The artifact the file belongs to.
    pub artifact_id: String,
    /// The file, relative to the source root.
    pub file: String,
    pub reason: SkipReason,
}

/// Failure injection for tests of the atomic import. The real library uses [`NoHooks`].
pub(crate) trait ImportHooks {
    /// Called before each file is written into an artifact's temporary folder.
    fn before_file(&self, _artifact_id: &str, _dest_rel: &str) -> io::Result<()> {
        Ok(())
    }
    /// Called after every folder is in place and before the index transaction commits.
    fn before_commit(&self) -> io::Result<()> {
        Ok(())
    }
}

/// The production hooks: never fail.
pub(crate) struct NoHooks;
impl ImportHooks for NoHooks {}

/// `rel` as a list of plain names, or `UnsafePath` when it is absolute, empty, or has `..`, `.`
/// or a prefix in it.
fn plain_components(rel: &str) -> Result<Vec<&std::ffi::OsStr>> {
    let unsafe_path = || Error::UnsafePath { path: rel.into() };
    let mut parts = Vec::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(name) => parts.push(name),
            _ => return Err(unsafe_path()),
        }
    }
    if parts.is_empty() {
        return Err(unsafe_path());
    }
    Ok(parts)
}

/// `base` joined with `rel`, after checking `rel` cannot leave `base`.
pub(crate) fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    Ok(plain_components(rel)?
        .into_iter()
        .fold(base.to_path_buf(), |acc, name| acc.join(name)))
}

/// The folder the artifact's files are stored relative to: its own folder when everything lives
/// in it, otherwise its parent folder when everything lives there, otherwise the source root.
/// Chosen once per artifact so stored paths never collide.
fn store_base(artifact: &Artifact, files: &[&str]) -> String {
    let own = artifact.path.split('#').next().unwrap_or_default();
    let parent = own.rsplit_once('/').map_or("", |(parent, _)| parent);
    let all_in =
        |base: &str| base.is_empty() || files.iter().all(|f| f.starts_with(&format!("{base}/")));
    if !own.is_empty() && all_in(own) {
        own.to_owned()
    } else if all_in(parent) {
        parent.to_owned()
    } else {
        String::new()
    }
}

/// Where each listed file goes: `(path in the source, path in the store)`.
pub(crate) fn plan(artifact: &Artifact) -> Result<Vec<(String, String)>> {
    let listed: Vec<&str> = artifact
        .files
        .iter()
        .map(String::as_str)
        // An MCP server is stored as `server.json`, not as the shared `.mcp.json`.
        .filter(|f| !(artifact.kind == ArtifactKind::Mcp && *f == MCP_FILE))
        .collect();
    let base = store_base(artifact, &listed);
    let mut out = Vec::new();
    for file in listed {
        plain_components(file)?;
        let dest = if base.is_empty() {
            file
        } else {
            &file[base.len() + 1..]
        };
        if artifact.kind == ArtifactKind::Mcp && dest == SERVER_FILE {
            return Err(Error::UnsafePath { path: file.into() });
        }
        out.push((file.to_owned(), dest.to_owned()));
    }
    Ok(out)
}

/// The artifact as it looks inside its library copy, for auditing the copy: `files` (and `path`
/// when it is one of them) are the stored paths, relative to the artifact's folder. An MCP
/// server's stored `server.json` is left out, since its definition is the artifact's metadata.
/// Also returns the map from stored path back to source path.
pub(crate) fn stored_view(
    artifact: &Artifact,
) -> Result<(Artifact, std::collections::HashMap<String, String>)> {
    let mut view = artifact.clone();
    let mut back = std::collections::HashMap::new();
    view.files.clear();
    for (source, dest) in plan(artifact)? {
        if source == artifact.path {
            view.path.clone_from(&dest);
        }
        back.insert(dest.clone(), source);
        view.files.push(dest);
    }
    if artifact.kind == ArtifactKind::Mcp {
        view.files.retain(|f| f != SERVER_FILE);
    }
    Ok((view, back))
}

/// Upper snake case for a key name: `apiKey`, `X-Api-Key` and `api_key` all become `API_KEY`.
fn upper_snake(key: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in key.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push('_');
            }
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c.to_ascii_uppercase());
        } else {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    let trimmed = out.trim_end_matches('_');
    if trimmed.is_empty() {
        "SECRET".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `${KEY}` for a key name.
pub(super) fn placeholder(key: &str) -> String {
    format!("${{{}}}", upper_snake(key))
}

/// The auth scheme a header value starts with (`Bearer `, with its original case), if any.
fn auth_scheme(value: &str) -> Option<&str> {
    let value = value.trim();
    ["Bearer ", "Basic ", "Token "].iter().find_map(|scheme| {
        value
            .get(..scheme.len())
            .filter(|p| p.eq_ignore_ascii_case(scheme))
    })
}

/// The `field` map (`env` or `headers`) of the server `name` in the source's `.mcp.json`, read
/// only to recover the auth scheme the scan's `<redacted>` marker dropped. Any failure gives an
/// empty map: the placeholder then simply has no scheme in front.
fn source_map(source_root: &Path, name: &str, field: &str) -> Map<String, Value> {
    let read = || -> Option<Map<String, Value>> {
        if refusal(source_root, MCP_FILE, u64::MAX).ok()?.is_some() {
            return None;
        }
        let bytes = fs::read(safe_join(source_root, MCP_FILE).ok()?).ok()?;
        let root: Value = serde_json::from_slice(&bytes).ok()?;
        root.get("mcpServers")?
            .get(name)?
            .get(field)?
            .as_object()
            .cloned()
    };
    read().unwrap_or_default()
}

/// The `field` entries of `metadata` the scan redacted, as their key names.
fn redacted_keys(metadata: &Map<String, Value>, field: &str) -> Vec<String> {
    metadata
        .get(field)
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter(|(_, v)| v.as_str() == Some(REDACTED))
                .map(|(k, _)| format!("{field}.{k}"))
                .collect()
        })
        .unwrap_or_default()
}

/// The keys whose literal secrets the copy of `artifact` replaces, as `env.<KEY>` and
/// `headers.<KEY>`, sorted. Read from the scan's metadata, in which only the redacted entries
/// still say `<redacted>`; never carries a value.
pub(crate) fn replaced_secret_keys(artifact: &Artifact) -> Vec<String> {
    let fields: &[&str] = match artifact.kind {
        ArtifactKind::Settings => &["env"],
        ArtifactKind::Mcp => &["env", "headers"],
        _ => &[],
    };
    let mut keys: Vec<String> = fields
        .iter()
        .flat_map(|f| redacted_keys(&artifact.metadata, f))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// The definition an MCP server is stored with, as `{ "<name>": { ... } }`, and the keys that
/// were replaced.
///
/// The scan's metadata has literal secrets as `<redacted>`; each becomes `${KEY}`, keeping the
/// `Bearer `-style scheme of the original header when the source still has it.
fn mcp_definition(source_root: &Path, artifact: &Artifact) -> (Value, Vec<String>) {
    let mut definition = artifact.metadata.clone();
    // Scan bookkeeping, not part of the server's own definition.
    definition.remove("layoutBy");
    for field in ["env", "headers"] {
        if !definition.contains_key(field) {
            continue;
        }
        let original = source_map(source_root, &artifact.name, field);
        let Some(Value::Object(map)) = definition.get_mut(field) else {
            continue;
        };
        for (key, value) in map.iter_mut() {
            if value.as_str() != Some(REDACTED) {
                continue;
            }
            let scheme = original
                .get(key)
                .and_then(Value::as_str)
                .and_then(auth_scheme)
                .unwrap_or_default();
            *value = Value::String(format!("{scheme}{}", placeholder(key)));
        }
    }
    let mut outer = Map::new();
    outer.insert(artifact.name.clone(), Value::Object(definition));
    (Value::Object(outer), replaced_secret_keys(artifact))
}

/// Rewrites a file's bytes on the way into the store, and returns the keys it replaced. A
/// Settings file has its literal `env` secrets replaced by `${KEY}` and is written
/// pretty-printed (unchanged when it held none); a Settings file that is not valid JSON is
/// refused. Every other file is copied byte for byte.
pub(crate) fn rewrite(
    artifact: &Artifact,
    src_rel: &str,
    bytes: Vec<u8>,
) -> Result<(Vec<u8>, Vec<String>)> {
    if artifact.kind != ArtifactKind::Settings || src_rel != artifact.path {
        return Ok((bytes, Vec::new()));
    }
    let unreadable = || Error::UnreadableSettings {
        path: src_rel.to_owned(),
    };
    let mut root: Value = serde_json::from_slice(&bytes).map_err(|_| unreadable())?;
    let mut keys = Vec::new();
    if let Some(Value::Object(env)) = root.get_mut("env") {
        for (key, value) in env.iter_mut() {
            if value.as_str().is_some_and(|v| is_literal_secret(key, v)) {
                *value = Value::String(placeholder(key));
                keys.push(format!("env.{key}"));
            }
        }
    }
    if keys.is_empty() {
        return Ok((bytes, keys));
    }
    Ok((serde_json::to_vec_pretty(&root)?, keys))
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Why `rel` (under `root`) must not be copied, or `None` for a regular file that may be.
fn refusal(root: &Path, rel: &str, max_file_bytes: u64) -> Result<Option<SkipReason>> {
    let parts = plain_components(rel)?;
    // No folder on the way may be a symlink: the scan never followed one.
    let mut current = root.to_path_buf();
    for name in &parts[..parts.len() - 1] {
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(Error::UnsafePath { path: rel.into() }),
            Err(_) => return Ok(Some(SkipReason::NotRegularFile)),
        }
    }
    let name = parts[parts.len() - 1].to_string_lossy();
    current.push(&*name);
    let Ok(meta) = fs::symlink_metadata(&current) else {
        return Ok(Some(SkipReason::NotRegularFile));
    };
    Ok(if !meta.file_type().is_file() {
        Some(SkipReason::NotRegularFile)
    } else if is_secret_name(&name) {
        Some(SkipReason::SecretFile)
    } else if is_local_name(&name) {
        Some(SkipReason::LocalFile)
    } else if meta.len() > max_file_bytes {
        Some(SkipReason::TooLarge)
    } else {
        None
    })
}

/// Whether the file at `path` has any executable bit (unix); `false` elsewhere or when unreadable.
pub(crate) fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// Makes the file at `path` executable by its owner, group and others (0o755); a no-op off unix.
pub(crate) fn make_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// What copying one artifact did.
pub(crate) struct Copied {
    /// Listed files that were left out.
    pub skipped: Vec<SkippedFile>,
    /// Keys (`env.<KEY>`, `headers.<KEY>`) whose literal secrets became placeholders.
    pub replaced_secret_keys: Vec<String>,
}

/// The largest file an import that opted in to large files copies: 100 MiB.
pub const LARGE_FILE_CEILING: u64 = 100 * 1024 * 1024;

/// The per-file copy limit: the scan's (1 MiB), or [`LARGE_FILE_CEILING`] when the import or
/// the artifact's source opted in to large files.
pub(crate) fn copy_limit(large_files: bool) -> u64 {
    if large_files {
        LARGE_FILE_CEILING
    } else {
        ScanOptions::default().max_file_bytes as u64
    }
}

/// Copies `artifact` from `source_root` into the existing, empty folder `dest`, leaving out
/// files over `max` bytes.
pub(crate) fn copy_artifact(
    source_root: &Path,
    artifact: &Artifact,
    artifact_id: &str,
    dest: &Path,
    hooks: &dyn ImportHooks,
    max: u64,
) -> Result<Copied> {
    let mut skipped = Vec::new();
    let mut replaced = Vec::new();
    let write = |dest_rel: &str, bytes: Vec<u8>, executable: bool| -> Result<()> {
        let target = safe_join(dest, dest_rel)?;
        hooks
            .before_file(artifact_id, dest_rel)
            .map_err(|e| io_error(&target, e))?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        }
        fs::write(&target, bytes).map_err(|e| io_error(&target, e))?;
        if executable {
            make_executable(&target).map_err(|e| io_error(&target, e))?;
        }
        Ok(())
    };
    if artifact.kind == ArtifactKind::Mcp {
        let (definition, keys) = mcp_definition(source_root, artifact);
        replaced.extend(keys);
        let json = serde_json::to_vec_pretty(&definition)?;
        write(SERVER_FILE, json, false)?;
    }
    for (src_rel, dest_rel) in plan(artifact)? {
        if let Some(reason) = refusal(source_root, &src_rel, max)? {
            skipped.push(SkippedFile {
                artifact_id: artifact_id.to_owned(),
                file: src_rel,
                reason,
            });
            continue;
        }
        let source = safe_join(source_root, &src_rel)?;
        let bytes = fs::read(&source).map_err(|e| io_error(&source, e))?;
        let (bytes, keys) = rewrite(artifact, &src_rel, bytes)?;
        replaced.extend(keys);
        write(&dest_rel, bytes, is_executable(&source))?;
    }
    replaced.sort();
    replaced.dedup();
    Ok(Copied {
        skipped,
        replaced_secret_keys: replaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(kind: ArtifactKind, path: &str, files: &[&str]) -> Artifact {
        Artifact {
            kind,
            name: "n".into(),
            description: None,
            path: path.into(),
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            metadata: Map::new(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        }
    }

    fn dests(a: &Artifact) -> Vec<String> {
        plan(a).expect("plan").into_iter().map(|(_, d)| d).collect()
    }

    #[test]
    fn folder_artifacts_are_stored_relative_to_their_folder() {
        let a = artifact(
            ArtifactKind::Skill,
            "skills/x",
            &["skills/x/SKILL.md", "skills/x/scripts/run.sh"],
        );
        assert_eq!(dests(&a), ["SKILL.md", "scripts/run.sh"]);
    }

    #[test]
    fn file_artifacts_are_stored_relative_to_their_parent() {
        let a = artifact(ArtifactKind::Agent, "agents/a.md", &["agents/a.md"]);
        assert_eq!(dests(&a), ["a.md"]);
        let root = artifact(
            ArtifactKind::ClaudeMd,
            "CLAUDE.md",
            &["CLAUDE.md", "docs/b.md"],
        );
        assert_eq!(dests(&root), ["CLAUDE.md", "docs/b.md"]);
    }

    #[test]
    fn files_outside_the_base_fall_back_to_root_relative() {
        let a = artifact(
            ArtifactKind::Settings,
            ".claude/settings.json",
            &[".claude/settings.json", "scripts/hook.sh"],
        );
        assert_eq!(dests(&a), [".claude/settings.json", "scripts/hook.sh"]);
    }

    #[test]
    fn mcp_server_drops_the_shared_file_and_keeps_helper_scripts() {
        let a = artifact(
            ArtifactKind::Mcp,
            ".mcp.json#s",
            &[".mcp.json", "bin/helper.sh"],
        );
        assert_eq!(dests(&a), ["bin/helper.sh"]);
    }

    #[test]
    fn unsafe_paths_are_refused() {
        for bad in ["../x", "/etc/passwd", "a/../../x", "./a", "", "a//.."] {
            let a = artifact(ArtifactKind::Agent, "agents/a.md", &[bad]);
            assert!(
                matches!(plan(&a), Err(Error::UnsafePath { .. })),
                "{bad:?} should be refused"
            );
        }
        assert!(safe_join(Path::new("/d"), "../x").is_err());
        assert_eq!(
            safe_join(Path::new("/d"), "a/b").expect("join"),
            Path::new("/d/a/b")
        );
    }

    #[test]
    fn a_reserved_server_json_in_an_mcp_artifact_is_refused() {
        let a = artifact(
            ArtifactKind::Mcp,
            ".mcp.json#s",
            &[".mcp.json", "server.json"],
        );
        assert!(plan(&a).is_err());
    }
}
