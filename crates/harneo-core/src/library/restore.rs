//! Putting artifacts that arrived inside a profile file into the library, with the provenance
//! the file records. Same guarantees as an import: staged first, audited, then either every
//! folder and row lands or none does.

use std::fs;
use std::path::PathBuf;

use rusqlite::params;
use serde_json::Value;

use super::copy::{safe_join, stored_view};
use super::records::{self, Provenance};
use super::{io_error, ArtifactId, Library};
use crate::artifact::{Artifact, ArtifactKind, ScanOptions};
use crate::audit;
use crate::error::{Error, Result};

/// One artifact to restore: its scan record, where it came from and its stored files.
pub(crate) struct Restored {
    pub id: ArtifactId,
    pub artifact: Artifact,
    pub provenance: Provenance,
    /// Stored files (relative to the artifact's folder) and their bytes.
    pub files: Vec<(String, Vec<u8>)>,
    /// The stored files that keep their executable bit.
    pub executable: Vec<String>,
}

/// What a restored artifact's source location starts with: it has no real source, so nothing
/// is ever cloned or scanned for it.
pub(crate) const PROFILE_FILE_SOURCE: &str = "profile file ";

fn refused(reason: &str) -> Error {
    Error::InvalidProfile {
        reason: reason.into(),
    }
}

/// Rebuilds from the stored bytes what the audit and install read from the scan record, so a
/// file cannot say one thing in its record and store another. An MCP artifact must store a
/// `server.json` holding exactly its own server (install writes every key it finds into
/// `.mcp.json`); its definition becomes the record's metadata. An agent's `tools` come from
/// the stored file's frontmatter.
fn rebuild(artifact: &Artifact, dir: &std::path::Path) -> Result<Artifact> {
    let mut artifact = artifact.clone();
    match artifact.kind {
        ArtifactKind::Mcp => {
            let bytes = fs::read(safe_join(dir, "server.json")?)
                .map_err(|_| refused("an MCP server in the file has no server.json"))?;
            let Ok(Value::Object(mut servers)) = serde_json::from_slice::<Value>(&bytes) else {
                return Err(refused("an MCP server's server.json is not a JSON object"));
            };
            match (servers.len(), servers.remove(&artifact.name)) {
                (1, Some(Value::Object(definition))) => artifact.metadata = definition,
                _ => {
                    return Err(refused(
                        "an MCP server's server.json must hold exactly the server it names",
                    ))
                }
            }
        }
        ArtifactKind::Agent => {
            let (view, _) = stored_view(&artifact)?;
            let text = safe_join(dir, &view.path)
                .ok()
                .and_then(|file| fs::read_to_string(file).ok())
                .unwrap_or_default();
            let data = crate::artifact::frontmatter::parse(&text).data();
            match data.get("tools") {
                Some(tools) => artifact.metadata.insert("tools".into(), tools.clone()),
                None => artifact.metadata.remove("tools"),
            };
        }
        _ => {}
    }
    Ok(artifact)
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
        let mut rebuilt = Vec::new();
        for item in &fresh {
            let source = &item.provenance.source;
            let location_ok = match source.kind.as_str() {
                "local" => {
                    std::path::Path::new(&source.location).is_absolute()
                        || source.location.starts_with(PROFILE_FILE_SOURCE)
                }
                "git" => crate::library::Source::parse_git(
                    &source.location,
                    source.branch.as_deref(),
                    source.subfolder.as_deref(),
                )
                .is_ok(),
                _ => false,
            };
            if !is_id(&item.id) || !is_id(&source.id) || !location_ok {
                return Err(refused(
                    "the file holds an artifact with a malformed id or source",
                ));
            }
            // Companion files must land inside the harness (this refuses the artifact if not).
            crate::profiles::layout::placed(&item.artifact)?;
            let dir = staging.path().join(&item.id);
            fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
            for (rel, bytes) in &item.files {
                let target = safe_join(&dir, rel)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
                }
                fs::write(&target, bytes).map_err(|e| io_error(&target, e))?;
                if item.executable.contains(rel) {
                    super::make_executable(&target).map_err(|e| io_error(&target, e))?;
                }
            }
            let artifact = rebuild(&item.artifact, &dir)?;
            let (view, to_source) = stored_view(&artifact)?;
            let mut result = audit::audit_stored(&view, &dir, &options);
            for finding in &mut result.findings {
                if let Some(source) = to_source.get(&finding.file) {
                    finding.file.clone_from(source);
                }
            }
            audits.push(result);
            rebuilt.push(artifact);
        }
        let artifacts_dir = self.store.artifacts_dir();
        let mut placed: Vec<PathBuf> = Vec::new();
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            for ((item, audit), artifact) in fresh.iter().zip(&audits).zip(&rebuilt) {
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
                    artifact,
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
