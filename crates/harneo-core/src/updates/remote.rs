//! Looking at a source as it is now, for the update check: the remote's commit (`ls-remote`),
//! a shallow clone when that commit moved, or an in-place read of a local folder. Every git
//! call goes through `library::git`; a source that cannot be read becomes a [`Failure`], never
//! an error that would stop the other sources.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::artifact::{AgentAdapter, Artifact, ClaudeAdapter, ScanOptions};
use crate::error::Error;
use crate::library::git::{self, Checkout, GitConfig};
use crate::library::{git_subfolder, Source, SourceRecord};

/// Clones the update check made, kept for the session by source id so *Update* can copy from
/// them. A newer clone of the same source replaces (and deletes) the old one, and dropping the
/// library deletes them all; a crash leaves `.clone-*` folders the library sweeps on open.
#[derive(Debug, Default)]
pub(crate) struct Session {
    kept: Mutex<HashMap<String, Arc<Checkout>>>,
}

impl Session {
    /// Keeps `checkout` for `source_id`, replacing an older clone: that one is deleted once
    /// nobody who got it from [`Self::get`] still uses it.
    fn keep(&self, source_id: &str, checkout: Checkout) -> Arc<Checkout> {
        let checkout = Arc::new(checkout);
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(source_id.to_owned(), Arc::clone(&checkout));
        checkout
    }

    fn get(&self, source_id: &str) -> Option<Arc<Checkout>> {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(source_id)
            .cloned()
    }
}

/// A source's files on disk, for copying from. Holds the clone alive while it is used.
#[derive(Debug)]
pub(crate) struct Located {
    /// The folder the source's artifacts are scanned from.
    pub root: PathBuf,
    /// The commit of a git source's clone.
    pub commit: Option<String>,
    _clone: Option<Arc<Checkout>>,
}

/// Finds `source`'s files for an update: a local folder in place; a git source in the clone the
/// check kept, else in a new shallow clone (which is then kept for the next artifact).
pub(crate) fn locate(
    config: &GitConfig,
    session: &Session,
    library_root: &Path,
    source: &SourceRecord,
) -> Result<Located, Failure> {
    check_source(source)?;
    if source.kind == "local" {
        let root = PathBuf::from(&source.location);
        if !root.is_dir() {
            return Err(Failure::Missing);
        }
        return Ok(Located {
            root,
            commit: None,
            _clone: None,
        });
    }
    let checkout = match session.get(&source.id) {
        Some(kept) => kept,
        None => {
            let fresh = git::clone(
                config,
                library_root,
                &source.location,
                source.branch.as_deref(),
            )
            .map_err(unreachable)?;
            session.keep(&source.id, fresh)
        }
    };
    let root = checkout_root(&checkout, source)?;
    Ok(Located {
        root,
        commit: Some(checkout.commit.clone()),
        _clone: Some(checkout),
    })
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
    check_source(source)?;
    if source.kind == "local" {
        let root = Path::new(&source.location);
        if !root.is_dir() {
            return Err(Failure::Missing);
        }
        return Ok(Upstream::Scanned {
            commit: None,
            artifacts: scan(root, source)?,
        });
    }
    let url = source.location.as_str();
    let branch = source.branch.as_deref();
    let remote = git::ls_remote(config, url, branch).map_err(unreachable)?;
    if known == Some(remote.as_str()) {
        return Ok(Upstream::Unchanged);
    }
    let checkout = git::clone(config, library_root, url, branch).map_err(unreachable)?;
    let root = checkout_root(&checkout, source)?;
    let artifacts = scan(&root, source)?;
    let commit = checkout.commit.clone();
    session.keep(&source.id, checkout);
    Ok(Upstream::Scanned {
        commit: Some(commit),
        artifacts,
    })
}

/// Refuses a stored source that git or a scan should never see: a local one that is not an
/// absolute path (an artifact that came in a profile file has no source linked), a git one whose
/// URL or branch does not parse as the user could have typed it. Rows can come from a file
/// someone else wrote, so this runs before any git call or scan.
fn check_source(source: &SourceRecord) -> Result<(), Failure> {
    let refuse = |why: &str| Err(Failure::Unreachable(why.to_owned()));
    match source.kind.as_str() {
        "local" if Path::new(&source.location).is_absolute() => Ok(()),
        "local" => refuse("it came in a profile file and has no source linked; link one to update"),
        "git" => {
            let branch = source.branch.as_deref();
            let plain = |b: &str| {
                !b.is_empty()
                    && !b.starts_with('-')
                    && !b.contains("..")
                    && !b.chars().any(|c| c.is_whitespace() || c.is_control())
            };
            let parsed = Source::parse_git(&source.location, branch, source.subfolder.as_deref());
            if parsed.is_ok() && branch.is_none_or(plain) {
                Ok(())
            } else {
                refuse("its recorded source is not a valid git address")
            }
        }
        _ => refuse("its recorded source is not one Harneo can read"),
    }
}

/// The source's folder inside its clone, which must stay inside it.
fn checkout_root(checkout: &Checkout, source: &SourceRecord) -> Result<PathBuf, Failure> {
    let relative = git_subfolder(source.subfolder.as_deref()).map_err(unreachable)?;
    let root = checkout.path().join(relative);
    let inside = root
        .canonicalize()
        .ok()
        .zip(checkout.path().canonicalize().ok())
        .is_some_and(|(root, base)| root.is_dir() && root.starts_with(base));
    if inside {
        Ok(root)
    } else {
        Err(Failure::Missing)
    }
}

/// Scans the source's folder as its import did: a git source passes its GitHub repo, so a
/// marketplace entry naming that repo merges with its plugin folder the same way.
pub(crate) fn scan(root: &Path, source: &SourceRecord) -> Result<Vec<Artifact>, Failure> {
    let options = ScanOptions {
        repo: (source.kind == "git")
            .then(|| crate::artifact::github_repo(&source.location))
            .flatten(),
        ..ScanOptions::default()
    };
    ClaudeAdapter
        .scan(root, &options)
        .map(|report| report.artifacts)
        .map_err(unreachable)
}

fn unreachable(error: Error) -> Failure {
    Failure::Unreachable(error.to_string())
}
