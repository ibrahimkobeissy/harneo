//! The map as the screen reads it: the current draw with facts and staleness, its history, edits
//! and restore. Draws are rows of `harnmap_draws`; only the newest is ever edited.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::draw::{Basis, BasisArtifact};
use super::map::{MapDoc, Zone};
use super::{templates, CategoryId, Fact, Hook, MapEdit};
use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::library::{records, ArtifactId, Library};
use crate::profiles::{ProfileId, ResolvedItem};

/// Name and kind of an artifact on the map, so nodes can be labelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ArtifactInfo {
    pub name: String,
    pub kind: ArtifactKind,
}

/// A category change since a map was drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct CategoryChange {
    pub from: CategoryId,
    pub to: CategoryId,
}

/// How the profile differs from what the current map was drawn from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Staleness {
    /// In the profile now, not when drawn: they wait in Unplaced.
    pub added: Vec<ArtifactId>,
    /// Drawn, no longer in the profile: their placements are left out of the view.
    pub removed: Vec<ArtifactId>,
    /// In both, with a different digest.
    pub changed: Vec<ArtifactId>,
    pub category_changed: Option<CategoryChange>,
}

impl Staleness {
    fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.category_changed.is_none()
    }
}

/// A profile's current map with what was recorded when it was drawn.
///
/// A profile that was never drawn gets the template skeleton: `drawn` is false, `no` is 0 and
/// the other recorded fields are empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct MapView {
    /// False before the first draw: `doc` is then the category's template.
    pub drawn: bool,
    pub doc: MapDoc,
    pub no: u32,
    /// Unix seconds.
    #[ts(type = "number")]
    pub created_at: i64,
    pub model: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub description: String,
    /// How many draws the profile has.
    pub draw_count: u32,
    /// Facts from the files, by artifact id.
    pub facts: BTreeMap<ArtifactId, Vec<Fact>>,
    /// Groups of artifacts joined by bundle links.
    pub bundles: Vec<Vec<ArtifactId>>,
    /// The profile's artifacts with no place on the map, in profile order.
    pub unplaced: Vec<ArtifactId>,
    /// Name and kind of every artifact of the profile.
    pub artifacts: BTreeMap<ArtifactId, ArtifactInfo>,
    /// The settings template's hooks, for the rail.
    pub hooks: Vec<Hook>,
    /// `None` when the map is up to date (and always before the first draw).
    pub stale: Option<Staleness>,
}

/// One draw in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct DrawSummary {
    pub no: u32,
    #[ts(type = "number")]
    pub created_at: i64,
    pub model: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub description: String,
    /// Computed by comparing the draw with the one before it.
    pub change_line: String,
    pub restored_from: Option<u32>,
}

/// A stored draw.
#[derive(Debug, Clone)]
pub(super) struct DrawRow {
    pub no: u32,
    pub created_at: i64,
    pub model: String,
    pub duration_ms: u64,
    pub description: String,
    pub category: String,
    pub doc: MapDoc,
    pub basis: Basis,
    pub restored_from: Option<u32>,
}

const COLUMNS: &str = "no, created_at, model, duration_ms, description, category, map, basis, \
                       restored_from";

fn row_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<DrawRow> {
    let bad = |e: serde_json::Error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    };
    Ok(DrawRow {
        no: r.get(0)?,
        created_at: r.get(1)?,
        model: r.get(2)?,
        duration_ms: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
        description: r.get(4)?,
        category: r.get(5)?,
        doc: serde_json::from_str(&r.get::<_, String>(6)?).map_err(bad)?,
        basis: serde_json::from_str(&r.get::<_, String>(7)?).map_err(bad)?,
        restored_from: r.get(8)?,
    })
}

fn zone_name(doc: &MapDoc, zone: &Zone) -> String {
    match zone {
        Zone::Stage { id } => doc
            .stages
            .iter()
            .find(|s| s.id == *id)
            .map_or_else(|| id.clone(), |s| s.name.clone()),
        Zone::Toolbelt => "the toolbelt".to_owned(),
        Zone::Floor => "the floor".to_owned(),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// The change between two draws as one line; `name` labels an artifact.
fn change_line(
    prev: &MapDoc,
    doc: &MapDoc,
    category_label: &str,
    name: &dyn Fn(&str) -> String,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let before: HashMap<&str, &Zone> = prev
        .placements
        .iter()
        .map(|p| (p.artifact.as_str(), &p.zone))
        .collect();
    let mut moved: Vec<String> = Vec::new();
    let mut placed = 0;
    for p in &doc.placements {
        match before.get(p.artifact.as_str()) {
            Some(zone) if **zone != p.zone => moved.push(format!(
                "{} moved to {}",
                name(&p.artifact),
                zone_name(doc, &p.zone)
            )),
            Some(_) => {}
            None => placed += 1,
        }
    }
    if moved.len() > 2 {
        parts.push(format!("{} artifacts moved", moved.len()));
    } else {
        parts.extend(moved);
    }
    if placed > 0 {
        parts.push(format!("{placed} placed"));
    }
    let now: HashSet<&str> = doc.placements.iter().map(|p| p.artifact.as_str()).collect();
    let left = prev
        .placements
        .iter()
        .filter(|p| !now.contains(p.artifact.as_str()))
        .count();
    if left > 0 {
        parts.push(format!("{left} left unplaced"));
    }
    if prev.category != doc.category {
        parts.push(format!("category changed to {category_label}"));
    }
    let old_stage = |id: &str| prev.stages.iter().find(|s| s.id == id);
    let added: Vec<&str> = doc
        .stages
        .iter()
        .filter(|s| old_stage(&s.id).is_none())
        .map(|s| s.name.as_str())
        .collect();
    match added.as_slice() {
        [] => {}
        [one] => parts.push(format!("stage {one} added")),
        many => parts.push(format!("{} stages added", many.len())),
    }
    let removed: Vec<&str> = prev
        .stages
        .iter()
        .filter(|s| !doc.stages.iter().any(|d| d.id == s.id))
        .map(|s| s.name.as_str())
        .collect();
    match removed.as_slice() {
        [] => {}
        [one] => parts.push(format!("stage {one} removed")),
        many => parts.push(format!("{} stages removed", many.len())),
    }
    let renamed: Vec<String> = doc
        .stages
        .iter()
        .filter_map(|s| {
            old_stage(&s.id)
                .filter(|o| o.name != s.name)
                .map(|o| format!("{} renamed to {}", o.name, s.name))
        })
        .collect();
    if renamed.len() > 2 {
        parts.push(plural(renamed.len(), "stage renamed", "stages renamed"));
    } else {
        parts.extend(renamed);
    }
    if prev.shape != doc.shape {
        parts.push("shape changed".to_owned());
    }
    if parts.is_empty() {
        "No changes to the map".to_owned()
    } else {
        parts.join(" · ")
    }
}

impl Library {
    /// The newest draw of `profile`, or `None` before the first.
    pub(super) fn latest_draw(&self, profile: ProfileId) -> Result<Option<DrawRow>> {
        Ok(self
            .store
            .conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM harnmap_draws WHERE profile_id = ?1
                     ORDER BY no DESC LIMIT 1"
                ),
                params![profile],
                row_of,
            )
            .optional()?)
    }

    fn draw_row(&self, profile: ProfileId, no: u32) -> Result<DrawRow> {
        self.store
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM harnmap_draws WHERE profile_id = ?1 AND no = ?2"),
                params![profile, no],
                row_of,
            )
            .optional()?
            .ok_or(Error::DrawNotFound { no })
    }

    /// The profile's resolved artifacts now, with what a map drawn from them would record.
    pub(super) fn current_basis(&self, profile: ProfileId) -> Result<(Vec<ResolvedItem>, Basis)> {
        let view = self.profile(profile, None)?;
        let items = self.resolve(&view.spec)?.artifacts;
        let mut artifacts = Vec::new();
        for item in &items {
            let digest = records::load(&self.store.conn, &item.id)?
                .and_then(|d| d.artifact.digest)
                .unwrap_or_default();
            artifacts.push(BasisArtifact {
                id: item.id.clone(),
                digest,
            });
        }
        Ok((
            items,
            Basis {
                profile_version: view.version,
                artifacts,
            },
        ))
    }

    /// The profile's current map: its latest draw, or the category's template skeleton
    /// (`drawn: false`) before the first draw. Placements of artifacts that left the profile
    /// are left out.
    pub fn harnmap(&self, profile: ProfileId) -> Result<MapView> {
        let view = self.profile(profile, None)?;
        let (items, basis) = self.current_basis(profile)?;
        let conn = &self.store.conn;
        let latest = self.latest_draw(profile)?;
        let draw_count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM harnmap_draws WHERE profile_id = ?1",
            params![profile],
            |r| r.get(0),
        )?;
        let known: HashSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let (drawn, mut doc, recorded, stale) = match latest {
            Some(row) => {
                let stale = staleness(&row, &basis, view.category);
                let recorded = (
                    row.no,
                    row.created_at,
                    row.model,
                    row.duration_ms,
                    row.description,
                );
                (true, row.doc, recorded, stale)
            }
            None => (
                false,
                MapDoc::from_template(view.category),
                (0, 0, String::new(), 0, String::new()),
                None,
            ),
        };
        doc.placements
            .retain(|p| known.contains(p.artifact.as_str()));
        let placed: HashSet<&str> = doc.placements.iter().map(|p| p.artifact.as_str()).collect();
        let unplaced = items
            .iter()
            .filter(|i| !placed.contains(i.id.as_str()))
            .map(|i| i.id.clone())
            .collect();
        let (no, created_at, model, duration_ms, description) = recorded;
        Ok(MapView {
            drawn,
            doc,
            no,
            created_at,
            model,
            duration_ms,
            description,
            draw_count,
            facts: super::facts::facts(conn, &items)?,
            bundles: super::facts::bundles(conn, &items)?,
            unplaced,
            artifacts: items
                .iter()
                .map(|i| {
                    (
                        i.id.clone(),
                        ArtifactInfo {
                            name: i.name.clone(),
                            kind: i.kind,
                        },
                    )
                })
                .collect(),
            hooks: super::facts::hooks(conn, &items)?,
            stale,
        })
    }

    /// Applies `edit` to the current draw and saves it at once; older draws never change.
    /// An edit that would leave the map unsound is refused and nothing is saved. Returns the
    /// map as it is now.
    pub fn harnmap_edit(&self, profile: ProfileId, edit: MapEdit) -> Result<MapView> {
        let row = self.latest_draw(profile)?.ok_or(Error::NoMap)?;
        let (items, _) = self.current_basis(profile)?;
        let known: HashSet<ArtifactId> = items.into_iter().map(|i| i.id).collect();
        let mut doc = row.doc;
        doc.apply(edit, &known)?;
        // Placements of artifacts that left the profile stay on record (they come back if the
        // artifact does), so they count as known here.
        let sound: HashSet<ArtifactId> = known
            .iter()
            .cloned()
            .chain(doc.placements.iter().map(|p| p.artifact.clone()))
            .collect();
        let reasons = doc.validate(&sound);
        if !reasons.is_empty() {
            return Err(Error::InvalidMapEdit {
                reason: reasons.join("; "),
            });
        }
        self.store.conn.execute(
            "UPDATE harnmap_draws SET map = ?3 WHERE profile_id = ?1 AND no = ?2",
            params![profile, row.no, serde_json::to_string(&doc)?],
        )?;
        self.harnmap(profile)
    }

    /// Every draw of `profile`, newest first, each with the line of what changed since the
    /// draw before it.
    pub fn harnmap_history(&self, profile: ProfileId) -> Result<Vec<DrawSummary>> {
        let mut stmt = self.store.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM harnmap_draws WHERE profile_id = ?1 ORDER BY no ASC"
        ))?;
        let rows = stmt
            .query_map(params![profile], row_of)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let name = |id: &str| -> String {
            records::load(&self.store.conn, id)
                .ok()
                .flatten()
                .map_or_else(|| id.to_owned(), |d| d.artifact.name)
        };
        let label = |category: CategoryId| -> String {
            templates()
                .into_iter()
                .find(|t| t.id == category)
                .map_or_else(String::new, |t| t.label.to_owned())
        };
        let mut out = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let line = match (i.checked_sub(1).map(|p| &rows[p]), row.restored_from) {
                (_, Some(from)) => format!("Restored from draw {from}"),
                (None, None) => {
                    format!("First drawing on the {} template", label(row.doc.category))
                }
                (Some(prev), None) => {
                    change_line(&prev.doc, &row.doc, &label(row.doc.category), &name)
                }
            };
            out.push(DrawSummary {
                no: row.no,
                created_at: row.created_at,
                model: row.model.clone(),
                duration_ms: row.duration_ms,
                description: row.description.clone(),
                change_line: line,
                restored_from: row.restored_from,
            });
        }
        out.reverse();
        Ok(out)
    }

    /// The map of draw `no`, as it was saved, for a preview.
    pub fn harnmap_draw_at(&self, profile: ProfileId, no: u32) -> Result<MapDoc> {
        Ok(self.draw_row(profile, no)?.doc)
    }

    /// Copies draw `no` as a new draw (the number after the latest) and returns its number.
    /// The copy has the same map, description, model and duration, its basis is the profile
    /// as it is now, and it records which draw it came from.
    pub fn harnmap_restore(&self, profile: ProfileId, no: u32) -> Result<u32> {
        let row = self.draw_row(profile, no)?;
        let (_, basis) = self.current_basis(profile)?;
        let conn = &self.store.conn;
        let tx = conn.unchecked_transaction()?;
        let next: u32 = tx.query_row(
            "SELECT COALESCE(MAX(no), 0) + 1 FROM harnmap_draws WHERE profile_id = ?1",
            params![profile],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms,
                 description, category, map, basis, restored_from)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                profile,
                next,
                records::now(),
                row.model,
                i64::try_from(row.duration_ms).unwrap_or(i64::MAX),
                row.description,
                row.category,
                serde_json::to_string(&row.doc)?,
                serde_json::to_string(&basis)?,
                no,
            ],
        )?;
        tx.commit()?;
        Ok(next)
    }
}

/// How the profile now differs from what `row` was drawn from; `None` when it does not.
fn staleness(row: &DrawRow, now: &Basis, category: CategoryId) -> Option<Staleness> {
    let was: HashMap<&str, &str> = row
        .basis
        .artifacts
        .iter()
        .map(|a| (a.id.as_str(), a.digest.as_str()))
        .collect();
    let current: HashMap<&str, &str> = now
        .artifacts
        .iter()
        .map(|a| (a.id.as_str(), a.digest.as_str()))
        .collect();
    let mut found = Staleness {
        added: now
            .artifacts
            .iter()
            .filter(|a| !was.contains_key(a.id.as_str()))
            .map(|a| a.id.clone())
            .collect(),
        removed: row
            .basis
            .artifacts
            .iter()
            .filter(|a| !current.contains_key(a.id.as_str()))
            .map(|a| a.id.clone())
            .collect(),
        changed: now
            .artifacts
            .iter()
            .filter(|a| was.get(a.id.as_str()).is_some_and(|d| *d != a.digest))
            .map(|a| a.id.clone())
            .collect(),
        category_changed: None,
    };
    if row.doc.category != category {
        found.category_changed = Some(CategoryChange {
            from: row.doc.category,
            to: category,
        });
    }
    (!found.is_empty()).then_some(found)
}
