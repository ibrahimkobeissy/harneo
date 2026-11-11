//! Parts of plugins that must now stay whole (ADR-006): update checks judge each unpacked plugin
//! at its source and keep the verdict on the source; the library marks the parts of a plugin
//! that fails, and *Replace with the whole plugin* (the reverse of ADR-004's unpack) reads the
//! plugin from its source again, imports it whole and gives every profile that held its parts
//! the plugin instead, as a new version. The unpacked plugin's own files are not kept, so both
//! the check and the replacement read the source (a local folder, or the clone the update check
//! kept, else a new shallow clone).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::unpack_plugin::UnpackedPart;
use super::{
    copy_artifact, copy_limit, ids, io_error, query::LibraryItem, records, Library, NoHooks,
};
use crate::artifact::{Artifact, ArtifactKind, ScanOptions};
use crate::audit;
use crate::error::{Error, Result};
use crate::plugin::WholeReason;
use crate::profiles::ProfileChange;
use crate::updates::remote;

/// The verdicts of a source: each unpacked plugin folder, and why it must stay whole.
pub(crate) type Verdicts = BTreeMap<String, Option<WholeReason>>;

/// What replacing the parts of a plugin with the plugin does, or did: the same shape for the plan
/// shown before and the outcome after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ReplaceOutcome {
    /// The plugin's name.
    pub plugin: String,
    /// The library's id for the whole plugin; empty in a plan, which does not read the source.
    pub plugin_id: String,
    /// Why the plugin must stay whole, as a sentence.
    pub reason: String,
    /// The parts that leave the library.
    pub parts: Vec<UnpackedPart>,
    /// The profiles that get a new version holding the plugin instead of its parts.
    pub profiles: Vec<ProfileChange>,
    /// Harnmap placements of the parts dropped from those profiles' current maps.
    pub placements_dropped: u32,
}

/// The verdicts stored on a source, `None` until an update check has judged it.
pub(crate) fn stored(conn: &Connection, source_id: &str) -> Result<Option<Verdicts>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT whole_reasons FROM sources WHERE id = ?1",
            params![source_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

/// Keeps what an update check found: one verdict per unpacked plugin folder it could read. A
/// folder the check could not read keeps its earlier verdict.
pub(crate) fn store(
    conn: &Connection,
    source_id: &str,
    found: &[(String, Option<WholeReason>)],
) -> Result<()> {
    let unpacked = records::stored_unpack(conn, source_id)?;
    let mut verdicts = stored(conn, source_id)?.unwrap_or_default();
    verdicts.retain(|folder, _| unpacked.contains(folder));
    for (folder, reason) in found {
        verdicts.insert(folder.clone(), reason.clone());
    }
    let json = serde_json::to_string(&verdicts)?;
    conn.execute(
        "UPDATE sources SET whole_reasons = ?2 WHERE id = ?1",
        params![source_id, json],
    )?;
    Ok(())
}

/// True when every unpacked plugin of the source has a verdict.
pub(crate) fn judged(conn: &Connection, source_id: &str) -> Result<bool> {
    let unpacked = records::stored_unpack(conn, source_id)?;
    if unpacked.is_empty() {
        return Ok(true);
    }
    let verdicts = stored(conn, source_id)?.unwrap_or_default();
    Ok(unpacked.iter().all(|f| verdicts.contains_key(f)))
}

/// The plugin an unpacked part came from: its name and folder.
fn plugin_of(artifact: &Artifact) -> Option<(String, String)> {
    let from = artifact.metadata.get("fromPlugin")?;
    let folder = from.get("folder").and_then(Value::as_str)?;
    let name = from.get("name").and_then(Value::as_str).unwrap_or(folder);
    Some((name.to_owned(), folder.to_owned()))
}

/// The sentence a part carries when its plugin must stay whole, for the part `id`.
pub(crate) fn notice_of(conn: &Connection, id: &str) -> Result<Option<String>> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT source_id, artifact FROM artifacts WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((source_id, json)) = row else {
        return Ok(None);
    };
    let Ok(artifact) = serde_json::from_str::<Artifact>(&json) else {
        return Ok(None);
    };
    let Some((_, folder)) = plugin_of(&artifact) else {
        return Ok(None);
    };
    Ok(stored(conn, &source_id)?
        .and_then(|v| v.get(&folder).cloned().flatten())
        .map(|reason| reason.text()))
}

/// Marks the listed parts whose plugin must stay whole.
pub(crate) fn mark_items(conn: &Connection, items: &mut [LibraryItem]) -> Result<()> {
    let mut cache: HashMap<String, Option<Verdicts>> = HashMap::new();
    for item in items.iter_mut().filter(|i| i.from_plugin.is_some()) {
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT source_id, artifact FROM artifacts WHERE id = ?1",
                params![item.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((source_id, json)) = row else {
            continue;
        };
        let Some((_, folder)) = serde_json::from_str::<Artifact>(&json)
            .ok()
            .and_then(|a| plugin_of(&a))
        else {
            continue;
        };
        if !cache.contains_key(&source_id) {
            cache.insert(source_id.clone(), stored(conn, &source_id)?);
        }
        item.must_stay_whole = cache
            .get(&source_id)
            .and_then(Option::as_ref)
            .and_then(|v| v.get(&folder).cloned().flatten())
            .map(|reason| reason.text());
    }
    Ok(())
}

/// The library's parts of one unpacked plugin, in id order.
fn parts_of(conn: &Connection, source_id: &str, folder: &str) -> Result<Vec<(String, Artifact)>> {
    let mut stmt =
        conn.prepare("SELECT id, artifact FROM artifacts WHERE source_id = ?1 ORDER BY id")?;
    let rows = stmt.query_map(params![source_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut parts = Vec::new();
    for row in rows {
        let (id, json) = row?;
        let Ok(artifact) = serde_json::from_str::<Artifact>(&json) else {
            continue;
        };
        if plugin_of(&artifact).is_some_and(|(_, f)| f == folder) {
            parts.push((id, artifact));
        }
    }
    Ok(parts)
}

/// Gives the plugin the parts' tags and puts it in their manual collections at the first part's
/// place. The parts' own rows cascade away when they are deleted.
fn carry_over(conn: &Connection, plugin_id: &str, part_ids: &[String]) -> Result<()> {
    for part_id in part_ids {
        conn.execute(
            "INSERT OR IGNORE INTO artifact_tags (artifact_id, tag_id)
             SELECT ?1, tag_id FROM artifact_tags WHERE artifact_id = ?2",
            params![plugin_id, part_id],
        )?;
        let held: Vec<(i64, i64)> = conn
            .prepare("SELECT collection_id, position FROM collection_items WHERE artifact_id = ?1")?
            .query_map(params![part_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (collection, position) in held {
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, artifact_id, position)
                 VALUES (?1, ?2, ?3)",
                params![collection, plugin_id, position],
            )?;
        }
    }
    Ok(())
}

impl Library {
    /// What replacing the parts `id` belongs to with their whole plugin would do. Reads only the
    /// library: the profiles that change and the parts that leave. Changes nothing.
    pub fn replace_with_whole_plan(&self, id: &str) -> Result<ReplaceOutcome> {
        let flagged = self.flagged_plugin(id)?;
        self.replace_outcome(&flagged, "", false)
    }

    /// Reads the plugin `id` belongs to from its source and imports it whole in place of its
    /// parts (see [`Library::finish_replace_with_whole`]). The three steps run back to back
    /// here; callers that share the library across threads run the middle one without it.
    pub fn replace_with_whole(&mut self, id: &str) -> Result<ReplaceOutcome> {
        let prepared = self.prepare_replace_with_whole(id)?.run()?;
        self.finish_replace_with_whole(prepared)
    }

    /// Step one: reads what replacing needs from the library (the flagged plugin, its source
    /// record and the parts as they are now). Refused unless a check found the plugin must stay
    /// whole.
    pub fn prepare_replace_with_whole(&self, id: &str) -> Result<ReplaceJob> {
        let flagged = self.flagged_plugin(id)?;
        let record = records::load(&self.store.conn, id)?
            .ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })?
            .provenance
            .source;
        let large_files: bool = self.store.conn.query_row(
            "SELECT large_files FROM sources WHERE id = ?1",
            params![record.id],
            |r| r.get(0),
        )?;
        Ok(ReplaceJob {
            git: self.git.clone(),
            session: std::sync::Arc::clone(&self.session),
            library_root: self.store.root().to_path_buf(),
            id: id.to_owned(),
            record,
            large_files,
            flagged,
        })
    }

    /// Step three, under the lock: refuses unless the parts, the verdict and the plugin's
    /// absence are as step one saw them, then imports the plugin whole in place of its parts:
    /// the parts leave the library (their tags and collections go to the plugin), every profile
    /// holding any of them gets the plugin instead as a new version, their maps forget the
    /// parts' placements and the source stops unpacking the plugin. All or nothing.
    pub fn finish_replace_with_whole(
        &mut self,
        prepared: PreparedReplace,
    ) -> Result<ReplaceOutcome> {
        let PreparedReplace {
            id,
            record,
            flagged,
            plugin,
            plugin_id,
            copied,
            plugin_audit,
            staging,
            dir,
        } = prepared;
        let now_flagged = self.flagged_plugin(&id).map_err(|_| changed(&flagged))?;
        if now_flagged.fingerprint() != flagged.fingerprint() {
            return Err(changed(&flagged));
        }
        if records::existing(&self.store.conn, &[plugin_id.as_str()])?.contains(&plugin_id) {
            return Err(Error::PluginExclusive {
                plugin: flagged.name,
                detail: "the plugin is already in the library".into(),
            });
        }
        let artifacts_dir = self.store.artifacts_dir();
        let part_ids: Vec<String> = flagged.parts.iter().map(|(i, _)| i.clone()).collect();
        let mut placed: Option<PathBuf> = None;
        let outcome = (|| -> Result<ReplaceOutcome> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            records::insert_artifact(
                &tx,
                &plugin_id,
                &record.id,
                &plugin,
                now,
                &copied.replaced_secret_keys,
                &plugin_audit,
            )?;
            crate::updates::record_copy_digest(
                &tx,
                &plugin_id,
                &crate::updates::copy_digest(&dir)?,
            )?;
            crate::bundles::add_reference_links(&tx, &[plugin_id.as_str()], now)?;
            records::update_unpack(&tx, &record.id, &[], std::slice::from_ref(&flagged.folder))?;
            let mut verdicts = stored(&tx, &record.id)?.unwrap_or_default();
            verdicts.remove(&flagged.folder);
            tx.execute(
                "UPDATE sources SET whole_reasons = ?2 WHERE id = ?1",
                params![record.id, serde_json::to_string(&verdicts)?],
            )?;
            carry_over(&tx, &plugin_id, &part_ids)?;
            let done = Self::replace_outcome_in(&tx, &flagged, &plugin_id, true)?;
            for part in &part_ids {
                if !records::delete(&tx, part)? {
                    return Err(Error::NotInLibrary { id: part.clone() });
                }
            }
            let target = artifacts_dir.join(&plugin_id);
            if target.exists() {
                fs::remove_dir_all(&target).map_err(|e| io_error(&target, e))?;
            }
            fs::rename(&dir, &target).map_err(|e| io_error(&target, e))?;
            placed = Some(target);
            tx.commit()?;
            Ok(done)
        })();
        drop(staging);
        match outcome {
            Ok(done) => {
                for part in &part_ids {
                    // A leftover folder with no row is harmless, so a failure is not reported.
                    let _ = fs::remove_dir_all(artifacts_dir.join(part));
                }
                Ok(done)
            }
            Err(error) => {
                if let Some(folder) = placed {
                    // Best effort: the error being reported matters more than a failed cleanup.
                    let _ = fs::remove_dir_all(folder);
                }
                Err(error)
            }
        }
    }

    fn replace_outcome(&self, f: &Flagged, plugin_id: &str, apply: bool) -> Result<ReplaceOutcome> {
        Self::replace_outcome_in(&self.store.conn, f, plugin_id, apply)
    }

    fn replace_outcome_in(
        conn: &Connection,
        f: &Flagged,
        plugin_id: &str,
        apply: bool,
    ) -> Result<ReplaceOutcome> {
        let part_ids: Vec<&str> = f.parts.iter().map(|(i, _)| i.as_str()).collect();
        let profiles =
            crate::profiles::replace_parts_with_plugin(conn, plugin_id, &part_ids, apply)?;
        let mut dropped = 0;
        for profile in &profiles {
            for part in &part_ids {
                dropped += crate::harnmap::drop_from_current_draw(conn, profile.id, part, apply)?;
            }
        }
        Ok(ReplaceOutcome {
            plugin: f.name.clone(),
            plugin_id: plugin_id.to_owned(),
            reason: f.reason.text(),
            parts: f
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

    /// The plugin an unpacked part `id` belongs to, when a check found it must stay whole.
    fn flagged_plugin(&self, id: &str) -> Result<Flagged> {
        let conn = &self.store.conn;
        let detail =
            records::load(conn, id)?.ok_or_else(|| Error::NotInLibrary { id: id.to_owned() })?;
        let source_id = detail.provenance.source.id;
        let (name, folder) = plugin_of(&detail.artifact).ok_or_else(|| Error::NotAPlugin {
            name: detail.artifact.name.clone(),
        })?;
        let reason = stored(conn, &source_id)?
            .and_then(|v| v.get(&folder).cloned().flatten())
            .ok_or_else(|| Error::Update {
                message: format!("{name} has not been found to need staying whole"),
            })?;
        let parts = parts_of(conn, &source_id, &folder)?;
        Ok(Flagged {
            name,
            folder,
            reason,
            parts,
        })
    }
}

/// An unpacked plugin that must now stay whole, with the parts the library holds of it.
#[derive(Clone)]
struct Flagged {
    name: String,
    folder: String,
    reason: WholeReason,
    parts: Vec<(String, Artifact)>,
}

impl Flagged {
    /// What must be unchanged between preparing and finishing a replacement: the plugin, the
    /// verdict, and each part's id and record.
    fn fingerprint(&self) -> (String, String, String, Vec<(String, String)>) {
        (
            self.name.clone(),
            self.folder.clone(),
            self.reason.text(),
            self.parts
                .iter()
                .map(|(id, a)| (id.clone(), serde_json::to_string(a).unwrap_or_default()))
                .collect(),
        )
    }
}

fn changed(flagged: &Flagged) -> Error {
    Error::Update {
        message: format!(
            "{} changed in the library while its source was read; try again",
            flagged.name
        ),
    }
}

/// Step one of replacing parts with their whole plugin: what the slow step needs, read under
/// the lock. It holds no library.
pub struct ReplaceJob {
    git: super::git::GitConfig,
    session: std::sync::Arc<remote::Session>,
    library_root: PathBuf,
    id: String,
    record: records::SourceRecord,
    large_files: bool,
    flagged: Flagged,
}

/// The plugin read from its source and copied to a staging folder, ready to be written.
pub struct PreparedReplace {
    id: String,
    record: records::SourceRecord,
    flagged: Flagged,
    plugin: Artifact,
    plugin_id: String,
    copied: super::copy::Copied,
    plugin_audit: audit::Audit,
    staging: tempfile::TempDir,
    dir: PathBuf,
}

impl ReplaceJob {
    /// Step two, no library: locates the source (cloning when no clone is kept), scans it,
    /// judges the plugin again and copies it to a staging folder.
    pub fn run(self) -> Result<PreparedReplace> {
        let Self {
            git,
            session,
            library_root,
            id,
            record,
            large_files,
            flagged,
        } = self;
        let located =
            remote::locate(&git, &session, &library_root, &record).map_err(|failure| {
                Error::Update {
                    message: match failure {
                        remote::Failure::Missing => "the source folder is gone".to_owned(),
                        remote::Failure::Unreachable(reason) => reason,
                    },
                }
            })?;
        let scanned = remote::scan(
            &located.root,
            &record,
            &super::layout_claude::StoredLayout::default(),
        )
        .map_err(|_| Error::Update {
            message: "the source could not be read".to_owned(),
        })?;
        let mut plugin = scanned
            .artifacts
            .into_iter()
            .find(|a| a.kind == ArtifactKind::Plugin && a.path == flagged.folder)
            .ok_or_else(|| Error::Update {
                message: "the plugin is gone from its source".to_owned(),
            })?;
        let options = ScanOptions::default();
        let mut walker = crate::artifact::walk::Walker::new(&located.root, &options);
        if crate::plugin::must_stay_whole(&mut walker, &plugin).is_none() {
            return Err(Error::Update {
                message: format!(
                    "{} can stay unpacked at its source now; check for updates again",
                    flagged.name
                ),
            });
        }
        if let Some(commit) = &located.commit {
            plugin.metadata.insert(
                crate::updates::IMPORTED_COMMIT.to_owned(),
                Value::String(commit.clone()),
            );
        }
        let source = super::Source::from_record(&record)?;
        let plugin_id = ids::artifact_id(&source.identity()?, &plugin.path);
        let staging = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(&library_root)
            .map_err(|e| io_error(&library_root, e))?;
        let dir = staging.path().join(&plugin_id);
        fs::create_dir(&dir).map_err(|e| io_error(&dir, e))?;
        let copied = copy_artifact(
            &located.root,
            &plugin,
            &plugin_id,
            &dir,
            &NoHooks,
            copy_limit(large_files),
        )?;
        let mut plugin_audit = audit::audit(&plugin, &located.root, &options);
        plugin_audit
            .unaudited
            .retain(|f| !copied.skipped.iter().any(|s| &s.file == f));
        Ok(PreparedReplace {
            id,
            record,
            flagged,
            plugin,
            plugin_id,
            copied,
            plugin_audit,
            staging,
            dir,
        })
    }
}
