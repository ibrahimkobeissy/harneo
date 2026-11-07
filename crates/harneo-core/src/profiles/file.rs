//! `<name>.harneo-profile.json`: one self-contained file holding a profile version and every
//! artifact it resolves to, so another machine needs nothing else.
//!
//! The file is untrusted on the way in: its size is capped, a newer format is refused, every
//! stored path must be a plain relative path, literal secrets are refused, and artifacts are
//! stored through the library's atomic path with a fresh audit.

use crate::harnmap::{ArtifactKey, CategoryId, FileFit, FileHarnmap, PreparedMap};
use std::collections::HashMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{invalid, is_placeholder, ProfileId, ProfileSpec, MAX_NAME_CHARS};
use crate::artifact::literal_secret::{is_literal_secret, REDACTED};
use crate::artifact::{Artifact, ArtifactKind};
use crate::error::Result;
use crate::library::{is_executable, ArtifactId, Library, Provenance, Restored};

/// The `format` field of every profile file.
pub const FORMAT: &str = "harneo-profile";
/// The newest `formatVersion` this build reads and writes.
pub const FORMAT_VERSION: u32 = 2;
/// The most draws or saved fits a file may carry is set in the Harnmap module.
/// The largest profile file, exported or imported.
pub const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
/// Added to the name of an imported profile whose name is taken.
pub const IMPORTED_SUFFIX: &str = " (imported)";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileFile {
    format: String,
    format_version: u32,
    exported_at: i64,
    profile: FileProfile,
    artifacts: Vec<FileArtifact>,
    /// The map: the current draw, and earlier ones when history is included (format 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    harnmap: Option<FileHarnmap>,
    /// Saved Harnfit verdicts (format 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fits: Vec<FileFit>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileProfile {
    name: String,
    description: String,
    version: u32,
    spec: ProfileSpec,
    /// Absent in a format 1 file: `software`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<CategoryId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category_note: Option<String>,
}

/// What an export carries beyond the profile and its artifacts. The current map is always in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    /// Include the earlier drawings of the map.
    pub include_history: bool,
    /// Include the saved Harnfit verdicts.
    pub include_fits: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_history: true,
            include_fits: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileArtifact {
    id: ArtifactId,
    artifact: Artifact,
    provenance: Provenance,
    files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    path: String,
    encoding: Encoding,
    content: String,
    /// The stored file is executable; absent means not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    executable: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Encoding {
    Utf8,
    Base64,
}

impl FileEntry {
    fn new(path: String, bytes: &[u8], executable: bool) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self {
                path,
                encoding: Encoding::Utf8,
                content: text.to_owned(),
                executable,
            },
            Err(_) => Self {
                path,
                encoding: Encoding::Base64,
                content: STANDARD.encode(bytes),
                executable,
            },
        }
    }

    fn bytes(&self) -> Result<Vec<u8>> {
        match self.encoding {
            Encoding::Utf8 => Ok(self.content.clone().into_bytes()),
            Encoding::Base64 => STANDARD
                .decode(&self.content)
                .map_err(|_| invalid("a file in the profile file is not valid base64")),
        }
    }
}

/// The profile a file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ImportedProfile {
    pub name: String,
    pub description: String,
    /// The version it was exported at.
    pub version: u32,
    pub spec: ProfileSpec,
}

/// One artifact a file carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ImportArtifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    /// Already here, by id or by content digest; importing leaves it alone.
    pub in_library: bool,
    /// Where the file says it came from: `owner/repo` or a folder name.
    pub source: String,
    /// How many files the artifact carries.
    pub file_count: u32,
}

/// What importing a file would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileImportPreview {
    pub profile: ImportedProfile,
    pub artifacts: Vec<ImportArtifact>,
    /// A profile of that name exists (ignoring case): the import adds " (imported)".
    pub name_clash: bool,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileImportReport {
    pub profile_id: ProfileId,
    /// The name the profile got.
    pub name: String,
    /// Artifacts newly stored in the library.
    pub imported: Vec<ArtifactId>,
    /// How many drawings of the map the profile got (0: no map).
    pub map_draws: u32,
    /// How many saved fits the profile got.
    pub fits: u32,
}

/// Names of the entries of `env` / `headers` maps in `value` that hold a literal secret.
fn literal_keys_in(value: &Value, prefix: &str, out: &mut Vec<String>) {
    for field in ["env", "headers"] {
        let Some(Value::Object(map)) = value.get(field) else {
            continue;
        };
        for (key, v) in map {
            if v.as_str()
                .is_some_and(|v| v != REDACTED && is_literal_secret(key, v))
            {
                out.push(format!("{prefix}{field}.{key}"));
            }
        }
    }
}

/// Key names (never values) of the literal secrets in one artifact: its scan record and its
/// stored Settings file or MCP definition.
fn literal_secrets(item: &FileArtifact, bytes_of: &[(String, Vec<u8>)]) -> Vec<String> {
    let mut keys = Vec::new();
    literal_keys_in(
        &Value::Object(item.artifact.metadata.clone()),
        "",
        &mut keys,
    );
    for (path, bytes) in bytes_of {
        let Ok(json) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        match item.artifact.kind {
            ArtifactKind::Settings if path.ends_with("settings.json") => {
                literal_keys_in(&json, "", &mut keys);
            }
            ArtifactKind::Mcp if path == "server.json" => {
                if let Value::Object(servers) = &json {
                    for server in servers.values() {
                        literal_keys_in(server, "", &mut keys);
                    }
                }
            }
            _ => {}
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

fn secret_refusal(name: &str, keys: &[String]) -> crate::error::Error {
    invalid(format!(
        "{name} holds literal secret values ({}); a profile file carries placeholders only",
        keys.join(", ")
    ))
}

fn decode_files(entries: &[FileEntry]) -> Result<Vec<(String, Vec<u8>)>> {
    entries
        .iter()
        .map(|e| Ok((e.path.clone(), e.bytes()?)))
        .collect()
}

/// A file read and checked, before anything is stored.
struct Parsed {
    file: ProfileFile,
    /// Each file artifact's local id when the library has it by id or digest.
    local: HashMap<ArtifactId, ArtifactId>,
    /// The id each file artifact gets here (see [`own_id`]), by the id the file gives it.
    own: HashMap<ArtifactId, ArtifactId>,
    /// The checked map part, mapped to this library's ids.
    map: PreparedMap,
}

/// The id an artifact from a profile file gets: derived from what it stores, in a namespace no
/// real source can produce, so a file can neither claim the id of an artifact imported from a
/// real source nor make a later genuine import look "already in library". The same content
/// always gets the same id, so importing a file twice stores it once.
fn own_id(item: &FileArtifact, files: &[(String, Vec<u8>)]) -> ArtifactId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let kind = serde_json::to_string(&item.artifact.kind).unwrap_or_default();
    for part in [
        kind.as_str(),
        item.artifact.name.as_str(),
        item.artifact.path.as_str(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort();
    for (path, bytes) in sorted {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    crate::library::ids::artifact_id("profile-file", &hash)
}

/// The source row an artifact from a profile file gets: no folder and no address, only the
/// file's profile name, so no update check ever reaches git or a local scan for it.
fn file_source(profile_name: &str) -> crate::library::SourceRecord {
    let name: String = profile_name
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .take(MAX_NAME_CHARS)
        .collect();
    crate::library::SourceRecord {
        id: crate::library::ids::source_id(&format!("profile-file:{name}")),
        kind: "local".to_owned(),
        location: format!("{}{}", crate::library::PROFILE_FILE_SOURCE, name.trim()),
        branch: None,
        subfolder: None,
        commit_sha: None,
    }
}

fn match_local(conn: &Connection, item: &FileArtifact, own: &str) -> Result<Option<ArtifactId>> {
    let by_id = conn
        .query_row(
            "SELECT id FROM artifacts WHERE id = ?1",
            params![own],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    if by_id.is_some() {
        return Ok(by_id);
    }
    let Some(digest) = &item.artifact.digest else {
        return Ok(None);
    };
    let kind = serde_json::to_value(item.artifact.kind)?;
    Ok(conn
        .query_row(
            "SELECT id FROM artifacts WHERE digest = ?1 AND kind = ?2 ORDER BY id LIMIT 1",
            params![digest, kind.as_str().unwrap_or_default()],
            |r| r.get(0),
        )
        .optional()?)
}

fn name_taken(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM profiles WHERE name = ?1 COLLATE NOCASE)",
        params![name],
        |r| r.get(0),
    )?)
}

/// The spec's references to artifacts the file does not carry, or of the wrong kind.
fn check_spec(file: &ProfileFile) -> Result<()> {
    let kinds: HashMap<&str, ArtifactKind> = file
        .artifacts
        .iter()
        .map(|a| (a.id.as_str(), a.artifact.kind))
        .collect();
    let kind_of = |id: &str| {
        kinds
            .get(id)
            .copied()
            .ok_or_else(|| invalid("the profile names an artifact the file does not carry"))
    };
    let spec = &file.profile.spec;
    if spec.items.is_empty() && spec.settings.is_none() && spec.mcp.is_empty() {
        return Err(invalid("a profile needs at least one artifact"));
    }
    for id in &spec.items {
        if matches!(kind_of(id)?, ArtifactKind::Settings | ArtifactKind::Mcp) {
            return Err(invalid(
                "the profile file lists a template or server as an item",
            ));
        }
    }
    if let Some(id) = &spec.settings {
        if kind_of(id)? != ArtifactKind::Settings {
            return Err(invalid(
                "the profile file's template is not a Settings artifact",
            ));
        }
    }
    for choice in &spec.mcp {
        if kind_of(&choice.artifact)? != ArtifactKind::Mcp {
            return Err(invalid("the profile file's server is not an MCP artifact"));
        }
        if choice.env.values().any(|v| !is_placeholder(v)) {
            return Err(invalid(
                "the profile file has an MCP value that is not a placeholder",
            ));
        }
    }
    for id in spec.manual_only.keys() {
        kind_of(id)?;
    }
    Ok(())
}

fn key_of(artifact: &Artifact) -> ArtifactKey {
    ArtifactKey {
        kind: artifact.kind,
        name: artifact.name.clone(),
        digest: artifact.digest.clone().unwrap_or_default(),
    }
}

fn parse(library: &Library, bytes: &[u8]) -> Result<Parsed> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(invalid("the profile file is larger than 50 MiB"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("this is not a Harneo profile file (not valid JSON)"))?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(invalid("this is not a Harneo profile file"));
    }
    let version = value
        .get("formatVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("the profile file has no format version"))?;
    if version > u64::from(FORMAT_VERSION) {
        return Err(invalid(format!(
            "this profile file is format {version}, newer than this Harneo reads \
             ({FORMAT_VERSION}); update Harneo to import it"
        )));
    }
    let mut file: ProfileFile = serde_json::from_value(value)
        .map_err(|e| invalid(format!("the profile file is damaged: {e}")))?;
    if version < 2 {
        file.harnmap = None;
        file.fits.clear();
        file.profile.category = None;
        file.profile.category_note = None;
    }
    check_spec(&file)?;
    let conn = &library.store.conn;
    let mut local = HashMap::new();
    let mut own = HashMap::new();
    for item in &file.artifacts {
        let files = decode_files(&item.files)?;
        let keys = literal_secrets(item, &files);
        if !keys.is_empty() {
            return Err(secret_refusal(&item.artifact.name, &keys));
        }
        let id = own_id(item, &files);
        if own.insert(item.id.clone(), id.clone()).is_some() {
            return Err(invalid("the profile file lists an artifact id twice"));
        }
        if let Some(found) = match_local(conn, item, &id)? {
            local.insert(item.id.clone(), found);
        }
    }
    let mut resolver = HashMap::new();
    for item in &file.artifacts {
        if let Some(id) = local.get(&item.id).or_else(|| own.get(&item.id)) {
            resolver.insert(key_of(&item.artifact), id.clone());
        }
    }
    let map = library.prepare_map(file.harnmap.as_ref(), &file.fits, &resolver)?;
    Ok(Parsed {
        file,
        local,
        own,
        map,
    })
}

/// `name` made unique among profiles by " (imported)", then a number.
fn free_name(conn: &Connection, name: &str) -> Result<String> {
    let base: String = name.trim().chars().take(MAX_NAME_CHARS - 16).collect();
    let mut candidate = format!("{base}{IMPORTED_SUFFIX}");
    let mut n = 2;
    while name_taken(conn, &candidate)? {
        candidate = format!("{base} (imported {n})");
        n += 1;
    }
    Ok(candidate)
}

fn remap(spec: &ProfileSpec, local: &HashMap<ArtifactId, ArtifactId>) -> ProfileSpec {
    let map = |id: &ArtifactId| local.get(id).unwrap_or(id).clone();
    ProfileSpec {
        items: spec.items.iter().map(map).collect(),
        settings: spec.settings.as_ref().map(map),
        mcp: spec
            .mcp
            .iter()
            .map(|m| super::McpChoice {
                artifact: map(&m.artifact),
                env: m.env.clone(),
            })
            .collect(),
        manual_only: spec.manual_only.iter().map(|(k, v)| (map(k), *v)).collect(),
    }
}

impl Library {
    /// The profile at `version` (the current one when `None`) with every artifact it resolves
    /// to, as the bytes of a `.harneo-profile.json` file. Refuses when an artifact is gone from
    /// the library, a file holds a literal secret, or the result is over 50 MiB. The file also
    /// carries the category and the map (every drawing with `options.include_history`, else
    /// only the current one) and the saved fits (with `options.include_fits`), naming artifacts
    /// by kind, name and digest.
    pub fn export_profile(
        &self,
        id: ProfileId,
        version: Option<u32>,
        options: ExportOptions,
    ) -> Result<Vec<u8>> {
        let view = self.profile(id, version)?;
        let resolved = self.resolve(&view.spec)?;
        if !resolved.missing.is_empty() {
            return Err(invalid(
                "the profile uses artifacts that are no longer in the library",
            ));
        }
        let mut artifacts = Vec::new();
        let mut key_map = HashMap::new();
        let mut total = 0usize;
        for item in &resolved.artifacts {
            let detail = self.detail(&item.id)?;
            let folder = self.store.artifacts_dir().join(&item.id);
            let mut stored = Vec::new();
            let mut executable = Vec::new();
            for rel in &detail.files {
                let target = crate::library::safe_join(&folder, rel)?;
                let bytes = std::fs::read(&target).map_err(|e| crate::Error::Io {
                    path: target.display().to_string(),
                    source: e,
                })?;
                total += bytes.len();
                if total > MAX_FILE_BYTES {
                    return Err(invalid("the profile is larger than 50 MiB to export"));
                }
                executable.push(is_executable(&target));
                stored.push((rel.clone(), bytes));
            }
            let entry = FileArtifact {
                id: item.id.clone(),
                artifact: detail.artifact,
                provenance: detail.provenance,
                files: stored
                    .iter()
                    .zip(&executable)
                    .map(|((p, b), x)| FileEntry::new(p.clone(), b, *x))
                    .collect(),
            };
            let keys = literal_secrets(&entry, &stored);
            if !keys.is_empty() {
                return Err(secret_refusal(&entry.artifact.name, &keys));
            }
            key_map.insert(entry.id.clone(), key_of(&entry.artifact));
            artifacts.push(entry);
        }
        let (harnmap, fits) = self.export_map(id, &key_map, options)?;
        let file = ProfileFile {
            format: FORMAT.to_owned(),
            format_version: FORMAT_VERSION,
            exported_at: crate::library::records::now(),
            profile: FileProfile {
                name: view.name,
                description: view.description,
                version: view.version,
                spec: view.spec,
                category: Some(view.category),
                category_note: view.category_note,
            },
            artifacts,
            harnmap,
            fits,
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(invalid("the profile is larger than 50 MiB to export"));
        }
        Ok(bytes)
    }

    /// Reads a profile file and says what importing it would do. Changes nothing.
    pub fn import_profile_file(&self, bytes: &[u8]) -> Result<ProfileImportPreview> {
        let conn = &self.store.conn;
        let Parsed { file, local, .. } = parse(self, bytes)?;
        Ok(ProfileImportPreview {
            name_clash: name_taken(conn, &file.profile.name)?,
            artifacts: file
                .artifacts
                .iter()
                .map(|a| ImportArtifact {
                    id: a.id.clone(),
                    kind: a.artifact.kind,
                    name: a.artifact.name.clone(),
                    in_library: local.contains_key(&a.id),
                    // The file's own claim about its origin is not shown: it cannot be checked.
                    source: crate::library::source_label(
                        "local",
                        &file_source(&file.profile.name).location,
                    ),
                    file_count: u32::try_from(a.files.len()).unwrap_or(u32::MAX),
                })
                .collect(),
            profile: ImportedProfile {
                name: file.profile.name,
                description: file.profile.description,
                version: file.profile.version,
                spec: file.profile.spec,
            },
        })
    }

    /// Imports the file: the artifacts it carries that the library lacks are stored atomically
    /// with their recorded provenance and a fresh audit, then the profile is created at version
    /// 1 pointing at the library's own ids. `name` overrides the file's name; without it a
    /// name that is taken becomes "<name> (imported)". The file is read again here, so `bytes`
    /// must be the ones previewed.
    pub fn import_profile_file_commit(
        &mut self,
        bytes: &[u8],
        name: Option<&str>,
    ) -> Result<ProfileImportReport> {
        let Parsed {
            file,
            local,
            own,
            map,
        } = parse(self, bytes)?;
        let final_name = match name {
            Some(given) => given.trim().to_owned(),
            None if name_taken(&self.store.conn, &file.profile.name)? => {
                free_name(&self.store.conn, &file.profile.name)?
            }
            None => file.profile.name.clone(),
        };
        super::clean_name(&final_name)?;
        if name_taken(&self.store.conn, &final_name)? {
            return Err(crate::Error::ProfileExists { name: final_name });
        }
        let mut restore: Vec<Restored> = Vec::new();
        let source = file_source(&file.profile.name);
        let now = crate::library::records::now();
        for item in &file.artifacts {
            let Some(id) = own.get(&item.id) else {
                continue;
            };
            if local.contains_key(&item.id) || restore.iter().any(|r| &r.id == id) {
                continue;
            }
            restore.push(Restored {
                id: id.clone(),
                artifact: item.artifact.clone(),
                provenance: Provenance {
                    source: source.clone(),
                    source_path: item.artifact.path.clone(),
                    digest: item.artifact.digest.clone(),
                    imported_at: now,
                    replaced_secret_keys: item.provenance.replaced_secret_keys.clone(),
                },
                files: decode_files(&item.files)?,
                executable: item
                    .files
                    .iter()
                    .filter(|e| e.executable)
                    .map(|e| e.path.clone())
                    .collect(),
            });
        }
        let imported = self.restore_artifacts(restore)?;
        let mut ids = own;
        ids.extend(local);
        let spec = remap(&file.profile.spec, &ids);
        let profile_id = self.create_profile(
            &final_name,
            &file.profile.description,
            &spec,
            file.profile.category.unwrap_or(CategoryId::Software),
            file.profile.category_note.as_deref(),
        )?;
        self.store_map(profile_id, &map)?;
        Ok(ProfileImportReport {
            profile_id,
            name: final_name,
            imported,
            map_draws: map.draw_count(),
            fits: map.fit_count(),
        })
    }
}
