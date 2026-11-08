//! A profile's map and fits as they travel in a profile file (format 2).
//!
//! Inside the file an artifact is named by a **key** (`kind`, `name`, `digest`), never by a
//! library id: ids differ per machine. Export swaps ids for keys; import swaps keys for the ids
//! the import produced or matched, drops what cannot be mapped, and treats everything as
//! untrusted: the map must parse as a [`MapDoc`], pass its checks after the texts are cut to
//! their limits, and counts are capped. Anything malformed refuses the whole file; nothing is
//! half imported, because the checks run before anything is stored.
//!
//! Fits: a candidate that is a library artifact travels as a key and is dropped on import when
//! no artifact in the file or the library matches it; a candidate from an import preview
//! (`src:...`) travels as its key string and is kept as is. Conflicts and quotes naming an
//! artifact that cannot be mapped are dropped.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::draw::{Basis, MAX_STAGES, MAX_STAGE_NAME};
use super::fit::{FitBasis, FitVerdict};
use super::map::{cut, MapDoc};
use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::library::{records, ArtifactId, Library};
use crate::profiles::{ExportOptions, ProfileId};

/// The most draws a file may carry.
pub const MAX_DRAWS: usize = 500;
/// The most fits a file may carry.
pub const MAX_FITS: usize = 500;

const MAX_DESCRIPTION: usize = 2000;
const MAX_MODEL: usize = 60;
const MAX_TEXT: usize = 2000;
const MAX_LABEL: usize = 200;
const MAX_LOOP_LABEL: usize = 60;

/// What names an artifact inside a profile file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ArtifactKey {
    pub kind: ArtifactKind,
    pub name: String,
    #[serde(default)]
    pub digest: String,
}

/// The `harnmap` part of a profile file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct FileHarnmap {
    pub draws: Vec<FileDraw>,
}

/// One draw as stored in a file; `map` names artifacts by key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileDraw {
    pub no: u32,
    pub created_at: i64,
    pub model: String,
    pub duration_ms: u64,
    pub description: String,
    pub category: String,
    pub map: Value,
    #[serde(default)]
    pub restored_from: Option<u32>,
}

/// A fit's candidate in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum CandidateRef {
    /// A library artifact.
    Key(ArtifactKey),
    /// A candidate of an import preview, its `src:` key as saved.
    Source(String),
}

/// One saved fit as stored in a file; `verdict` names artifacts by key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileFit {
    pub candidate: CandidateRef,
    pub created_at: i64,
    pub model: String,
    pub duration_ms: u64,
    /// The candidate's digest when the fit was made.
    #[serde(default)]
    pub candidate_digest: String,
    /// What a `src:` candidate was read from, as saved.
    #[serde(default)]
    pub source: String,
    pub verdict: Value,
}

/// A draw checked and mapped to this library's ids.
#[derive(Debug, Clone)]
pub(crate) struct PreparedDraw {
    no: u32,
    created_at: i64,
    model: String,
    duration_ms: u64,
    description: String,
    doc: MapDoc,
    restored_from: Option<u32>,
}

/// A fit checked and mapped to this library's ids.
#[derive(Debug, Clone)]
pub(crate) struct PreparedFit {
    key: String,
    verdict: FitVerdict,
    candidate_digest: String,
    source: String,
}

/// The map part of a file, ready to store once the profile exists.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedMap {
    draws: Vec<PreparedDraw>,
    fits: Vec<PreparedFit>,
}

impl PreparedMap {
    /// How many draws will be stored.
    pub fn draw_count(&self) -> u32 {
        u32::try_from(self.draws.len()).unwrap_or(u32::MAX)
    }

    /// How many fits will be stored.
    pub fn fit_count(&self) -> u32 {
        u32::try_from(self.fits.len()).unwrap_or(u32::MAX)
    }
}

fn malformed(reason: impl Display) -> Error {
    Error::InvalidProfile {
        reason: format!("The file's map part is malformed: {reason}"),
    }
}

/// Swaps every artifact reference of a map document with `swap` (id to key on export, key to id
/// on import); a reference `swap` answers `None` for is dropped.
fn rewrite_map(doc: &mut Value, swap: &dyn Fn(&Value) -> Result<Option<Value>>) -> Result<()> {
    if let Some(placements) = doc.get_mut("placements").and_then(Value::as_array_mut) {
        let mut kept = Vec::new();
        for mut placement in std::mem::take(placements) {
            let Some(reference) = placement.get("artifact") else {
                return Err(malformed("a placement names no artifact"));
            };
            if let Some(swapped) = swap(reference)? {
                placement["artifact"] = swapped;
                if let Some(next) = placement.get_mut("next").and_then(Value::as_array_mut) {
                    let mut steps = Vec::new();
                    for step in std::mem::take(next) {
                        if let Some(swapped) = swap(&step)? {
                            steps.push(swapped);
                        }
                    }
                    *next = steps;
                }
                if let Some(reference) = placement.get("parent").filter(|v| !v.is_null()) {
                    match swap(reference)? {
                        Some(swapped) => placement["parent"] = swapped,
                        None => placement["parent"] = Value::Null,
                    }
                }
                if let Some(runners) = placement.get_mut("alsoRunBy").and_then(Value::as_array_mut)
                {
                    let mut kept_runners = Vec::new();
                    for runner in std::mem::take(runners) {
                        if let Some(swapped) = swap(&runner)? {
                            kept_runners.push(swapped);
                        }
                    }
                    *runners = kept_runners;
                }
                kept.push(placement);
            }
        }
        *placements = kept;
    }
    if let Some(hidden) = doc.get_mut("hidden").and_then(Value::as_array_mut) {
        let mut kept = Vec::new();
        for reference in std::mem::take(hidden) {
            if let Some(swapped) = swap(&reference)? {
                kept.push(swapped);
            }
        }
        *hidden = kept;
    }
    Ok(())
}

fn as_key(value: &Value) -> Result<ArtifactKey> {
    serde_json::from_value(value.clone()).map_err(|_| malformed("an artifact key is not valid"))
}

fn id_of(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| malformed("an artifact reference is not an id"))
}

impl Library {
    /// The map part and the fits of `profile` for a file. `keys` names the artifacts the file
    /// carries; references to any other artifact are left out.
    pub(crate) fn export_map(
        &self,
        profile: ProfileId,
        keys: &HashMap<ArtifactId, ArtifactKey>,
        options: ExportOptions,
    ) -> Result<(Option<FileHarnmap>, Vec<FileFit>)> {
        let key_of = |reference: &Value| -> Result<Option<Value>> {
            keys.get(id_of(reference)?)
                .map(|k| serde_json::to_value(k).map_err(Error::from))
                .transpose()
        };
        let conn = &self.store.conn;
        let mut stmt = conn.prepare(
            "SELECT no, created_at, model, duration_ms, description, category, map, restored_from
             FROM harnmap_draws WHERE profile_id = ?1 ORDER BY no",
        )?;
        let rows = stmt
            .query_map(params![profile], |r| {
                Ok((
                    FileDraw {
                        no: r.get(0)?,
                        created_at: r.get(1)?,
                        model: r.get(2)?,
                        duration_ms: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                        description: r.get(4)?,
                        category: r.get(5)?,
                        map: Value::Null,
                        restored_from: r.get(7)?,
                    },
                    r.get::<_, String>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let newest = rows.len().saturating_sub(1);
        let mut draws = Vec::new();
        for (i, (mut draw, map)) in rows.into_iter().enumerate() {
            if !options.include_history && i != newest {
                continue;
            }
            draw.map = serde_json::from_str(&map)?;
            rewrite_map(&mut draw.map, &key_of)?;
            draws.push(draw);
        }
        let harnmap = (!draws.is_empty()).then_some(FileHarnmap { draws });

        let mut fits = Vec::new();
        if options.include_fits {
            let mut stmt = conn.prepare(
                "SELECT verdict, basis FROM harnfit_fits WHERE profile_id = ?1
                 ORDER BY candidate",
            )?;
            let rows = stmt
                .query_map(params![profile], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (verdict, basis) in rows {
                let verdict: FitVerdict = serde_json::from_str(&verdict)?;
                let basis: FitBasis = serde_json::from_str(&basis)?;
                if let Some(fit) = self.fit_to_file(&verdict, &basis, keys)? {
                    fits.push(fit);
                }
            }
        }
        Ok((harnmap, fits))
    }

    fn fit_to_file(
        &self,
        verdict: &FitVerdict,
        basis: &FitBasis,
        keys: &HashMap<ArtifactId, ArtifactKey>,
    ) -> Result<Option<FileFit>> {
        let candidate = match verdict.candidate.key.strip_prefix("lib:") {
            Some(id) => {
                let Some(found) = records::load(&self.store.conn, id)? else {
                    return Ok(None);
                };
                CandidateRef::Key(ArtifactKey {
                    kind: found.artifact.kind,
                    name: found.artifact.name,
                    digest: found.artifact.digest.unwrap_or_default(),
                })
            }
            None => CandidateRef::Source(verdict.candidate.key.clone()),
        };
        let mut body = serde_json::to_value(verdict)?;
        if let Some(list) = body.get_mut("conflictsWith").and_then(Value::as_array_mut) {
            let mut kept = Vec::new();
            for reference in std::mem::take(list) {
                if let Some(k) = keys.get(id_of(&reference)?) {
                    kept.push(serde_json::to_value(k)?);
                }
            }
            *list = kept;
        }
        if let Some(list) = body.get_mut("quotes").and_then(Value::as_array_mut) {
            let mut kept = Vec::new();
            for mut quote in std::mem::take(list) {
                let id = quote
                    .get("artifact")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if id == "candidate" {
                    kept.push(quote);
                } else if let Some(k) = keys.get(&id) {
                    quote["artifact"] = serde_json::to_value(k)?;
                    kept.push(quote);
                }
            }
            *list = kept;
        }
        if matches!(candidate, CandidateRef::Key(_)) {
            body["candidate"]["key"] = Value::String("lib:".to_owned());
        }
        Ok(Some(FileFit {
            candidate,
            created_at: verdict.created_at,
            model: verdict.model.clone(),
            duration_ms: verdict.duration_ms,
            candidate_digest: basis.candidate_digest.clone(),
            source: basis.source.clone(),
            verdict: body,
        }))
    }

    /// The library artifact of `key` by kind and digest, for a fit candidate the file does not
    /// carry.
    fn library_match(&self, key: &ArtifactKey) -> Result<Option<ArtifactId>> {
        if key.digest.is_empty() {
            return Ok(None);
        }
        let kind = serde_json::to_value(key.kind)?;
        Ok(rusqlite::OptionalExtension::optional(
            self.store.conn.query_row(
                "SELECT id FROM artifacts WHERE digest = ?1 AND kind = ?2 ORDER BY id LIMIT 1",
                params![key.digest, kind.as_str().unwrap_or_default()],
                |r| r.get::<_, String>(0),
            ),
        )?)
    }

    /// Checks the map part of a file and maps it to library ids. `resolver` gives the id each
    /// artifact the file carries gets here. Reads the library, changes nothing.
    pub(crate) fn prepare_map(
        &self,
        part: Option<&FileHarnmap>,
        fits: &[FileFit],
        resolver: &HashMap<ArtifactKey, ArtifactId>,
    ) -> Result<PreparedMap> {
        let known: HashSet<ArtifactId> = resolver.values().cloned().collect();
        let id_of_key = |reference: &Value| -> Result<Option<Value>> {
            Ok(resolver
                .get(&as_key(reference)?)
                .map(|id| Value::String(id.clone())))
        };
        let mut prepared = PreparedMap::default();
        let drafts = part.map_or(&[][..], |p| p.draws.as_slice());
        if drafts.len() > MAX_DRAWS {
            return Err(malformed(format!("more than {MAX_DRAWS} drawings")));
        }
        let mut numbers = HashSet::new();
        for draw in drafts {
            if draw.no == 0 || !numbers.insert(draw.no) {
                return Err(malformed("a drawing number is zero or repeated"));
            }
            let mut value = draw.map.clone();
            rewrite_map(&mut value, &id_of_key)?;
            let mut doc: MapDoc = serde_json::from_value(value).map_err(malformed)?;
            doc.cut_to_limits();
            if doc.stages.len() > MAX_STAGES {
                return Err(malformed(format!("more than {MAX_STAGES} stages")));
            }
            for stage in &mut doc.stages {
                cut(&mut stage.name, MAX_STAGE_NAME);
            }
            for l in &mut doc.loops {
                cut(&mut l.label, MAX_LOOP_LABEL);
            }
            let mut seen = HashSet::new();
            doc.placements.retain(|p| seen.insert(p.artifact.clone()));
            let mut seen = HashSet::new();
            doc.hidden.retain(|h| seen.insert(h.clone()));
            let reasons = doc.validate(&known);
            if !reasons.is_empty() {
                return Err(malformed(reasons.join("; ")));
            }
            let mut description = draw.description.clone();
            cut(&mut description, MAX_DESCRIPTION);
            let mut model = draw.model.clone();
            cut(&mut model, MAX_MODEL);
            prepared.draws.push(PreparedDraw {
                no: draw.no,
                created_at: draw.created_at,
                model,
                duration_ms: draw.duration_ms,
                description,
                doc,
                restored_from: draw.restored_from,
            });
        }
        prepared.draws.sort_by_key(|d| d.no);
        let kept: HashSet<u32> = prepared.draws.iter().map(|d| d.no).collect();
        for draw in &mut prepared.draws {
            draw.restored_from = draw.restored_from.filter(|n| kept.contains(n));
        }

        if fits.len() > MAX_FITS {
            return Err(malformed(format!("more than {MAX_FITS} saved fits")));
        }
        let mut keys = HashSet::new();
        for fit in fits {
            if let Some(mapped) = self.prepare_fit_entry(fit, resolver)? {
                if keys.insert(mapped.key.clone()) {
                    prepared.fits.push(mapped);
                }
            }
        }
        Ok(prepared)
    }

    fn prepare_fit_entry(
        &self,
        fit: &FileFit,
        resolver: &HashMap<ArtifactKey, ArtifactId>,
    ) -> Result<Option<PreparedFit>> {
        let key = match &fit.candidate {
            CandidateRef::Key(k) => {
                let found = match resolver.get(k) {
                    Some(id) => Some(id.clone()),
                    None => self.library_match(k)?,
                };
                match found {
                    Some(id) => format!("lib:{id}"),
                    None => return Ok(None),
                }
            }
            CandidateRef::Source(s) if s.starts_with("src:") && s.chars().count() <= 1000 => {
                s.clone()
            }
            CandidateRef::Source(_) => return Err(malformed("a saved fit has a bad candidate")),
        };
        let mut body = fit.verdict.clone();
        let mut ids_of = |field: &str, name: &dyn Fn(&mut Value) -> Option<Value>| {
            if let Some(list) = body.get_mut(field).and_then(Value::as_array_mut) {
                let mut kept = Vec::new();
                for mut item in std::mem::take(list) {
                    if let Some(mapped) = name(&mut item) {
                        kept.push(mapped);
                    }
                }
                *list = kept;
            }
        };
        let lookup = |value: &Value| -> Option<String> {
            let key: ArtifactKey = serde_json::from_value(value.clone()).ok()?;
            resolver.get(&key).cloned()
        };
        ids_of("conflictsWith", &|item| lookup(item).map(Value::String));
        ids_of("quotes", &|quote| {
            let reference = quote.get("artifact")?.clone();
            if reference.as_str() != Some("candidate") {
                quote["artifact"] = Value::String(lookup(&reference)?);
            }
            Some(std::mem::take(quote))
        });
        if !body.get("candidate").is_some_and(Value::is_object) {
            return Err(malformed("a saved fit has no candidate"));
        }
        body["candidate"]["key"] = Value::String(key.clone());
        let mut verdict: FitVerdict = serde_json::from_value(body).map_err(malformed)?;
        verdict.created_at = fit.created_at;
        verdict.duration_ms = fit.duration_ms;
        verdict.model.clone_from(&fit.model);
        cut(&mut verdict.model, MAX_MODEL);
        cut(&mut verdict.why, MAX_TEXT);
        if let Some(change) = &mut verdict.change {
            cut(change, MAX_TEXT);
        }
        cut(&mut verdict.candidate.label, MAX_LABEL);
        cut(&mut verdict.candidate.source, MAX_LABEL);
        for quote in &mut verdict.quotes {
            cut(&mut quote.text, MAX_TEXT);
            cut(&mut quote.file, MAX_LABEL);
        }
        let mut source = fit.source.clone();
        cut(&mut source, 1000);
        Ok(Some(PreparedFit {
            key,
            verdict,
            candidate_digest: fit.candidate_digest.clone(),
            source,
        }))
    }

    /// Stores a prepared map part for `profile`. Its basis is the profile as it is now, so a
    /// freshly imported map is not out of date.
    pub(crate) fn store_map(&self, profile: ProfileId, map: &PreparedMap) -> Result<()> {
        if map.draws.is_empty() && map.fits.is_empty() {
            return Ok(());
        }
        let (_, basis) = self.current_basis(profile)?;
        let tx = self.store.conn.unchecked_transaction()?;
        for draw in &map.draws {
            tx.execute(
                "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms,
                     description, category, map, basis, restored_from)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    profile,
                    draw.no,
                    draw.created_at,
                    draw.model,
                    i64::try_from(draw.duration_ms).unwrap_or(i64::MAX),
                    draw.description,
                    draw.doc.category.as_str(),
                    serde_json::to_string(&draw.doc)?,
                    serde_json::to_string(&basis)?,
                    draw.restored_from,
                ],
            )?;
        }
        for fit in &map.fits {
            tx.execute(
                "INSERT INTO harnfit_fits
                     (profile_id, candidate, created_at, model, duration_ms, verdict, basis)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    profile,
                    fit.key,
                    fit.verdict.created_at,
                    fit.verdict.model,
                    i64::try_from(fit.verdict.duration_ms).unwrap_or(i64::MAX),
                    serde_json::to_string(&fit.verdict)?,
                    serde_json::to_string(&fit_basis(&basis, fit))?,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn fit_basis(profile: &Basis, fit: &PreparedFit) -> FitBasis {
    FitBasis {
        profile: profile.clone(),
        candidate_digest: fit.candidate_digest.clone(),
        source: fit.source.clone(),
    }
}
