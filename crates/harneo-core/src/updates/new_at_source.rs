//! *Update all* imports what is new at a source (decision 8 of `spec/updates.md`): every
//! artifact the source holds at a path the library does not, except the paths the user removed
//! (tombstones, `removed_paths`). It goes through the same import as the Import flow, read with
//! the layout stored for the source; Claude is never called. What that layout cannot place is
//! reported as *not placed*, never guessed.
//!
//! Like an update, it runs in steps so a clone is never made under the library's lock:
//! [`ImportJob::run`] finds the source's files (no lock), [`Library::finish_import`] scans and
//! imports (under the lock, local files only).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use super::apply::{unavailable, UpdateOutcome, UpdateReport};
use super::remote::{self, Located, Session};
use super::{source_rows, IMPORTED_COMMIT};
use crate::artifact::ScanOptions;
use crate::error::{Error, Result};
use crate::library::git::GitConfig;
use crate::library::records::{self, SourceRecord};
use crate::library::{ids, Candidate, Library, NoHooks, Placed, Preview, Source};

/// The new paths of the artifacts of `source_id` the last check found renamed.
fn renamed_paths(conn: &rusqlite::Connection, source_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT u.renamed_path FROM upstream u JOIN artifacts a ON a.id = u.artifact_id
         WHERE a.source_id = ?1 AND u.state = 'renamed' AND u.renamed_path IS NOT NULL",
    )?;
    let rows = stmt.query_map([source_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// A source the last check found new artifacts in. Owns everything, so it can leave the lock.
#[derive(Debug)]
pub struct ImportJob {
    git: GitConfig,
    session: Arc<Session>,
    library_root: PathBuf,
    source: SourceRecord,
}

/// A source's files on disk, found by [`ImportJob::run`], waiting to be imported from.
#[derive(Debug)]
pub struct SourceFiles {
    source: SourceRecord,
    located: Located,
}

impl ImportJob {
    /// Finds the source's files (no lock needed): a local folder in place, a git source in the
    /// clone the check kept, else in a new shallow one.
    pub fn run(&self) -> Result<SourceFiles> {
        let located = remote::locate(&self.git, &self.session, &self.library_root, &self.source)
            .map_err(unavailable)?;
        Ok(SourceFiles {
            source: self.source.clone(),
            located,
        })
    }

    /// The report for a source whose files could not be found.
    pub(super) fn failed(&self, error: &Error) -> UpdateReport {
        let mut report = UpdateReport::new(&self.source.id, UpdateOutcome::Failed, (None, None));
        report.message = Some(format!("could not look for new artifacts: {error}"));
        report.name = Some(self.source.location.clone());
        report
    }
}

fn reported(
    candidate: &Candidate,
    outcome: UpdateOutcome,
    message: Option<String>,
) -> UpdateReport {
    let mut report = UpdateReport::new(&candidate.id, outcome, (None, None));
    report.name = Some(candidate.artifact.name.clone());
    report.source_path = Some(candidate.artifact.path.clone());
    report.message = message;
    report
}

impl Library {
    /// The sources the last check found new artifacts in, as jobs.
    pub(super) fn import_jobs(&self) -> Result<Vec<ImportJob>> {
        let ids: Vec<String> = {
            let mut stmt = self
                .store
                .conn
                .prepare("SELECT id FROM sources WHERE new_upstream > 0 ORDER BY id")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut jobs = Vec::new();
        for id in ids {
            for (source, _) in source_rows(&self.store.conn, Some(&id))? {
                jobs.push(ImportJob {
                    git: self.git.clone(),
                    session: Arc::clone(&self.session),
                    library_root: self.store.root().to_path_buf(),
                    source,
                });
            }
        }
        Ok(jobs)
    }

    /// Imports the artifacts of the source that the library neither holds nor removed, and
    /// reports each: imported, not placed, or failed. What a source's stored layout cannot place
    /// (a plugin part Claude never answered for) stays out of the library.
    pub fn finish_import(&mut self, files: SourceFiles) -> Result<Vec<UpdateReport>> {
        let SourceFiles {
            source: record,
            located,
        } = files;
        let source = Source::from_record(&record)?;
        let identity = source.identity()?;
        if ids::source_id(&identity) != record.id {
            return Err(Error::Update {
                message: "its recorded source does not match its address".to_owned(),
            });
        }
        let conn = &self.store.conn;
        let mut unpack = records::stored_layout(conn, &record.id)?.unpack;
        for folder in crate::library::unpack::held_folders(conn, &record.id)? {
            if !unpack.contains(&folder) {
                unpack.push(folder);
            }
        }
        let options = ScanOptions {
            unpack,
            unpack_by_default: false,
            ..ScanOptions::default()
        };
        let mut preview = self.scan_candidates(&source, &identity, &located.root, &options)?;
        let mut removed = records::removed_paths(conn, &record.id)?;
        // The new place of a renamed artifact is not new: applying the rename imports it.
        removed.extend(renamed_paths(conn, &record.id)?);
        let large_files: bool = conn.query_row(
            "SELECT large_files FROM sources WHERE id = ?1",
            rusqlite::params![record.id],
            |r| r.get(0),
        )?;
        if let Some(commit) = &located.commit {
            for candidate in &mut preview.candidates {
                candidate
                    .artifact
                    .metadata
                    .insert(IMPORTED_COMMIT.to_owned(), Value::String(commit.clone()));
            }
        }
        let new: Vec<&Candidate> = preview
            .candidates
            .iter()
            .filter(|c| !c.already_in_library && !removed.contains(&c.artifact.path))
            .collect();
        let placed = Placed {
            root: &located.root,
            commit: located.commit.as_deref(),
            large_files,
            hooks: &NoHooks,
        };
        let reports = self.import_each(&preview, &new, &placed);
        let left = reports
            .iter()
            .filter(|r| r.outcome != UpdateOutcome::Imported)
            .count();
        self.store.conn.execute(
            "UPDATE sources SET new_upstream = ?2 WHERE id = ?1",
            rusqlite::params![record.id, i64::try_from(left).unwrap_or(i64::MAX)],
        )?;
        Ok(reports)
    }

    /// Imports `new` in one go; when that fails (one of them cannot be placed) one by one, so
    /// the rest still land.
    fn import_each(
        &mut self,
        preview: &Preview,
        new: &[&Candidate],
        placed: &Placed<'_>,
    ) -> Vec<UpdateReport> {
        let ids: Vec<String> = new.iter().map(|c| c.id.clone()).collect();
        if ids.is_empty() {
            return Vec::new();
        }
        if self.import_located(preview, &ids, placed).is_ok() {
            return new
                .iter()
                .map(|c| reported(c, UpdateOutcome::Imported, None))
                .collect();
        }
        new.iter()
            .map(
                |c| match self.import_located(preview, std::slice::from_ref(&c.id), placed) {
                    Ok(_) => reported(c, UpdateOutcome::Imported, None),
                    Err(error @ Error::PartWithoutAnswer { .. }) => {
                        reported(c, UpdateOutcome::NotPlaced, Some(error.to_string()))
                    }
                    Err(error) => reported(c, UpdateOutcome::Failed, Some(error.to_string())),
                },
            )
            .collect()
    }
}
