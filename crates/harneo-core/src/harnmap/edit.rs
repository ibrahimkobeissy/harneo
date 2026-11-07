//! The user's edits of the current map. Each one is checked against the whole map before it is
//! saved; an edit that would leave the map unsound is refused and changes nothing.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::draw::{slug, MAX_STAGES, MAX_STAGE_NAME};
use super::map::{cut, MapDoc, PlacedBy, Stage, StageId, StageOrigin, Zone, MAX_NOTE};
use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::library::ArtifactId;

/// One change the user makes to the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum MapEdit {
    /// Moves a placed artifact to `zone` at position `order`, and pins it.
    #[serde(rename_all = "camelCase")]
    Move {
        artifact: ArtifactId,
        zone: Zone,
        order: u32,
    },
    /// Places an unplaced artifact, pinned, with a note.
    #[serde(rename_all = "camelCase")]
    Place {
        artifact: ArtifactId,
        zone: Zone,
        note: String,
    },
    /// Takes an artifact off the map; it goes back to Unplaced.
    #[serde(rename_all = "camelCase")]
    Remove { artifact: ArtifactId },
    /// Lets a re-draw move the artifact again.
    #[serde(rename_all = "camelCase")]
    Unpin { artifact: ArtifactId },
    /// Sets the note shown under the artifact's name.
    #[serde(rename_all = "camelCase")]
    Note { artifact: ArtifactId, text: String },
    /// Hides an artifact on the map.
    #[serde(rename_all = "camelCase")]
    Hide { artifact: ArtifactId },
    /// Shows a hidden artifact again.
    #[serde(rename_all = "camelCase")]
    Show { artifact: ArtifactId },
    /// Hides every artifact of a type.
    #[serde(rename_all = "camelCase")]
    HideType { kind: ArtifactKind },
    /// Shows a hidden type again.
    #[serde(rename_all = "camelCase")]
    ShowType { kind: ArtifactKind },
    /// Renames a stage; it becomes the user's.
    #[serde(rename_all = "camelCase")]
    RenameStage { id: StageId, name: String },
    /// Adds a stage after `after`, or at the end when `after` is empty.
    #[serde(rename_all = "camelCase")]
    AddStage {
        after: Option<StageId>,
        name: String,
    },
    /// Removes a stage: its artifacts go back to Unplaced (unpinned) and its loops go.
    #[serde(rename_all = "camelCase")]
    RemoveStage { id: StageId },
    /// Moves a stage to position `to` (counted from 0).
    #[serde(rename_all = "camelCase")]
    MoveStage { id: StageId, to: u32 },
}

fn refuse(reason: impl Into<String>) -> Error {
    Error::InvalidMapEdit {
        reason: reason.into(),
    }
}

fn zone_key(zone: &Zone) -> String {
    match zone {
        Zone::Stage { id } => format!("stage:{id}"),
        Zone::Toolbelt => "toolbelt".to_owned(),
        Zone::Floor => "floor".to_owned(),
    }
}

impl MapDoc {
    /// Takes `artifact` off the map and out of the hidden list; returns how many placements went.
    pub(crate) fn drop_artifact(&mut self, artifact: &str) -> usize {
        let before = self.placements.len();
        self.placements.retain(|p| p.artifact != artifact);
        self.hidden.retain(|a| a != artifact);
        self.renumber(None);
        before - self.placements.len()
    }

    /// Position of the placement of `artifact`.
    fn placed_at(&self, artifact: &str) -> Result<usize> {
        self.placements
            .iter()
            .position(|p| p.artifact == artifact)
            .ok_or_else(|| refuse(format!("{artifact} is not on the map")))
    }

    fn stage_at(&self, id: &str) -> Result<usize> {
        self.stages
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| refuse(format!("the stage {id:?} does not exist")))
    }

    /// Numbers the artifacts of every zone 0, 1, 2... in their current order; `first` wins a tie.
    fn renumber(&mut self, first: Option<&str>) {
        let mut rows: Vec<(String, u32, bool, usize)> = self
            .placements
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    zone_key(&p.zone),
                    p.order,
                    Some(p.artifact.as_str()) != first,
                    i,
                )
            })
            .collect();
        rows.sort();
        let mut next: HashMap<String, u32> = HashMap::new();
        for (key, _, _, i) in rows {
            let n = next.entry(key).or_insert(0);
            self.placements[i].order = *n;
            *n += 1;
        }
    }

    /// Applies `edit`. `known` is the profile's artifacts; `Place` takes one of those.
    /// Nothing is checked beyond the edit itself: callers [validate](MapDoc::validate) after.
    pub fn apply(&mut self, edit: MapEdit, known: &HashSet<ArtifactId>) -> Result<()> {
        match edit {
            MapEdit::Move {
                artifact,
                zone,
                order,
            } => {
                let at = self.placed_at(&artifact)?;
                let p = &mut self.placements[at];
                if p.zone != zone {
                    p.why = "You moved it here.".to_owned();
                }
                p.zone = zone;
                p.order = order;
                p.by = PlacedBy::User;
                p.pinned = true;
                self.renumber(Some(&artifact));
            }
            MapEdit::Place {
                artifact,
                zone,
                note,
            } => {
                if !known.contains(&artifact) {
                    return Err(refuse(format!("{artifact} is not in the profile")));
                }
                if self.placements.iter().any(|p| p.artifact == artifact) {
                    return Err(refuse(format!("{artifact} is already on the map")));
                }
                let order =
                    u32::try_from(self.placements.iter().filter(|p| p.zone == zone).count())
                        .unwrap_or(u32::MAX);
                let mut note = note.trim().to_owned();
                cut(&mut note, MAX_NOTE);
                self.placements.push(super::map::Placement {
                    artifact,
                    zone,
                    order,
                    note,
                    why: "You placed it here.".to_owned(),
                    role: String::new(),
                    when: String::new(),
                    hands_off: String::new(),
                    by: PlacedBy::User,
                    pinned: true,
                });
            }
            MapEdit::Remove { artifact } => {
                let at = self.placed_at(&artifact)?;
                self.placements.remove(at);
                self.renumber(None);
            }
            MapEdit::Unpin { artifact } => {
                let at = self.placed_at(&artifact)?;
                self.placements[at].pinned = false;
            }
            MapEdit::Note { artifact, text } => {
                let at = self.placed_at(&artifact)?;
                let mut text = text.trim().to_owned();
                cut(&mut text, MAX_NOTE);
                self.placements[at].note = text;
            }
            MapEdit::Hide { artifact } => {
                if !self.hidden.contains(&artifact) {
                    self.hidden.push(artifact);
                }
            }
            MapEdit::Show { artifact } => self.hidden.retain(|a| *a != artifact),
            MapEdit::HideType { kind } => {
                if !self.hidden_types.contains(&kind) {
                    self.hidden_types.push(kind);
                }
            }
            MapEdit::ShowType { kind } => self.hidden_types.retain(|k| *k != kind),
            MapEdit::RenameStage { id, name } => {
                let at = self.stage_at(&id)?;
                let name = stage_name(&name)?;
                self.stages[at].name = name;
                self.stages[at].origin = StageOrigin::User;
            }
            MapEdit::AddStage { after, name } => {
                let name = stage_name(&name)?;
                if self.stages.len() >= MAX_STAGES {
                    return Err(refuse(format!("a map holds at most {MAX_STAGES} stages")));
                }
                let at = match after {
                    Some(id) => self.stage_at(&id)? + 1,
                    None => self.stages.len(),
                };
                let taken: HashSet<String> = self.stages.iter().map(|s| s.id.clone()).collect();
                let id = slug(&name, &taken);
                self.stages.insert(
                    at,
                    Stage {
                        id,
                        name,
                        origin: StageOrigin::User,
                    },
                );
            }
            MapEdit::RemoveStage { id } => {
                self.stage_at(&id)?;
                if self.stages.len() == 1 {
                    return Err(refuse("a map keeps at least one stage"));
                }
                self.stages.retain(|s| s.id != id);
                let here = Zone::Stage { id: id.clone() };
                self.placements.retain(|p| p.zone != here);
                self.loops.retain(|l| l.from != id && l.to != id);
                for phase in &mut self.phases {
                    phase.stages.retain(|s| *s != id);
                }
                self.phases.retain(|p| !p.stages.is_empty());
                self.parallel.retain(|s| *s != id);
                self.renumber(None);
            }
            MapEdit::MoveStage { id, to } => {
                let at = self.stage_at(&id)?;
                let stage = self.stages.remove(at);
                let to = usize::try_from(to)
                    .unwrap_or(usize::MAX)
                    .min(self.stages.len());
                self.stages.insert(to, stage);
            }
        }
        Ok(())
    }
}

fn stage_name(name: &str) -> Result<String> {
    let mut name = name.trim().to_owned();
    if name.is_empty() {
        return Err(refuse("a stage needs a name"));
    }
    cut(&mut name, MAX_STAGE_NAME);
    Ok(name)
}
