//! Renames at the source (decision 10 of `spec/updates.md`). Renaming a skill in its repo shows
//! up as one artifact removed and one new, so the check pairs them: a removed artifact and a
//! new one of the same kind whose contents are the same once the name is ignored (the folder or
//! file name and the `name:` frontmatter line). Only a one-to-one match counts; anything
//! ambiguous, or a rename with other changes, stays removed plus new.
//!
//! Both sides are compared as stored: the library copy of the removed one against what a copy of
//! the new one would store (secrets replaced the same way), so the same rules decide.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::Stored;
use crate::artifact::Artifact;
use crate::error::Result;
use crate::library::{copy_artifact, copy_limit, stored_view, NoHooks};

/// A removed artifact and the new one it was renamed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pair {
    pub(super) removed_id: String,
    /// The new artifact's path and name in the source.
    pub(super) path: String,
    pub(super) name: String,
}

/// Pairs each of `removed` with the one artifact of `fresh` it was renamed to. `root` is the
/// source's folder; the removed artifacts' copies are read from the library under
/// `library_root`.
pub(super) fn find(
    library_root: &Path,
    root: &Path,
    large_files: bool,
    removed: &[&Stored],
    fresh: &[&Artifact],
) -> Vec<Pair> {
    let mut by_key: HashMap<(String, String), (Vec<usize>, Vec<usize>)> = HashMap::new();
    for (i, item) in removed.iter().enumerate() {
        let folder = library_root.join("artifacts").join(&item.id);
        if let Ok(digest) = stored_digest(&folder, &item.artifact) {
            by_key
                .entry((format!("{:?}", item.artifact.kind), digest))
                .or_default()
                .0
                .push(i);
        }
    }
    for (i, artifact) in fresh.iter().enumerate() {
        let kind = format!("{:?}", artifact.kind);
        if !removed
            .iter()
            .any(|r| format!("{:?}", r.artifact.kind) == kind)
        {
            continue;
        }
        if let Ok(digest) = candidate_digest(library_root, root, large_files, artifact) {
            if let Some(entry) = by_key.get_mut(&(kind, digest)) {
                entry.1.push(i);
            }
        }
    }
    let mut pairs: Vec<Pair> = by_key
        .into_values()
        .filter_map(|(old, new)| match (old.as_slice(), new.as_slice()) {
            ([old], [new]) => Some(Pair {
                removed_id: removed[*old].id.clone(),
                path: fresh[*new].path.clone(),
                name: fresh[*new].name.clone(),
            }),
            _ => None,
        })
        .collect();
    pairs.sort_by(|a, b| a.removed_id.cmp(&b.removed_id));
    pairs
}

/// The name-blind digest of what copying `artifact` from `root` would store.
fn candidate_digest(
    library_root: &Path,
    root: &Path,
    large_files: bool,
    artifact: &Artifact,
) -> Result<String> {
    let staging = tempfile::Builder::new()
        .prefix(".update-")
        .tempdir_in(library_root)
        .map_err(|e| io_error(library_root, e))?;
    copy_artifact(
        root,
        artifact,
        "rename",
        staging.path(),
        &NoHooks,
        copy_limit(large_files),
    )?;
    stored_digest(staging.path(), artifact)
}

/// SHA-256 over the stored files below `folder`: each path (the artifact's own file counts as
/// one nameless path) and content, with the `name:` line of a markdown frontmatter left out.
/// `artifact` is as scanned in its source, to find which stored file is its own.
fn stored_digest(folder: &Path, artifact: &Artifact) -> Result<String> {
    let own = stored_view(artifact)?.0.path;
    let mut files = Vec::new();
    walk(folder, "", &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for (rel, path) in files {
        let bytes = fs::read(&path).map_err(|e| io_error(&path, e))?;
        let bytes = without_name(bytes);
        hasher.update(if rel == own { "\0own" } else { &rel }.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        }))
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|e| io_error(dir, e))? {
        let entry = entry.map_err(|e| io_error(dir, e))?;
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type().map_err(|e| io_error(dir, e))?;
        if kind.is_dir() {
            walk(&entry.path(), &format!("{name}/"), out)?;
        } else if kind.is_file() {
            out.push((name, entry.path()));
        }
    }
    Ok(())
}

/// `bytes` without the `name:` line of its leading frontmatter block, when it has one.
fn without_name(bytes: Vec<u8>) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return bytes;
    };
    let mut kept = String::from("---\n");
    let mut lines = rest.split_inclusive('\n');
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            kept.push_str(line);
            closed = true;
            break;
        }
        if !line.trim_start().starts_with("name:") {
            kept.push_str(line);
        }
    }
    if !closed {
        return bytes;
    }
    kept.extend(lines);
    kept.into_bytes()
}

fn io_error(path: &Path, source: std::io::Error) -> crate::error::Error {
    crate::error::Error::Io {
        path: path.display().to_string(),
        source,
    }
}
