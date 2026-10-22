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
    /// Larger than the scan's per-file limit.
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
fn plan(artifact: &Artifact) -> Result<Vec<(String, String)>> {
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

/// The definition an MCP server is stored with, as `{ "<name>": { ... } }`.
///
/// This is the hook where literal secrets are swapped for `${KEY}` placeholders: today it copies
/// the scan's metadata as is, in which the scan has already replaced literal secrets with its
/// `<redacted>` marker.
fn mcp_definition(artifact: &Artifact) -> Value {
    let mut outer = Map::new();
    outer.insert(
        artifact.name.clone(),
        Value::Object(artifact.metadata.clone()),
    );
    Value::Object(outer)
}

/// Rewrites a file's bytes on the way into the store. Today every file is copied byte for byte;
/// this is where literal secrets in Settings JSON are replaced by placeholders.
fn rewrite(_artifact: &Artifact, _dest_rel: &str, bytes: Vec<u8>) -> Vec<u8> {
    bytes
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

/// Copies `artifact` from `source_root` into the existing, empty folder `dest`. Returns the
/// files that were left out.
pub(crate) fn copy_artifact(
    source_root: &Path,
    artifact: &Artifact,
    artifact_id: &str,
    dest: &Path,
    hooks: &dyn ImportHooks,
) -> Result<Vec<SkippedFile>> {
    let max = ScanOptions::default().max_file_bytes as u64;
    let mut skipped = Vec::new();
    let write = |dest_rel: &str, bytes: Vec<u8>| -> Result<()> {
        let target = safe_join(dest, dest_rel)?;
        hooks
            .before_file(artifact_id, dest_rel)
            .map_err(|e| io_error(&target, e))?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        }
        fs::write(&target, bytes).map_err(|e| io_error(&target, e))
    };
    if artifact.kind == ArtifactKind::Mcp {
        let json = serde_json::to_vec_pretty(&mcp_definition(artifact))?;
        write(SERVER_FILE, json)?;
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
        write(&dest_rel, rewrite(artifact, &dest_rel, bytes))?;
    }
    Ok(skipped)
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
