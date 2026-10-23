//! Looking at a source as it is now, for the update check: the remote's commit (`ls-remote`),
//! a shallow clone when that commit moved, or an in-place read of a local folder. Every git
//! call goes through `library::git`; a source that cannot be read becomes a [`Failure`], never
//! an error that would stop the other sources.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::artifact::{AgentAdapter, Artifact, ClaudeAdapter, ScanOptions};
use crate::error::Error;
use crate::library::git::{self, Checkout, GitConfig};
use crate::library::{git_subfolder, SourceRecord};

/// Clones the update check made, kept for the session by source id so *Update* can copy from
/// them. A newer clone of the same source replaces (and deletes) the old one, and dropping the
/// library deletes them all; a crash leaves `.clone-*` folders the library sweeps on open.
#[derive(Debug, Default)]
pub(crate) struct Session {
    kept: Mutex<HashMap<String, Checkout>>,
}

impl Session {
    fn keep(&self, source_id: &str, checkout: Checkout) {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(source_id.to_owned(), checkout);
    }
}

/// Why a source could not be looked at.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Offline, no access, unknown branch, git missing or too slow, an unreadable scan.
    Unreachable(String),
    /// The folder (or the repository's subfolder) is gone.
    Missing,
}

/// What the source holds now.
#[derive(Debug)]
pub(crate) enum Upstream {
    /// A git remote still at the commit recorded last time: nothing was downloaded.
    Unchanged,
    /// A fresh scan of the source's root. `commit` is the cloned commit of a git source.
    Scanned {
        commit: Option<String>,
        artifacts: Vec<Artifact>,
    },
}

/// Looks at `source`, whose last seen remote commit is `known` (git only).
pub(crate) fn fetch(
    config: &GitConfig,
    session: &Session,
    library_root: &Path,
    source: &SourceRecord,
    known: Option<&str>,
) -> Result<Upstream, Failure> {
    if source.kind == "local" {
        let root = Path::new(&source.location);
        if !root.is_dir() {
            return Err(Failure::Missing);
        }
        return Ok(Upstream::Scanned {
            commit: None,
            artifacts: scan(root)?,
        });
    }
    let url = source.location.as_str();
    let branch = source.branch.as_deref();
    let remote = git::ls_remote(config, url, branch).map_err(unreachable)?;
    if known == Some(remote.as_str()) {
        return Ok(Upstream::Unchanged);
    }
    let checkout = git::clone(config, library_root, url, branch).map_err(unreachable)?;
    let relative = git_subfolder(source.subfolder.as_deref()).map_err(unreachable)?;
    let root = checkout.path().join(relative);
    let inside = root
        .canonicalize()
        .ok()
        .zip(checkout.path().canonicalize().ok())
        .is_some_and(|(root, base)| root.is_dir() && root.starts_with(base));
    if !inside {
        return Err(Failure::Missing);
    }
    let artifacts = scan(&root)?;
    let commit = checkout.commit.clone();
    session.keep(&source.id, checkout);
    Ok(Upstream::Scanned {
        commit: Some(commit),
        artifacts,
    })
}

fn scan(root: &Path) -> Result<Vec<Artifact>, Failure> {
    ClaudeAdapter
        .scan(root, &ScanOptions::default())
        .map(|report| report.artifacts)
        .map_err(unreachable)
}

fn unreachable(error: Error) -> Failure {
    Failure::Unreachable(error.to_string())
}
