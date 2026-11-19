//! Applying a rename at the source (decision 10 of `spec/updates.md`): the new artifact is
//! imported and everything that pointed at the old one moves over, in one transaction; then the
//! old copy is removed, with no tombstone. The staged copy of the new artifact comes from the
//! same prepare step as an update (`UpdateJob::run`).
//!
//! Moved: tags, collections, links, the profiles' current versions (each gets a new version
//! naming the new artifact), installed files, harnfit fits, the manual-only default and the
//! import date. The old name stays as a usage alias, so uses logged under either name count.
//! Any failure puts the old folder back and leaves the old artifact as it was.

use std::fs;
use std::io;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::apply::{backup_copy, PreparedUpdate, RenameTarget, UpdateOutcome, UpdateReport};
use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::library::plugin_record;
use crate::library::records::{self, ArtifactDetail};
use crate::library::{ids, ImportHooks, Library, Source};

/// An artifact the last check found renamed at its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RenameItem {
    /// The library id of the artifact as it is now (the old name).
    pub id: String,
    pub kind: ArtifactKind,
    pub name: String,
    /// Its name in the source now.
    pub new_name: String,
    /// The library copy was edited: *Update all* leaves it for the user to apply one by one.
    pub local_edits: bool,
}

/// The renames *Update all* will apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RenamePlan {
    pub renames: Vec<RenameItem>,
}

fn stale(what: &str) -> Error {
    Error::Update {
        message: format!("{what} since it was prepared; check again"),
    }
}

impl Library {
    /// The artifacts the last check found renamed at their source, with their new names.
    pub fn renamed_at_source_plan(&self) -> Result<RenamePlan> {
        let rows = self
            .store
            .conn
            .prepare(
                "SELECT a.id, a.kind, a.name, u.renamed_name FROM artifacts a \
                 JOIN upstream u ON u.artifact_id = a.id WHERE u.state = 'renamed' \
                 ORDER BY a.name COLLATE NOCASE, a.id",
            )?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut renames = Vec::with_capacity(rows.len());
        for (id, kind, name, new_name) in rows {
            renames.push(RenameItem {
                kind: serde_json::from_value(serde_json::Value::String(kind))?,
                new_name: new_name.unwrap_or_else(|| name.clone()),
                local_edits: self.has_local_edits(&id)?,
                id,
                name,
            });
        }
        Ok(RenamePlan { renames })
    }

    /// Where the renamed artifact `detail` went: its new path in the source and the id that
    /// path gives it.
    pub(super) fn rename_target(&self, detail: &ArtifactDetail) -> Result<RenameTarget> {
        let path: Option<String> = self.store.conn.query_row(
            "SELECT renamed_path FROM upstream WHERE artifact_id = ?1",
            params![detail.id],
            |r| r.get(0),
        )?;
        let path = path.ok_or_else(|| Error::Update {
            message: "its new place in the source is not known; check again".to_owned(),
        })?;
        let identity = Source::from_record(&detail.provenance.source)?.identity()?;
        Ok(RenameTarget {
            id: ids::artifact_id(&identity, &path),
            path,
        })
    }

    /// Refuses a rename whose preparation is stale: the flag changed, the old copy changed, or
    /// the new artifact is in the library already.
    fn check_rename_current(&self, prepared: &PreparedUpdate, target: &RenameTarget) -> Result<()> {
        let old = prepared.id.as_str();
        let (state, ..) = self.flagged(old).map_err(|_| stale("its flag changed"))?;
        let path: Option<String> = self.store.conn.query_row(
            "SELECT renamed_path FROM upstream WHERE artifact_id = ?1",
            params![old],
            |r| r.get(0),
        )?;
        if state != "renamed" || path.as_deref() != Some(target.path.as_str()) {
            return Err(stale("its flag changed"));
        }
        if records::load(&self.store.conn, &target.id)?.is_some() {
            return Err(Error::Update {
                message: "the renamed artifact is in the library already".to_owned(),
            });
        }
        let folder = self.store.artifacts_dir().join(old);
        let copy = if folder.is_dir() {
            Some(super::copy_digest(&folder)?)
        } else {
            None
        };
        if copy != prepared.seen_copy_digest() {
            return Err(stale("the library copy changed"));
        }
        Ok(())
    }

    /// Step three of a rename: backs up an edited old copy, moves everything over and swaps the
    /// copies. *Update all* leaves an edited one alone.
    pub(super) fn finish_rename(
        &mut self,
        prepared: PreparedUpdate,
        hooks: &dyn ImportHooks,
    ) -> Result<UpdateReport> {
        let old = prepared.id.clone();
        let Some(target) = prepared.rename.clone() else {
            return Err(stale("its flag changed"));
        };
        self.check_rename_current(&prepared, &target)?;
        let edited = self.has_local_edits(&old)?;
        if edited && prepared.protect_edits() {
            return Ok(UpdateReport::new(
                &old,
                UpdateOutcome::SkippedLocalEdits,
                prepared.versions(),
            ));
        }
        let backup = if edited {
            Some(backup_copy(
                self.store.root(),
                &old,
                &self.store.artifacts_dir().join(&old),
            )?)
        } else {
            None
        };
        self.swap_rename(&prepared, &target, hooks)?;
        let mut report = UpdateReport::new(&old, UpdateOutcome::Renamed, prepared.versions());
        report.name = Some(prepared.artifact.name.clone());
        report.source_path = Some(target.path);
        report.backup = backup.map(|p| p.display().to_string());
        Ok(report)
    }

    fn swap_rename(
        &mut self,
        prepared: &PreparedUpdate,
        target: &RenameTarget,
        hooks: &dyn ImportHooks,
    ) -> Result<()> {
        let (old, new) = (prepared.id.as_str(), target.id.as_str());
        let artifacts_dir = self.store.artifacts_dir();
        let (old_folder, new_folder) = (artifacts_dir.join(old), artifacts_dir.join(new));
        let staged = prepared.staging.path().join(new);
        let library_root = self.store.root().to_path_buf();
        let io_error = |path: &std::path::Path, source: io::Error| Error::Io {
            path: path.display().to_string(),
            source,
        };
        // Not swept on open: if a crash leaves the old copy here, it is still the user's data.
        let parked_in = tempfile::Builder::new()
            .prefix(".swap-")
            .tempdir_in(&library_root)
            .map_err(|e| io_error(&library_root, e))?;
        let parked = parked_in.path().join(old);
        let (mut moved_old, mut placed_new) = (false, false);
        let outcome = (|| -> Result<()> {
            let tx = self.store.conn.transaction()?;
            let now = records::now();
            let (source_id, old_name, imported_at): (String, String, i64) = tx.query_row(
                "SELECT source_id, name, imported_at FROM artifacts WHERE id = ?1",
                params![old],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let artifact = &prepared.artifact;
            records::insert_artifact(
                &tx,
                new,
                &source_id,
                artifact,
                now,
                &prepared.replaced_secret_keys,
                &prepared.audit,
            )?;
            // The new artifact keeps the old one's library default and place in the timeline.
            tx.execute(
                "UPDATE artifacts SET imported_at = ?3,
                        manual_only = (SELECT manual_only FROM artifacts WHERE id = ?1)
                 WHERE id = ?2",
                params![old, new, imported_at],
            )?;
            if let Some((plugin_source, built)) = &prepared.plugin {
                plugin_record::save(&tx, plugin_source, built)?;
            }
            super::record_copy_digest(&tx, new, &prepared.copy_digest)?;
            tx.execute(
                "DELETE FROM removed_paths WHERE source_id = ?1 AND source_path = ?2",
                params![source_id, artifact.path],
            )?;
            // What the new version references replaces what the old one did.
            tx.execute(
                "DELETE FROM links WHERE from_id = ?1 AND origin = 'reference'
                   AND status = 'confirmed'",
                params![old],
            )?;
            // Everything that points at the old id points at the new one.
            for sql in [
                "UPDATE OR IGNORE artifact_tags SET artifact_id = ?2 WHERE artifact_id = ?1",
                "UPDATE OR IGNORE collection_items SET artifact_id = ?2 WHERE artifact_id = ?1",
                "UPDATE OR IGNORE links SET from_id = ?2 WHERE from_id = ?1",
                "UPDATE OR IGNORE links SET to_id = ?2 WHERE to_id = ?1",
                "UPDATE installed_files SET artifact_id = ?2 WHERE artifact_id = ?1",
                "UPDATE OR IGNORE harnfit_fits SET candidate = ?2 WHERE candidate = ?1",
            ] {
                tx.execute(sql, params![old, new])?;
            }
            crate::bundles::add_reference_links(&tx, &[new], now)?;
            crate::profiles::replace_artifact(&tx, old, new)?;
            crate::usage::add_alias(&tx, artifact.kind, &old_name, &artifact.name)?;
            if !records::delete(&tx, old)? {
                return Err(Error::NotInLibrary { id: old.to_owned() });
            }
            match fs::rename(&old_folder, &parked) {
                Ok(()) => moved_old = true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(&old_folder, e)),
            }
            // A folder with no row is left over from a crash; it is not a library entry.
            if new_folder.exists() {
                fs::remove_dir_all(&new_folder).map_err(|e| io_error(&new_folder, e))?;
            }
            fs::rename(&staged, &new_folder).map_err(|e| io_error(&new_folder, e))?;
            placed_new = true;
            hooks
                .before_commit()
                .map_err(|e| io_error(&library_root, e))?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = outcome {
            // Best effort: the error being reported matters more than a failed restore.
            if placed_new {
                let _ = fs::remove_dir_all(&new_folder);
            }
            if moved_old {
                let _ = fs::rename(&parked, &old_folder);
            }
            return Err(error);
        }
        Ok(())
    }
}
