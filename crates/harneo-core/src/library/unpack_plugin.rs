//! Unpacking a plugin already in the library whole (ADR-004): its pickable parts become ordinary
//! artifacts built from the stored copy, the plugin is removed, and every profile holding it gets
//! its parts instead as a new version. One transaction; the parts' folders are placed first and
//! taken back if anything fails.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    copy, copy_artifact, copy_limit, ids, io_error, records, unpack, Library, NoHooks, Source,
};
use crate::artifact::walk::Walker;
use crate::artifact::{Artifact, ArtifactKind, ScanOptions, ScanReport};
use crate::audit::{self, Audit};
use crate::error::{Error, Result};
use crate::plugin::{self, WholeReason};
use crate::profiles::ProfileChange;

/// One artifact an unpack creates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UnpackedPart {
    pub id: String,
    pub name: String,
    pub kind: ArtifactKind,
}

/// What unpacking a plugin does, or did: the same shape for the plan shown before and the
/// outcome after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UnpackOutcome {
    /// Why the plugin stays whole, when it does; nothing else is filled then.
    pub refused: Option<WholeReason>,
    /// The same reason as a sentence.
    pub reason: Option<String>,
    /// The artifacts the plugin becomes.
    pub parts: Vec<UnpackedPart>,
    /// The profiles that get a new version with the parts instead of the plugin.
    pub profiles: Vec<ProfileChange>,
    /// Harnmap placements of the plugin dropped from those profiles' current maps.
    pub placements_dropped: u32,
}

impl UnpackOutcome {
    fn refused(reason: WholeReason) -> Self {
        Self {
            reason: Some(reason.text()),
            refused: Some(reason),
            parts: Vec::new(),
            profiles: Vec::new(),
            placements_dropped: 0,
        }
    }
}

/// The plugin read back from its stored copy.
struct Built {
    source_id: String,
    large_files: bool,
    /// A folder holding the stored files at their source paths, which the parts are read from.
    scan_root: tempfile::TempDir,
    parts: Vec<(String, Artifact)>,
    refused: Option<WholeReason>,
}

fn part_kinds(parts: &[(String, Artifact)]) -> Vec<(String, ArtifactKind)> {
    parts.iter().map(|(id, a)| (id.clone(), a.kind)).collect()
}

impl Library {
    /// The source of a stored artifact, as the import first saw it.
    fn source_of(&self, id: &str) -> Result<(Source, String)> {
        let detail = records::load(&self.store.conn, id)?
            .ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })?;
        let record = detail.provenance.source;
        let source = if record.kind == "git" {
            Source::Git {
                url: record.location,
                branch: record.branch,
                subfolder: record.subfolder,
            }
        } else {
            Source::Local {
                path: PathBuf::from(record.location),
            }
        };
        Ok((source, record.id))
    }

    /// Reads the plugin's parts from its stored copy as a scan of the source would have given
    /// them (same paths, so the same ids).
    fn build_parts(&self, plugin_id: &str) -> Result<(Artifact, Built)> {
        let detail =
            records::load(&self.store.conn, plugin_id)?.ok_or_else(|| Error::NotInLibrary {
                id: plugin_id.to_owned(),
            })?;
        let plugin = detail.artifact;
        if plugin.kind != ArtifactKind::Plugin {
            return Err(Error::NotAPlugin { name: plugin.name });
        }
        let (source, source_id) = self.source_of(plugin_id)?;
        let identity = source.identity()?;
        if ids::artifact_id(&identity, &plugin.path) != plugin_id
            || ids::source_id(&identity) != source_id
        {
            return Err(Error::NotInLibrary {
                id: plugin_id.to_owned(),
            });
        }
        let large_files: bool = self.store.conn.query_row(
            "SELECT large_files FROM sources WHERE id = ?1",
            rusqlite::params![source_id],
            |r| r.get(0),
        )?;
        let stored = self.store.artifacts_dir().join(plugin_id);
        let scan_root = tempfile::Builder::new()
            .prefix(".unpack-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        for (from, kept) in copy::plan(&plugin)? {
            let from_stored = copy::safe_join(&stored, &kept)?;
            if !from_stored.is_file() {
                continue;
            }
            let to = copy::safe_join(scan_root.path(), &from)?;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
            }
            fs::copy(&from_stored, &to).map_err(|e| io_error(&to, e))?;
        }
        let options = ScanOptions::default();
        let mut walker = Walker::new(scan_root.path(), &options);
        let mut refused = plugin::must_stay_whole(&mut walker, &plugin);
        let mut parts = Vec::new();
        if refused.is_none() {
            let found =
                crate::artifact::claude::unpack_plugin(&mut walker, &plugin, &mut Vec::new());
            if found.is_empty() {
                refused = Some(WholeReason::NoParts);
            } else {
                let mut report = ScanReport {
                    artifacts: found,
                    ..ScanReport::default()
                };
                crate::artifact::digest::fill(&mut walker, &mut report);
                parts = report
                    .artifacts
                    .into_iter()
                    .map(|a| (ids::artifact_id(&identity, &a.path), a))
                    .collect();
            }
        }
        Ok((
            plugin,
            Built {
                source_id,
                large_files,
                scan_root,
                parts,
                refused,
            },
        ))
    }

    /// What unpacking the plugin `id` would do: its parts and the profiles that would change, or
    /// why it stays whole. Changes nothing.
    pub fn unpack_plan(&self, id: &str) -> Result<UnpackOutcome> {
        let (_, built) = self.build_parts(id)?;
        self.outcome_of(id, &built, false)
    }

    fn outcome_of(&self, id: &str, built: &Built, apply: bool) -> Result<UnpackOutcome> {
        if let Some(reason) = &built.refused {
            return Ok(UnpackOutcome::refused(reason.clone()));
        }
        let conn = &self.store.conn;
        let parts = part_kinds(&built.parts);
        let profiles = crate::profiles::replace_with_parts(conn, id, &parts, apply)?;
        let mut dropped = 0;
        for profile in &profiles {
            dropped += crate::harnmap::drop_from_current_draw(conn, profile.id, id, apply)?;
        }
        Ok(UnpackOutcome {
            refused: None,
            reason: None,
            parts: built
                .parts
                .iter()
                .map(|(id, a)| UnpackedPart {
                    id: id.clone(),
                    name: a.name.clone(),
                    kind: a.kind,
                })
                .collect(),
            profiles,
            placements_dropped: u32::try_from(dropped).unwrap_or(u32::MAX),
        })
    }

    /// Turns the whole plugin `id` into its parts. A plugin that must stay whole is refused
    /// with the reason and nothing changes. Otherwise the parts become ordinary artifacts of the
    /// plugin's source (audited, with `fromPlugin`, and the source's stored choice now unpacks
    /// the plugin), the plugin is removed, every profile holding it gets its parts as a new
    /// version and its map forgets the plugin's placement. All or nothing.
    pub fn unpack_in_library(&mut self, id: &str) -> Result<UnpackOutcome> {
        let (plugin, built) = self.build_parts(id)?;
        if let Some(reason) = built.refused {
            return Ok(UnpackOutcome::refused(reason));
        }
        let known = records::existing(
            &self.store.conn,
            &built
                .parts
                .iter()
                .map(|(i, _)| i.as_str())
                .collect::<Vec<_>>(),
        )?;
        if !known.is_empty() {
            return Err(Error::PluginExclusive {
                plugin: plugin.name,
                detail: "parts of the plugin are already in the library; remove them first".into(),
            });
        }
        let root = built.scan_root.path();
        let options = ScanOptions::default();
        let staging = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(self.store.root())
            .map_err(|e| io_error(self.store.root(), e))?;
        let mut audits: HashMap<&str, Audit> = HashMap::new();
        let mut replaced: HashMap<&str, Vec<String>> = HashMap::new();
        for (part_id, part) in &built.parts {
            let mut audit = audit::audit(part, root, &options);
            let dir = staging.path().join(part_id);
            fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
            let copied = copy_artifact(
                root,
                part,
                part_id,
                &dir,
                &NoHooks,
                copy_limit(built.large_files),
            )?;
            audit
                .unaudited
                .retain(|f| !copied.skipped.iter().any(|s| &s.file == f));
            audits.insert(part_id, audit);
            replaced.insert(part_id, copied.replaced_secret_keys);
        }
        let folders: Vec<String> = built
            .parts
            .iter()
            .filter_map(|(_, a)| unpack::part_folder(a))
            .collect();
        let artifacts_dir = self.store.artifacts_dir();
        let mut placed: Vec<PathBuf> = Vec::new();
        let outcome = (|| -> Result<UnpackOutcome> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            for (part_id, part) in &built.parts {
                let audit = audits
                    .get(part_id.as_str())
                    .ok_or_else(|| Error::NotInPreview {
                        id: part_id.clone(),
                    })?;
                let keys = replaced
                    .get(part_id.as_str())
                    .map_or(&[][..], Vec::as_slice);
                records::insert_artifact(&tx, part_id, &built.source_id, part, now, keys, audit)?;
                crate::updates::record_copy_digest(
                    &tx,
                    part_id,
                    &crate::updates::copy_digest(&staging.path().join(part_id))?,
                )?;
            }
            let fresh: Vec<&str> = built.parts.iter().map(|(i, _)| i.as_str()).collect();
            crate::bundles::add_reference_links(&tx, &fresh, now)?;
            records::update_unpack(&tx, &built.source_id, &folders, &[])?;
            carry_over(&tx, id, &fresh)?;
            let parts = part_kinds(&built.parts);
            let profiles = crate::profiles::replace_with_parts(&tx, id, &parts, true)?;
            let mut dropped = 0;
            for profile in &profiles {
                dropped += crate::harnmap::drop_from_current_draw(&tx, profile.id, id, true)?;
            }
            // The parts keep the source alive, so removing the plugin row cannot drop it.
            if !records::delete(&tx, id)? {
                return Err(Error::NotInLibrary { id: id.to_owned() });
            }
            for (part_id, _) in &built.parts {
                let target = artifacts_dir.join(part_id);
                if target.exists() {
                    fs::remove_dir_all(&target).map_err(|e| io_error(&target, e))?;
                }
                fs::rename(staging.path().join(part_id), &target)
                    .map_err(|e| io_error(&target, e))?;
                placed.push(target);
            }
            tx.commit()?;
            Ok(UnpackOutcome {
                refused: None,
                reason: None,
                parts: built
                    .parts
                    .iter()
                    .map(|(pid, a)| UnpackedPart {
                        id: pid.clone(),
                        name: a.name.clone(),
                        kind: a.kind,
                    })
                    .collect(),
                profiles,
                placements_dropped: u32::try_from(dropped).unwrap_or(u32::MAX),
            })
        })();
        match outcome {
            Ok(done) => {
                remove_copy(&artifacts_dir.join(id));
                Ok(done)
            }
            Err(error) => {
                for folder in &placed {
                    // Best effort: the error being reported matters more than a failed cleanup.
                    let _ = fs::remove_dir_all(folder);
                }
                Err(error)
            }
        }
    }
}

/// Gives the parts the plugin's tags and puts them in its manual collections at its place, in
/// part order. The plugin's own rows cascade away when it is deleted.
fn carry_over(conn: &rusqlite::Connection, plugin_id: &str, part_ids: &[&str]) -> Result<()> {
    let count = i64::try_from(part_ids.len()).unwrap_or(i64::MAX);
    for part_id in part_ids {
        conn.execute(
            "INSERT OR IGNORE INTO artifact_tags (artifact_id, tag_id)
             SELECT ?1, tag_id FROM artifact_tags WHERE artifact_id = ?2",
            rusqlite::params![part_id, plugin_id],
        )?;
    }
    let held: Vec<(i64, i64)> = conn
        .prepare("SELECT collection_id, position FROM collection_items WHERE artifact_id = ?1")?
        .query_map(rusqlite::params![plugin_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    for (collection, position) in held {
        // Room for the parts after the plugin's slot, which the first part takes over.
        conn.execute(
            "UPDATE collection_items SET position = position + ?3
             WHERE collection_id = ?1 AND position > ?2",
            rusqlite::params![collection, position, count - 1],
        )?;
        for (offset, part_id) in (0..).zip(part_ids) {
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, artifact_id, position)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![collection, part_id, position + offset],
            )?;
        }
    }
    Ok(())
}

/// Deletes the plugin's own stored copy once its row is gone; a leftover folder with no row is
/// harmless, so a failure is not reported.
fn remove_copy(folder: &Path) {
    let _ = fs::remove_dir_all(folder);
}
