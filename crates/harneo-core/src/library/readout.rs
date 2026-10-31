//! The main text of an artifact, for bundle detection. A preview's text goes through the same
//! copy as an import (so secrets are `${KEY}` placeholders); a stored copy is read as it is.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use super::copy::{copy_artifact, copy_limit, safe_join, stored_view, NoHooks};
use crate::artifact::{Artifact, ArtifactKind};
use crate::error::Result;

/// The most bytes read from the main file; the caller cuts it to characters.
const READ_BYTES: u64 = 64 * 1024;

/// The artifact as it looks in its stored copy (see `copy::stored_view`).
pub(crate) fn stored_view_of(artifact: &Artifact) -> Result<Artifact> {
    Ok(stored_view(artifact)?.0)
}

/// The main text of a candidate in `root`, copied (with the secret replacement) into a
/// temporary folder first. `None` when it cannot be copied safely or has no text.
pub(crate) fn preview_text(root: &Path, id: &str, artifact: &Artifact) -> Option<String> {
    let staging = tempfile::tempdir().ok()?;
    let dir = staging.path().join(id);
    fs::create_dir(&dir).ok()?;
    copy_artifact(root, artifact, id, &dir, &NoHooks, copy_limit(false)).ok()?;
    stored_text(&dir, artifact)
}

/// The main text of the stored copy in `folder`.
pub(crate) fn stored_text(folder: &Path, artifact: &Artifact) -> Option<String> {
    let (view, _) = stored_view(artifact).ok()?;
    let file = main_file(folder, &view)?;
    let mut bytes = Vec::new();
    File::open(file)
        .ok()?
        .take(READ_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn main_file(folder: &Path, view: &Artifact) -> Option<std::path::PathBuf> {
    if view.kind == ArtifactKind::Mcp {
        return Some(folder.join("server.json")).filter(|p| p.is_file());
    }
    let own = view.path.split('#').next().unwrap_or_default();
    if let Ok(path) = safe_join(folder, own) {
        if path.is_file() {
            return Some(path);
        }
    }
    let base = |f: &str| f.rsplit('/').next().unwrap_or(f).to_lowercase();
    let pick = view
        .files
        .iter()
        .find(|f| base(f) == "skill.md")
        .or_else(|| view.files.iter().find(|f| base(f).ends_with(".md")))
        .or_else(|| view.files.first())?;
    safe_join(folder, pick).ok().filter(|p| p.is_file())
}
