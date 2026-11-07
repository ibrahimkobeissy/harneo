//! Promoting a project's change into the library: the project's file becomes the library copy
//! (the old copy backed up first), so every project on the profile can get it. Also importing
//! an artifact that was added in a project.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use super::check::disk_bytes;
use crate::artifact::{AgentAdapter, ArtifactKind, ClaudeAdapter, ScanOptions};
use crate::audit;
use crate::error::{Error, Result};
use crate::install::Project;
use crate::library::records;
use crate::library::{safe_join, stored_view, ArtifactId, Library, Preview, Source};
use crate::profiles::layout::{load, placed};
use crate::updates::backup_copy;

/// What a promote did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PromoteReport {
    pub path: String,
    pub artifact: ArtifactId,
    /// Where the previous library copy was saved.
    pub backup: String,
}

/// A library preview of a project's added artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct AddedPreview {
    pub preview: Preview,
    /// The candidate at the path, to preselect; `None` when the scan found none there.
    pub candidate: Option<ArtifactId>,
}

fn refuse(message: impl Into<String>) -> Error {
    Error::Drift {
        message: message.into(),
    }
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// The project's `CLAUDE.md` without the `@HARNESS.md` import an install added (the library copy
/// never has it), when the library copy lacks it. Everything else is kept.
fn without_import(project: &[u8], stored: &[u8]) -> Vec<u8> {
    let added = crate::install::with_harness_import(stored) != stored;
    let Ok(text) = std::str::from_utf8(project) else {
        return project.to_vec();
    };
    if !added {
        return project.to_vec();
    }
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    if let Some(at) = lines.iter().rposition(|l| l.trim() == "@HARNESS.md") {
        lines.remove(at);
        if at > 0 && at == lines.len() && lines[at - 1].trim().is_empty() {
            lines.remove(at - 1);
        }
    }
    lines.concat().into_bytes()
}

/// The keys of `settings.json` where an install merges the profile's plugins.
const MERGED_KEYS: [&str; 2] = crate::plugin::MERGED_SETTINGS_KEYS;

fn object_at<'a>(root: &'a Value, key: &str) -> Option<&'a Map<String, Value>> {
    root.get(key).and_then(Value::as_object)
}

/// The project's `settings.json` as a template again: every entry the install merged in
/// (`rendered` has it, the library `template` lacks it or holds another value) goes back to
/// the template's state. Refused when the project changed or removed such an entry, since only
/// the profile can say that. Entries the user added in the project are kept.
fn without_merged_plugins(
    path: &str,
    disk: &[u8],
    rendered: &[u8],
    template: &[u8],
) -> Result<Vec<u8>> {
    let parse = |bytes: &[u8], what: &str| -> Result<Value> {
        serde_json::from_slice(bytes)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| refuse(format!("{path}: the {what} is not a JSON object")))
    };
    let mut project = parse(disk, "project's file")?;
    let rendered = parse(rendered, "installed version")?;
    let template = parse(template, "library copy")?;
    let Value::Object(root) = &mut project else {
        return Err(refuse(format!(
            "{path}: the project's file is not a JSON object"
        )));
    };
    for key in MERGED_KEYS {
        let Some(merged) = object_at(&rendered, key) else {
            continue;
        };
        let own = object_at(&template, key);
        for (name, value) in merged {
            let original = own.and_then(|o| o.get(name));
            if original == Some(value) {
                continue;
            }
            let entries = root.get_mut(key).and_then(Value::as_object_mut);
            let current = entries.as_ref().and_then(|e| e.get(name));
            if current != Some(value) {
                return Err(refuse(format!(
                    "{path}: the project changed {key} \"{name}\", which the profile sets; \
                     change the profile instead, or re-apply"
                )));
            }
            if let Some(entries) = entries {
                match original {
                    Some(v) => {
                        entries.insert(name.clone(), v.clone());
                    }
                    None => {
                        entries.remove(name);
                    }
                }
            }
        }
        let emptied = root
            .get(key)
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty);
        if emptied && own.is_none() {
            root.remove(key);
        }
    }
    let mut out = serde_json::to_vec_pretty(&project)?;
    out.push(b'\n');
    Ok(out)
}

/// Everything a promote writes, worked out before anything is written.
struct Prepared {
    id: ArtifactId,
    artifact: crate::artifact::Artifact,
    folder: std::path::PathBuf,
    /// The library file the project's file replaces, and its current bytes.
    file: std::path::PathBuf,
    old: Vec<u8>,
    /// The new library copy, and the project's file it came from.
    bytes: Vec<u8>,
    disk: Vec<u8>,
    replaced_keys: Option<Vec<String>>,
}

impl Library {
    /// Why promoting `path` of `project` would be refused, or `None` when it would go ahead.
    /// The same checks [`Self::promote`] runs, without writing anything.
    pub(super) fn promote_blocked(&self, project: &Project, path: &str) -> Option<String> {
        self.prepare_promote(project, path)
            .err()
            .map(|e| e.to_string())
    }

    /// Makes the project's edited copy of `path` the library copy of its artifact. Refused for
    /// files that are not edited (deleted, unchanged) and for generated files (`HARNESS.md`,
    /// `.mcp.json`, a generated `settings.json`). For a Settings template with plugins merged
    /// in, the merged entries are taken out first (see [`without_merged_plugins`]). The old
    /// library copy is backed up, the new one is audited again and recorded as the copy's own
    /// state (so it is not a local edit), and this project's manifest takes the project's file
    /// as what Harneo wrote.
    pub fn promote(&self, project_id: i64, path: &str) -> Result<PromoteReport> {
        let project = self.project(project_id)?;
        let _lock = self.lock_data_dir()?;
        let Prepared {
            id,
            artifact,
            folder,
            file,
            old,
            bytes,
            disk,
            replaced_keys,
        } = self.prepare_promote(&project, path)?;

        let backup = backup_copy(self.store.root(), &id, &folder)?;
        let write = |content: &[u8]| -> Result<()> {
            crate::install::write_atomic(&file, content).map_err(|e| io_error(&file, e))
        };
        write(&bytes)?;
        let recorded = (|| -> Result<()> {
            let (view, to_source) = stored_view(&artifact)?;
            let mut fresh = audit::audit_stored(&view, &folder, &ScanOptions::default());
            for finding in &mut fresh.findings {
                if let Some(source) = to_source.get(&finding.file) {
                    finding.file.clone_from(source);
                }
            }
            let tx = self.store.conn.unchecked_transaction()?;
            // For updates the copy now counts as edited, so it is never replaced unasked.
            let mut marked = artifact.clone();
            marked.metadata.insert(
                crate::updates::PROMOTED_AT.to_owned(),
                serde_json::Value::from(records::now()),
            );
            tx.execute(
                "UPDATE artifacts SET artifact = ?2 WHERE id = ?1",
                rusqlite::params![id, serde_json::to_string(&marked)?],
            )?;
            if let Some(keys) = &replaced_keys {
                tx.execute(
                    "UPDATE artifacts SET replaced_secret_keys = ?2 WHERE id = ?1",
                    rusqlite::params![id, serde_json::to_string(keys)?],
                )?;
            }
            records::update_audit(&tx, &id, &fresh, records::now())?;
            crate::updates::record_copy_digest(&tx, &id, &crate::updates::copy_digest(&folder)?)?;
            tx.execute(
                "UPDATE installed_files SET digest = ?3 WHERE project_id = ?1 AND path = ?2",
                rusqlite::params![project.id, path, crate::install::digest(&disk)],
            )?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = recorded {
            // The library copy goes back as it was; the error says why, and when even that
            // failed, where the previous copy is.
            if let Err(restore) = write(&old) {
                return Err(refuse(format!(
                    "{error}; restoring the previous library copy also failed ({restore}); \
                     it is saved in {}",
                    backup.display()
                )));
            }
            return Err(error);
        }
        Ok(PromoteReport {
            path: path.to_owned(),
            artifact: id,
            backup: backup.display().to_string(),
        })
    }

    /// Every check of a promote, and the new library copy. Reads only.
    fn prepare_promote(&self, project: &Project, path: &str) -> Result<Prepared> {
        let root = crate::install::project_folder(&project.path)?;
        let entry = self
            .installed_files(project.id)?
            .into_iter()
            .find(|f| f.path == path)
            .ok_or_else(|| refuse(format!("{path} is not a file Harneo installed")))?;
        let Some(id) = entry.artifact_id else {
            return Err(refuse(format!(
                "{path} is generated by Harneo; re-apply it instead"
            )));
        };
        let disk = disk_bytes(&root, path)?.ok_or_else(|| {
            refuse(format!(
                "{path} was deleted; only re-apply can bring it back"
            ))
        })?;
        if crate::install::digest(&disk) == entry.digest {
            return Err(refuse(format!("{path} has no changes to promote")));
        }
        let artifact =
            load(&self.store.conn, &id)?.ok_or_else(|| Error::NotInLibrary { id: id.clone() })?;
        let (view, _) = stored_view(&artifact)?;
        let stored = placed(&artifact)?
            .into_iter()
            .find(|(_, project_path)| project_path == path)
            .map(|(stored, _)| stored)
            .ok_or_else(|| refuse(format!("{path} is no longer a file of {}", artifact.name)))?;
        let folder = self.store.artifacts_dir().join(&id);
        let file = safe_join(&folder, &stored)?;
        let old = std::fs::read(&file).map_err(|e| io_error(&file, e))?;
        let main = stored == view.path;
        let mut replaced_keys: Option<Vec<String>> = None;
        let bytes = match artifact.kind {
            ArtifactKind::Settings if main => {
                let date = crate::install::utc_date(project.applied_at);
                let rendered = self
                    .render_pinned(project, &date)
                    .ok()
                    .and_then(|r| r.files.into_iter().find(|f| f.path == path))
                    .map(|f| f.bytes);
                let template = match rendered {
                    Some(rendered) if rendered != old => {
                        without_merged_plugins(path, &disk, &rendered, &old)?
                    }
                    _ => disk.clone(),
                };
                let (bytes, keys) = crate::library::rewrite(&view, &view.path, template)?;
                replaced_keys = Some(keys);
                bytes
            }
            ArtifactKind::ClaudeMd if path == "CLAUDE.md" => without_import(&disk, &old),
            _ => disk.clone(),
        };
        Ok(Prepared {
            id,
            artifact,
            folder,
            file,
            old,
            bytes,
            disk,
            replaced_keys,
        })
    }

    /// A library preview of the project's folder with the artifact at `path` (added in the
    /// project) to preselect. Nothing is imported.
    pub fn promote_added(&self, project_id: i64, path: &str) -> Result<AddedPreview> {
        let project = self.project(project_id)?;
        crate::install::safe_parts(path)?;
        let preview = self.preview(&Source::Local {
            path: project.path.into(),
        })?;
        let candidate = preview
            .candidates
            .iter()
            .find(|c| crate::artifact::rel_string(&ClaudeAdapter.install_path(&c.artifact)) == path)
            .map(|c| c.id.clone());
        if candidate.is_none() {
            return Err(refuse(format!("{path} is not an artifact of the project")));
        }
        Ok(AddedPreview { preview, candidate })
    }
}
