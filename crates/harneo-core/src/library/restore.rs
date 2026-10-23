//! Putting artifacts that arrived inside a profile file into the library, with the provenance
//! the file records. Same guarantees as an import: staged first, audited, then either every
//! folder and row lands or none does.

use std::fs;
use std::path::PathBuf;

use rusqlite::params;

use super::copy::{safe_join, stored_view};
use super::records::{self, Provenance};
use super::{io_error, ArtifactId, Library};
use crate::artifact::{Artifact, ScanOptions};
use crate::audit;
use crate::error::{Error, Result};

/// One artifact to restore: its scan record, where it came from and its stored files.
pub(crate) struct Restored {
    pub id: ArtifactId,
    pub artifact: Artifact,
    pub provenance: Provenance,
    /// Stored files (relative to the artifact's folder) and their bytes.
    pub files: Vec<(String, Vec<u8>)>,
}

fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Library {
    /// Stores `items` (ones already in the library are skipped) and returns the ids added.
    pub(crate) fn restore_artifacts(&mut self, items: Vec<Restored>) -> Result<Vec<ArtifactId>> {
        let known = records::existing(
            &self.store.conn,
            &items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        )?;
        let fresh: Vec<&Restored> = items.iter().filter(|i| !known.contains(&i.id)).collect();
        if fresh.is_empty() {
            return Ok(Vec::new());
        }
        let staging = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        let options = ScanOptions::default();
        let mut audits = Vec::new();
        for item in &fresh {
            let bad_source = &item.provenance.source;
            if !is_id(&item.id)
                || !is_id(&bad_source.id)
                || !matches!(bad_source.kind.as_str(), "local" | "git")
            {
                return Err(Error::InvalidProfile {
                    reason: "the file holds an artifact with a malformed id or source".into(),
                });
            }
            let dir = staging.path().join(&item.id);
            fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
            for (rel, bytes) in &item.files {
                let target = safe_join(&dir, rel)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
                }
                fs::write(&target, bytes).map_err(|e| io_error(&target, e))?;
            }
            let (view, to_source) = stored_view(&item.artifact)?;
            let mut result = audit::audit(&view, &dir, &options);
            for finding in &mut result.findings {
                if let Some(source) = to_source.get(&finding.file) {
                    finding.file.clone_from(source);
                }
            }
            audits.push(result);
        }
        let artifacts_dir = self.store.artifacts_dir();
        let mut placed: Vec<PathBuf> = Vec::new();
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            for (item, audit) in fresh.iter().zip(&audits) {
                let source = &item.provenance.source;
                tx.execute(
                    "INSERT OR IGNORE INTO sources (id, kind, location, branch, subfolder, commit_sha)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        source.id,
                        source.kind,
                        source.location,
                        source.branch,
                        source.subfolder,
                        source.commit_sha
                    ],
                )?;
                records::insert_artifact(
                    &tx,
                    &item.id,
                    &source.id,
                    &item.artifact,
                    item.provenance.imported_at,
                    &item.provenance.replaced_secret_keys,
                    audit,
                )?;
                crate::updates::record_copy_digest(
                    &tx,
                    &item.id,
                    &crate::updates::copy_digest(&staging.path().join(&item.id))?,
                )?;
            }
            let ids: Vec<&str> = fresh.iter().map(|i| i.id.as_str()).collect();
            crate::bundles::add_reference_links(&tx, &ids, now)?;
            for item in &fresh {
                let target = artifacts_dir.join(&item.id);
                if target.exists() {
                    fs::remove_dir_all(&target).map_err(|e| io_error(&target, e))?;
                }
                fs::rename(staging.path().join(&item.id), &target)
                    .map_err(|e| io_error(&target, e))?;
                placed.push(target);
            }
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            for folder in &placed {
                // Best effort: the error being reported matters more than a failed cleanup.
                let _ = fs::remove_dir_all(folder);
            }
            return Err(error);
        }
        Ok(fresh.iter().map(|i| i.id.clone()).collect())
    }
}
