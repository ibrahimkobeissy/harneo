//! The update check in three steps, so the network and clone work never runs under the
//! library's lock: [`Library::prepare_check`] reads what is needed (under the lock),
//! [`CheckJob::run`] looks at the sources (no lock, slow), [`Library::finish_check`] writes
//! the findings (under the lock, short). [`Library::check_updates`] chains the three.

use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::params;

use super::remote::{self, Failure, Session, Upstream};
use crate::library::layout_claude::StoredLayout;
use super::*;
use crate::library::git::GitConfig;

/// What the check needs, read from the library. Owns everything, so it can leave the lock.
#[derive(Debug)]
pub struct CheckJob {
    git: GitConfig,
    session: Arc<Session>,
    library_root: PathBuf,
    sources: Vec<SourceWork>,
    only: Option<String>,
}

#[derive(Debug)]
struct SourceWork {
    source: SourceRecord,
    remote_commit: Option<String>,
    stored: Vec<Stored>,
    /// What Claude placed and ignored when the source was imported, replayed on each scan.
    layout: StoredLayout,
}

/// What looking at the sources found, ready to be written by [`Library::finish_check`].
#[derive(Debug)]
pub struct CheckOutcome {
    sources: Vec<SourceOutcome>,
}

#[derive(Debug)]
struct SourceOutcome {
    id: String,
    found: Found,
}

#[derive(Debug)]
enum Found {
    Missing,
    Unreachable(String),
    Unchanged,
    Scanned {
        commit: Option<String>,
        judgements: Vec<Judgement>,
        new_upstream: u32,
        unplaced: Vec<String>,
        unplaced_overflow: u32,
        /// The source's seen commit moves only when every artifact of it was judged; after a
        /// one-artifact check the others must still be looked at next time.
        advance: bool,
    },
}

/// What was decided for one artifact, with the digest it was judged against.
#[derive(Debug)]
struct Judgement {
    id: String,
    /// The library's digest when judged; if it moved since (an update), the judgement is stale.
    judged_digest: Option<String>,
    what: Judged,
}

#[derive(Debug)]
enum Judged {
    Current,
    Outdated {
        upstream_digest: Option<String>,
        from: Option<String>,
        to: Option<String>,
    },
    Removed {
        from: Option<String>,
    },
    LayoutChanged {
        from: Option<String>,
    },
}

/// The commit a stored artifact was last taken from: its own after an update, else the
/// source's at import.
pub(super) fn imported_commit<'a>(
    artifact: &'a Artifact,
    source: &'a SourceRecord,
) -> Option<&'a str> {
    artifact
        .metadata
        .get(super::IMPORTED_COMMIT)
        .and_then(serde_json::Value::as_str)
        .or(source.commit_sha.as_deref())
}

impl CheckJob {
    /// Looks at each source (no lock needed). One that cannot be read is reported in its own
    /// entry and never stops the others. A git source whose remote commit equals the one seen
    /// last time is not cloned. A clone made here is kept for the session.
    #[must_use]
    pub fn run(self) -> CheckOutcome {
        let mut sources = Vec::new();
        for work in &self.sources {
            let id = work.source.id.clone();
            sources.push(SourceOutcome {
                id,
                found: self.look(work),
            });
        }
        CheckOutcome { sources }
    }

    fn look(&self, work: &SourceWork) -> Found {
        let source = &work.source;
        // The commit last seen on the remote; before any check, the one imported from.
        let known = work
            .remote_commit
            .as_deref()
            .or(source.commit_sha.as_deref());
        let upstream = match remote::fetch(
            &self.git,
            &self.session,
            &self.library_root,
            source,
            known,
            &work.layout,
        ) {
            Ok(upstream) => upstream,
            Err(Failure::Missing) => return Found::Missing,
            Err(Failure::Unreachable(reason)) => return Found::Unreachable(reason),
        };
        let (commit, scanned) = match upstream {
            Upstream::Unchanged => return Found::Unchanged,
            Upstream::Scanned { commit, scanned } => (commit, scanned),
        };
        let mut judgements = Vec::new();
        for item in work
            .stored
            .iter()
            .filter(|s| self.only.as_deref().is_none_or(|id| s.id == id))
        {
            let found = scanned.artifacts.iter().find(|a| a.path == item.source_path);
            let from_commit = imported_commit(&item.artifact, source);
            let what = match classify(item, found) {
                Verdict::Current => Judged::Current,
                Verdict::Outdated(upstream) => {
                    let (from, to) = versions(
                        &item.artifact,
                        Some(&upstream),
                        from_commit,
                        commit.as_deref(),
                    );
                    Judged::Outdated {
                        upstream_digest: upstream.digest.clone(),
                        from,
                        to,
                    }
                }
                Verdict::Removed => Judged::Removed {
                    from: versions(&item.artifact, None, None, None).0,
                },
                Verdict::LayoutChanged => Judged::LayoutChanged {
                    from: versions(&item.artifact, None, None, None).0,
                },
            };
            judgements.push(Judgement {
                id: item.id.clone(),
                judged_digest: item.digest.clone(),
                what,
            });
        }
        let known_paths: Vec<&str> = work.stored.iter().map(|s| s.source_path.as_str()).collect();
        let new_upstream = scanned
            .artifacts
            .iter()
            .filter(|a| !known_paths.contains(&a.path.as_str()))
            .count();
        Found::Scanned {
            commit,
            judgements,
            new_upstream: u32::try_from(new_upstream).unwrap_or(u32::MAX),
            unplaced: scanned.unplaced,
            unplaced_overflow: scanned.unplaced_overflow,
            advance: self.only.is_none(),
        }
    }
}

impl Library {
    /// Looks at the sources in `scope` and records which artifacts are outdated or removed
    /// upstream (see the module doc: this holds whatever lock the caller has for the whole
    /// check; the app calls the three steps separately).
    pub fn check_updates(&self, scope: &CheckScope) -> Result<CheckReport> {
        self.finish_check(self.prepare_check(scope)?.run())
    }

    /// Step one: reads the sources in `scope` and their stored artifacts.
    pub fn prepare_check(&self, scope: &CheckScope) -> Result<CheckJob> {
        let conn = &self.store.conn;
        let (source_filter, only) = match scope {
            CheckScope::All => (None, None),
            CheckScope::Source { id } => (Some(id.clone()), None),
            CheckScope::Artifact { id } => {
                let source: Option<String> = conn
                    .query_row(
                        "SELECT source_id FROM artifacts WHERE id = ?1",
                        params![id],
                        |r| r.get(0),
                    )
                    .optional()?;
                let source = source.ok_or_else(|| Error::NotInLibrary { id: id.clone() })?;
                (Some(source), Some(id.clone()))
            }
        };
        let mut sources = Vec::new();
        for (source, remote_commit) in source_rows(conn, source_filter.as_deref())? {
            let stored = stored_of_source(conn, &source.id)?;
            self.backfill_copy_digests(&stored)?;
            let layout = records::stored_layout(conn, &source.id)?;
            sources.push(SourceWork {
                source,
                remote_commit,
                stored,
                layout,
            });
        }
        Ok(CheckJob {
            git: self.git.clone(),
            session: Arc::clone(&self.session),
            library_root: self.store.root().to_path_buf(),
            sources,
            only,
        })
    }

    /// Step three: stores what the check found and reports it. A finding about an artifact
    /// that was updated or removed since step one is dropped.
    pub fn finish_check(&self, outcome: CheckOutcome) -> Result<CheckReport> {
        let mut report = CheckReport::default();
        for source in outcome.sources {
            let (unplaced, unplaced_overflow) = match &source.found {
                Found::Scanned {
                    unplaced,
                    unplaced_overflow,
                    ..
                } => (unplaced.clone(), *unplaced_overflow),
                _ => (Vec::new(), 0),
            };
            let result = self.store_source(&source.id, source.found)?;
            report.sources.push(SourceCheck {
                id: source.id,
                result,
                unplaced,
                unplaced_overflow,
            });
        }
        Ok(report)
    }

    fn store_source(&self, source_id: &str, found: Found) -> Result<SourceResult> {
        let conn = &self.store.conn;
        let now = records::now();
        let (commit, judgements, new_upstream, advance) = match found {
            Found::Missing => return Ok(SourceResult::Missing),
            Found::Unreachable(reason) => return Ok(SourceResult::Unreachable { reason }),
            Found::Unchanged => {
                conn.execute(
                    "UPDATE sources SET checked_at = ?2 WHERE id = ?1",
                    params![source_id, now],
                )?;
                return result_from_table(conn, source_id, 0);
            }
            Found::Scanned {
                commit,
                judgements,
                new_upstream,
                advance,
                ..
            } => (commit, judgements, new_upstream, advance),
        };
        let tx = conn.unchecked_transaction()?;
        for judgement in judgements {
            let current: Option<Option<String>> = tx
                .query_row(
                    "SELECT digest FROM artifacts WHERE id = ?1",
                    params![judgement.id],
                    |r| r.get(0),
                )
                .optional()?;
            // Gone from the library, or updated while the check ran.
            if current.is_none_or(|digest| digest != judgement.judged_digest) {
                continue;
            }
            let id = judgement.id.as_str();
            match judgement.what {
                Judged::Current => {
                    tx.execute("DELETE FROM upstream WHERE artifact_id = ?1", params![id])?;
                }
                Judged::Outdated {
                    upstream_digest,
                    from,
                    to,
                } => {
                    let existing = existing_row(&tx, id)?;
                    let kept = existing.as_ref().is_some_and(|e| {
                        e.state == UpstreamState::Skipped && e.upstream_digest == upstream_digest
                    });
                    if !kept {
                        write_row(
                            &tx,
                            &NewRow {
                                id,
                                state: UpstreamState::Outdated,
                                upstream_digest,
                                upstream_commit: &commit,
                                from_version: from,
                                to_version: to,
                                checked_at: now,
                            },
                        )?;
                    }
                }
                Judged::LayoutChanged { from } => write_row(
                    &tx,
                    &NewRow {
                        id,
                        state: UpstreamState::LayoutChanged,
                        upstream_digest: None,
                        upstream_commit: &commit,
                        from_version: from,
                        to_version: None,
                        checked_at: now,
                    },
                )?,
                Judged::Removed { from } => write_row(
                    &tx,
                    &NewRow {
                        id,
                        state: UpstreamState::Removed,
                        upstream_digest: None,
                        upstream_commit: &commit,
                        from_version: from,
                        to_version: None,
                        checked_at: now,
                    },
                )?,
            }
        }
        let seen = if advance { commit } else { None };
        tx.execute(
            "UPDATE sources SET checked_at = ?2, remote_commit = COALESCE(?3, remote_commit)
             WHERE id = ?1",
            params![source_id, now, seen],
        )?;
        tx.commit()?;
        result_from_table(conn, source_id, new_upstream)
    }
}
