//! Reading one file of a library copy for the in-app viewer, and finding the file to open in
//! the system's default app. Both go through [`Library::artifact_file`], which refuses
//! anything that is not a plain file inside the copy's folder.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::{io_error, Library};
use crate::error::{Error, Result};

/// Files larger than this (1 MB) are not previewed.
pub const MAX_VIEW_BYTES: u64 = 1_000_000;

/// What the viewer can show of one file.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum FileView {
    /// Valid UTF-8 text.
    Text { text: String },
    /// Not valid UTF-8: no preview.
    Binary,
    /// Over [`MAX_VIEW_BYTES`]: no preview.
    #[serde(rename_all = "camelCase")]
    TooLarge { size: f64 },
}

fn unsafe_path(path: &str) -> Error {
    Error::UnsafePath {
        path: path.to_owned(),
    }
}

impl Library {
    /// The real path of `path` (relative, forward slashes) inside the copy of artifact `id`.
    /// Refuses an id or path that climbs out (`..`), is absolute, or reaches a symlink that
    /// leaves the copy, and anything that is not a regular file.
    pub fn artifact_file(&self, id: &str, path: &str) -> Result<PathBuf> {
        let plain = |s: &str| {
            !s.is_empty()
                && Path::new(s)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
        };
        if !plain(id) || id.contains(['/', '\\']) {
            return Err(unsafe_path(id));
        }
        if !plain(path) || path.contains(['\\', '\0']) {
            return Err(unsafe_path(path));
        }
        let folder = self.store.artifacts_dir().join(id);
        let root = fs::canonicalize(&folder).map_err(|e| io_error(&folder, e))?;
        let wanted = root.join(path);
        let real = fs::canonicalize(&wanted).map_err(|e| io_error(&wanted, e))?;
        if !real.starts_with(&root) || !real.is_file() {
            return Err(unsafe_path(path));
        }
        Ok(real)
    }

    /// One file of an artifact's library copy, as the viewer shows it.
    pub fn read_artifact_file(&self, id: &str, path: &str) -> Result<FileView> {
        let real = self.artifact_file(id, path)?;
        let file = fs::File::open(&real).map_err(|e| io_error(&real, e))?;
        let size = file.metadata().map_err(|e| io_error(&real, e))?.len();
        if size > MAX_VIEW_BYTES {
            return Ok(FileView::TooLarge { size: size as f64 });
        }
        let mut bytes = Vec::new();
        file.take(MAX_VIEW_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io_error(&real, e))?;
        if bytes.len() as u64 > MAX_VIEW_BYTES {
            return Ok(FileView::TooLarge {
                size: bytes.len() as f64,
            });
        }
        Ok(match String::from_utf8(bytes) {
            Ok(text) => FileView::Text { text },
            Err(_) => FileView::Binary,
        })
    }
}
