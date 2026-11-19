//! Collapses identical copies of one artifact (same kind, name and content) in a preview.
//!
//! The scan's `artifact.digest` also hashes each file's path from the scan root, so two copies
//! in different folders never share it. It only tells us the scan could read the artifact
//! (`None` never collapses); copies are compared by [`content_key`], which hashes paths
//! relative to the artifact's own folder.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::Candidate;
use crate::artifact::{Artifact, ArtifactKind};

const DUPLICATE_NAME: &str = "scan.duplicate-name";

/// One candidate per (kind, name, digest): the one with the smallest path is kept and the
/// others' paths, sorted, go in its `copies`. Candidates without a digest never collapse.
/// The kept candidate loses `scan.duplicate-name` once no differing sibling remains.
pub(super) fn collapse_copies(root: &Path, candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut sizes: HashMap<(ArtifactKind, &str), usize> = HashMap::new();
    for c in &candidates {
        *sizes
            .entry((c.artifact.kind, c.artifact.name.as_str()))
            .or_default() += 1;
    }
    let keys: Vec<Option<String>> = candidates
        .iter()
        .map(|c| {
            let shared = sizes[&(c.artifact.kind, c.artifact.name.as_str())] > 1;
            (shared && c.artifact.digest.is_some()).then(|| content_key(root, &c.artifact))
        })
        .collect();
    let mut keeper: HashMap<(ArtifactKind, String, String), usize> = HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let Some(digest) = &keys[i] else {
            continue;
        };
        let key = (c.artifact.kind, c.artifact.name.clone(), digest.clone());
        match keeper.get(&key) {
            Some(&k) if candidates[k].artifact.path <= c.artifact.path => {}
            _ => {
                keeper.insert(key, i);
            }
        }
    }
    let mut copies: HashMap<usize, Vec<String>> = HashMap::new();
    let mut kept: Vec<Candidate> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    let mut dropped: Vec<(usize, String)> = Vec::new();
    for (i, c) in candidates.into_iter().enumerate() {
        let key = keys[i]
            .as_ref()
            .map(|d| (c.artifact.kind, c.artifact.name.clone(), d.clone()));
        match key.and_then(|k| keeper.get(&k).copied()) {
            Some(k) if k != i => dropped.push((k, c.artifact.path)),
            _ => {
                slot.insert(i, kept.len());
                kept.push(c);
            }
        }
    }
    for (k, path) in dropped {
        copies.entry(slot[&k]).or_default().push(path);
    }
    for (s, mut paths) in copies {
        paths.sort();
        kept[s].copies = paths;
    }
    let mut remaining: HashMap<(ArtifactKind, String), usize> = HashMap::new();
    for c in &kept {
        *remaining
            .entry((c.artifact.kind, c.artifact.name.clone()))
            .or_default() += 1;
    }
    for c in &mut kept {
        if remaining[&(c.artifact.kind, c.artifact.name.clone())] == 1 {
            c.artifact.diagnostics.retain(|d| d.code != DUPLICATE_NAME);
        }
    }
    kept
}

/// The artifacts of `fresh` that are identical copies (same kind, name and content, as in
/// [`collapse_copies`]) of one in `held`, or of another in `fresh` with a smaller path. The
/// update check uses it to count each new artifact once, the way the import preview folds them.
pub(crate) fn fold_fresh<'a>(
    root: &Path,
    held: &[&Artifact],
    fresh: &[&'a Artifact],
) -> Vec<&'a Artifact> {
    let key = |a: &Artifact| {
        a.digest
            .is_some()
            .then(|| (a.kind, a.name.clone(), content_key(root, a)))
    };
    let held_keys: HashSet<_> = held.iter().filter_map(|a| key(a)).collect();
    let keyed: Vec<_> = fresh.iter().map(|a| (*a, key(a))).collect();
    keyed
        .iter()
        .filter(|(a, k)| {
            k.as_ref().is_some_and(|k| {
                held_keys.contains(k)
                    || keyed
                        .iter()
                        .any(|(b, o)| o.as_ref() == Some(k) && b.path < a.path)
            })
        })
        .map(|(a, _)| *a)
        .collect()
}

/// SHA-256 over the artifact's files, each named relative to the artifact's own path, so the
/// same content in two folders gives the same key. Unreadable files and symlinks count by name.
fn content_key(root: &Path, artifact: &crate::artifact::Artifact) -> String {
    let mut files: Vec<&String> = artifact.files.iter().collect();
    files.sort();
    files.dedup();
    let mut hasher = Sha256::new();
    for rel in files {
        let name = rel
            .strip_prefix(artifact.path.as_str())
            .map_or(rel.as_str(), |n| n.trim_start_matches('/'));
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        let path = root.join(rel);
        let plain = fs::symlink_metadata(&path).is_ok_and(|m| m.is_file());
        match plain.then(|| fs::read(&path).ok()).flatten() {
            Some(bytes) => {
                hasher.update([1u8]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
            None => hasher.update([0u8]),
        }
    }
    hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
