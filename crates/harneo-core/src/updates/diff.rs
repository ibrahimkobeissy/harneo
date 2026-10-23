//! Per-file differences between the library copy and what an update would store, as line hunks.
//! Both sides are in stored form (secrets already replaced, MCP as `server.json`), so the diff
//! never shows a secret and lists exactly what *Update* would change.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use ts_rs::TS;

use crate::error::{Error, Result};

/// Lines of unchanged context kept around each change.
const CONTEXT: usize = 3;

/// How a file differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DiffStatus {
    /// New upstream.
    Added,
    /// Gone upstream.
    Removed,
    /// In both, with different content.
    Changed,
}

/// What a line is in a hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

/// One line of a hunk, without its line ending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
}

/// A run of changes with its context. Line numbers start at 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Hunk {
    /// First line of the hunk in the library copy.
    pub old_start: u32,
    /// First line of the hunk upstream.
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

/// One file that differs. `hunks` is empty for a file that is not text (it only says it changed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct FileDiff {
    /// Relative to the artifact's stored folder.
    pub path: String,
    pub status: DiffStatus,
    pub hunks: Vec<Hunk>,
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

fn read(dir: &Path, rel: &str) -> Result<Vec<u8>> {
    let path = dir.join(rel);
    fs::read(&path).map_err(|e| io_error(&path, e))
}

fn number(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The hunks turning `old` into `new`; none when either is not UTF-8 text.
pub(crate) fn hunks(old: &[u8], new: &[u8]) -> Vec<Hunk> {
    let (Ok(old), Ok(new)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        return Vec::new();
    };
    let diff = TextDiff::from_lines(old, new);
    diff.grouped_ops(CONTEXT)
        .iter()
        .map(|group| {
            let first = group.first();
            let mut lines = Vec::new();
            for op in group {
                for change in diff.iter_changes(op) {
                    let kind = match change.tag() {
                        ChangeTag::Equal => LineKind::Context,
                        ChangeTag::Delete => LineKind::Removed,
                        ChangeTag::Insert => LineKind::Added,
                    };
                    lines.push(DiffLine {
                        kind,
                        text: change.value().trim_end_matches(['\n', '\r']).to_owned(),
                    });
                }
            }
            Hunk {
                old_start: number(first.map_or(0, |op| op.old_range().start) + 1),
                new_start: number(first.map_or(0, |op| op.new_range().start) + 1),
                lines,
            }
        })
        .collect()
}

/// The files that differ between the library copy `current` and the staged upstream copy
/// `upstream`, by path.
pub(super) fn diff_dirs(current: &Path, upstream: &Path) -> Result<Vec<FileDiff>> {
    let old = crate::library::stored_files(current)?;
    let new = crate::library::stored_files(upstream)?;
    let mut paths: Vec<&String> = old.iter().chain(new.iter()).collect();
    paths.sort();
    paths.dedup();
    let mut out = Vec::new();
    for path in paths {
        let (was, is) = (old.contains(path), new.contains(path));
        let diff = match (was, is) {
            (true, true) => {
                let (a, b) = (read(current, path)?, read(upstream, path)?);
                if a == b {
                    continue;
                }
                FileDiff {
                    path: path.clone(),
                    status: DiffStatus::Changed,
                    hunks: hunks(&a, &b),
                }
            }
            (false, true) => FileDiff {
                path: path.clone(),
                status: DiffStatus::Added,
                hunks: hunks(b"", &read(upstream, path)?),
            },
            _ => FileDiff {
                path: path.clone(),
                status: DiffStatus::Removed,
                hunks: hunks(&read(current, path)?, b""),
            },
        };
        out.push(diff);
    }
    Ok(out)
}
