//! What an install will do, before it does it: the [`Plan`], the scaffold plan and the path
//! rules every plan is held to (relative, inside the project, no symlinks, never a local file).

use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::{Error, Result};
use crate::library::{ArtifactId, Library};
use crate::profiles::{PathFrom, ProfileId};
use crate::updates::Hunk;

/// What a plan does with one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum WriteAction {
    /// The file does not exist yet and is created.
    New,
    /// The file exists, is backed up, then overwritten.
    Replace,
    /// The file exists, is backed up, then deleted.
    Delete,
    /// The project's file stays as it is.
    Keep,
    /// The profile's file is not added.
    Skip,
}

/// One path of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Write {
    /// Relative to the project root, with forward slashes.
    pub path: String,
    pub action: WriteAction,
    /// The content written (`New`, `Replace`); base64 over IPC.
    #[serde(with = "optional_bytes")]
    #[ts(type = "string | null")]
    pub bytes: Option<Vec<u8>>,
    /// Line changes against the project's file, for a refit's `Replace`.
    pub diff: Option<Vec<Hunk>>,
    /// The library artifact the content comes from; `None` for generated files.
    pub artifact: Option<ArtifactId>,
    /// The file is written executable (a script); unix only, absent means not.
    #[serde(default)]
    pub executable: bool,
}

/// What a plan is for. `apply` registers the project for every kind but `Removal`, which
/// unregisters it, and `Restore`, which leaves the registration as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum PlanKind {
    /// A profile into a folder with no harness.
    #[default]
    Scaffold,
    /// A profile into a folder that has a harness.
    Refit,
    /// The difference between a project's profile and another profile or version.
    Switch,
    /// Harneo's files out, the originals back, the project unregistered.
    Removal,
    /// The files of a backup set put back.
    Restore,
}

/// Everything an install will do to one project. Nothing is written until [`Library::apply`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Plan {
    /// The project folder, canonical.
    pub target: String,
    pub profile_id: ProfileId,
    pub profile_name: String,
    pub profile_version: u32,
    /// Why the backup set is made, for the Backups tab ("Scaffold web v2").
    pub reason: String,
    pub writes: Vec<Write>,
    /// Files that will be backed up first (`Replace` and `Delete`).
    pub backup_count: u32,
    /// What the plan is for; plans from older callers are scaffolds.
    #[serde(default)]
    pub kind: PlanKind,
    /// Managed files that were edited in the project since Harneo wrote them (switch and
    /// removal). Each is still backed up before it is replaced or deleted.
    #[serde(default)]
    pub edited_in_project: Vec<String>,
    /// Paths whose write puts back something that is not Harneo's content (a project's original
    /// file): the manifest stops listing them once applied.
    #[serde(default)]
    pub restored: Vec<String>,
}

mod optional_bytes {
    use super::{Engine, STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => s.serialize_some(&STANDARD.encode(b)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|t| STANDARD.decode(t).map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// What marks a folder as already having a harness.
const HARNESS_MARKERS: [&str; 3] = [".claude", "CLAUDE.md", ".mcp.json"];
/// Files that are the user's own and are never read, written or deleted.
const LOCAL_FILES: [&str; 2] = ["CLAUDE.local.md", "settings.local.json"];

fn install(message: impl Into<String>) -> Error {
    Error::Install {
        message: message.into(),
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `secs` (Unix) as UTC `(year, month, day, hour, minute, second)`.
fn utc(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, rem / 3_600, rem % 3_600 / 60, rem % 60)
}

/// `2026-10-23`.
pub(crate) fn utc_date(secs: i64) -> String {
    let (y, m, d, ..) = utc(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `2026-10-23T14-05-09Z`: sorts by time and is a valid folder name everywhere.
pub(crate) fn utc_stamp(secs: i64) -> String {
    let (y, mo, d, h, mi, s) = utc(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}Z")
}

/// The names of `rel` when it is a safe project path: relative, plain names only, forward
/// slashes, nothing under `.git/`, never one of the user's local files.
pub(crate) fn safe_parts(rel: &str) -> Result<Vec<&str>> {
    let refuse = |why: &str| install(format!("{rel}: {why}"));
    if rel.is_empty() || rel.contains('\\') || rel.contains('\0') {
        return Err(refuse("not a plain relative path"));
    }
    let mut parts = Vec::new();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(refuse("not a plain relative path"));
        }
        if part.eq_ignore_ascii_case(".git") {
            return Err(refuse("Harneo never touches .git"));
        }
        parts.push(part);
    }
    if Path::new(rel).is_absolute() {
        return Err(refuse("not a plain relative path"));
    }
    if parts
        .last()
        .is_some_and(|n| LOCAL_FILES.iter().any(|l| n.eq_ignore_ascii_case(l)))
    {
        return Err(refuse("a local file, which Harneo never touches"));
    }
    Ok(parts)
}

/// `target/rel` after the path rules, refusing a symlink at any existing step below `target`.
pub(crate) fn resolve_inside(target: &Path, rel: &str) -> Result<PathBuf> {
    let mut path = target.to_path_buf();
    for part in safe_parts(rel)? {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::UnsafePath {
                    path: format!("{rel} (a symlink)"),
                });
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    path: path.display().to_string(),
                    source,
                })
            }
        }
    }
    Ok(path)
}

/// The canonical project folder, which must be a real folder (not a symlink path that moved).
pub(crate) fn project_folder(target: &str) -> Result<PathBuf> {
    let path = Path::new(target);
    let canonical = path
        .canonicalize()
        .map_err(|_| install(format!("{target} is not a folder")))?;
    if canonical != path || !canonical.is_dir() {
        return Err(install(format!("{target} is not a folder")));
    }
    Ok(canonical)
}

/// Checks every path of a plan: safe, unique, and not behind a symlink.
pub(crate) fn check_plan(plan: &Plan) -> Result<PathBuf> {
    let target = project_folder(&plan.target)?;
    let mut seen = std::collections::HashSet::new();
    for write in &plan.writes {
        resolve_inside(&target, &write.path)?;
        if !seen.insert(write.path.to_ascii_lowercase()) {
            return Err(install(format!("{} appears twice in the plan", write.path)));
        }
        let needs_bytes = matches!(write.action, WriteAction::New | WriteAction::Replace);
        // A `Keep` may carry the bytes of a file that is already identical, so the manifest tracks it.
        let allows_bytes = needs_bytes || write.action == WriteAction::Keep;
        if needs_bytes && write.bytes.is_none() || !allows_bytes && write.bytes.is_some() {
            return Err(install(format!(
                "{}: content does not match its action",
                write.path
            )));
        }
    }
    Ok(target)
}

/// Proves the folder accepts files by creating and removing one.
fn probe_writable(dir: &Path) -> Result<()> {
    let file = tempfile::Builder::new()
        .prefix(".harneo-probe-")
        .tempfile_in(dir)
        .map_err(|_| install(format!("{} is not writable", dir.display())))?;
    file.close()
        .map_err(|_| install(format!("{} is not writable", dir.display())))
}

/// The canonical folder a scaffold may write into: it is a folder, holds no harness and accepts files.
fn scaffold_folder(target: &Path) -> Result<PathBuf> {
    let shown = target.display().to_string();
    if !target.is_dir() {
        return Err(install(format!("{shown} is not a folder")));
    }
    let canonical = target
        .canonicalize()
        .map_err(|_| install(format!("{shown} is not a folder")))?;
    let found: Vec<&str> = HARNESS_MARKERS
        .into_iter()
        .filter(|m| fs::symlink_metadata(canonical.join(m)).is_ok())
        .collect();
    if !found.is_empty() {
        return Err(Error::HasHarness {
            path: shown,
            found: found.join(", "),
        });
    }
    probe_writable(&canonical)?;
    Ok(canonical)
}

impl Library {
    /// Checks a folder for scaffolding without a profile: refused when it has `.claude/`,
    /// `CLAUDE.md` or `.mcp.json`, is not a folder, or is not writable. Writes nothing.
    pub fn check_scaffold_target(&self, target: &Path) -> Result<()> {
        scaffold_folder(target).map(|_| ())
    }

    /// The plan to write profile `profile_id` (at `version`, the current one when `None`) into a
    /// folder with no harness. Writes nothing. Refused when the folder has `.claude/`,
    /// `CLAUDE.md` or `.mcp.json`, is not a folder, or is not writable.
    pub fn plan_scaffold(
        &self,
        target: &Path,
        profile_id: ProfileId,
        version: Option<u32>,
    ) -> Result<Plan> {
        let canonical = scaffold_folder(target)?;
        let view = self.profile(profile_id, version)?;
        let rendered = self.render(
            &view.spec,
            &view.name,
            view.version,
            &utc_date(crate::library::records::now()),
        )?;
        let writes = rendered
            .files
            .into_iter()
            .map(|file| Write {
                path: file.path,
                action: WriteAction::New,
                bytes: Some(file.bytes),
                diff: None,
                artifact: match file.from {
                    PathFrom::Artifact { artifact } => Some(artifact),
                    PathFrom::Generated { .. } => None,
                },
                executable: file.executable,
            })
            .collect();
        let plan = Plan {
            target: canonical.display().to_string(),
            profile_id,
            reason: format!("Scaffold {} v{}", view.name, view.version),
            profile_name: view.name,
            profile_version: view.version,
            writes,
            backup_count: 0,
            kind: PlanKind::Scaffold,
            edited_in_project: Vec::new(),
            restored: Vec::new(),
        };
        check_plan(&plan)?;
        Ok(plan)
    }
}
