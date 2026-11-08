//! The map document: the JSON a draw stores, and its checks.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{templates, CategoryId, Shape};
use crate::artifact::ArtifactKind;
use crate::library::ArtifactId;

/// A stage's id, unique within a map.
pub type StageId = String;

/// Longest `Placement::note`, in characters.
pub const MAX_NOTE: usize = 60;
/// Longest `Placement::why`, in characters.
pub const MAX_WHY: usize = 400;
/// Most natural next steps kept per placement.
pub const MAX_NEXT: usize = 4;
/// Most "also run by" artifacts kept per placement.
pub const MAX_ALSO_RUN_BY: usize = 3;
/// Longest `role`, `when` or `hands_off`, in characters.
pub const MAX_TOOLTIP: usize = 120;

/// Who made a stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum StageOrigin {
    Template,
    Claude,
    User,
}

/// One stage of the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Stage {
    pub id: StageId,
    pub name: String,
    pub origin: StageOrigin,
}

/// A return path between two stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Loop {
    pub from: StageId,
    pub to: StageId,
    pub label: String,
}

/// Where a placed artifact sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Zone {
    /// On the line, at one stage.
    Stage { id: StageId },
    /// Available at any stage.
    Toolbelt,
    /// Always on.
    Floor,
}

/// Who placed an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum PlacedBy {
    Claude,
    User,
}

/// One artifact on the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Placement {
    pub artifact: ArtifactId,
    pub zone: Zone,
    pub order: u32,
    /// Shown under the name, at most [`MAX_NOTE`] characters.
    pub note: String,
    /// The reasoning for the place, at most [`MAX_WHY`] characters.
    pub why: String,
    /// The heartbeat tooltip, each at most [`MAX_TOOLTIP`] characters.
    pub role: String,
    pub when: String,
    pub hands_off: String,
    /// Claude's reading of the natural next steps after this artifact: other artifacts of the
    /// profile, most likely first, at most [`MAX_NEXT`]. Empty in maps drawn before it existed.
    #[serde(default)]
    #[ts(as = "Option<Vec<ArtifactId>>", optional)]
    pub next: Vec<ArtifactId>,
    /// Claude's reading: the one artifact this runs only through (a skill behind a command, a
    /// subagent behind a skill). A child sits in its parent's zone and moves with it.
    #[serde(default)]
    #[ts(optional)]
    pub parent: Option<ArtifactId>,
    /// Other artifacts that also run this one, at most [`MAX_ALSO_RUN_BY`]; empty without a
    /// parent.
    #[serde(default)]
    #[ts(as = "Option<Vec<ArtifactId>>", optional)]
    pub also_run_by: Vec<ArtifactId>,
    pub by: PlacedBy,
    pub pinned: bool,
}

/// What differs between a drawn map and its template; Harneo computes it after a draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum TemplateChange {
    StageKept {
        id: StageId,
    },
    StageRenamed {
        id: StageId,
        from: String,
        to: String,
    },
    StageAdded {
        id: StageId,
        name: String,
    },
    StageRemoved {
        id: StageId,
        name: String,
    },
    LoopAdded {
        from: StageId,
        to: StageId,
        label: String,
    },
    Unplaced {
        count: u32,
    },
}

/// A profile's map: the JSON in `harnmap_draws.map`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct MapDoc {
    pub category: CategoryId,
    pub shape: Shape,
    pub stages: Vec<Stage>,
    pub loops: Vec<Loop>,
    /// Groups of stages (video's pre-production, production, post-production).
    pub phases: Vec<MapPhase>,
    /// Stage ids drawn as parallel tracks between a fork and a join.
    pub parallel: Vec<StageId>,
    pub placements: Vec<Placement>,
    pub hidden: Vec<ArtifactId>,
    pub hidden_types: Vec<ArtifactKind>,
    pub changes: Vec<TemplateChange>,
}

/// A named group of stages on a map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct MapPhase {
    pub name: String,
    pub stages: Vec<StageId>,
}

/// `text` cut to at most `max` characters, on a character boundary.
pub(super) fn cut(text: &mut String, max: usize) {
    if let Some((at, _)) = text.char_indices().nth(max) {
        text.truncate(at);
    }
}

impl MapDoc {
    /// The empty map of a category: its template's stages and loops, nothing placed. Custom
    /// starts as a line with its one open stage.
    pub fn from_template(category: CategoryId) -> MapDoc {
        let template = templates().into_iter().find(|t| t.id == category);
        let Some(t) = template else {
            return MapDoc::empty(category);
        };
        MapDoc {
            category,
            shape: t.shape.unwrap_or(Shape::Line),
            stages: t
                .stages
                .iter()
                .map(|s| Stage {
                    id: s.id.to_owned(),
                    name: s.name.to_owned(),
                    origin: StageOrigin::Template,
                })
                .collect(),
            loops: t
                .loops
                .iter()
                .map(|l| Loop {
                    from: l.from.to_owned(),
                    to: l.to.to_owned(),
                    label: l.label.to_owned(),
                })
                .collect(),
            phases: t
                .phases
                .iter()
                .map(|p| MapPhase {
                    name: p.name.to_owned(),
                    stages: p.stages.iter().map(|s| (*s).to_owned()).collect(),
                })
                .collect(),
            parallel: t.parallel.iter().map(|s| (*s).to_owned()).collect(),
            placements: Vec::new(),
            hidden: Vec::new(),
            hidden_types: Vec::new(),
            changes: Vec::new(),
        }
    }

    fn empty(category: CategoryId) -> MapDoc {
        MapDoc {
            category,
            shape: Shape::Line,
            stages: Vec::new(),
            loops: Vec::new(),
            phases: Vec::new(),
            parallel: Vec::new(),
            placements: Vec::new(),
            hidden: Vec::new(),
            hidden_types: Vec::new(),
            changes: Vec::new(),
        }
    }

    /// Trims every over-long text to its limit, on character boundaries.
    pub fn cut_to_limits(&mut self) {
        for p in &mut self.placements {
            cut(&mut p.note, MAX_NOTE);
            cut(&mut p.why, MAX_WHY);
            cut(&mut p.role, MAX_TOOLTIP);
            cut(&mut p.when, MAX_TOOLTIP);
            cut(&mut p.hands_off, MAX_TOOLTIP);
            p.next.truncate(MAX_NEXT);
            p.also_run_by.truncate(MAX_ALSO_RUN_BY);
        }
    }

    /// Every reason the map is not sound, as readable lines; empty when it is. `known` is the
    /// profile's artifacts.
    pub fn validate(&self, known: &HashSet<ArtifactId>) -> Vec<String> {
        let mut reasons = Vec::new();
        let mut stage_ids: HashSet<&str> = HashSet::new();
        for s in &self.stages {
            if !stage_ids.insert(&s.id) {
                reasons.push(format!("stage {:?} appears twice", s.id));
            }
        }
        let stage = |id: &str, what: &str, reasons: &mut Vec<String>| {
            if !stage_ids.contains(id) {
                reasons.push(format!("{what} names the unknown stage {id:?}"));
            }
        };
        for l in &self.loops {
            stage(&l.from, "a loop", &mut reasons);
            stage(&l.to, "a loop", &mut reasons);
        }
        for p in &self.phases {
            for id in &p.stages {
                stage(id, &format!("the phase {:?}", p.name), &mut reasons);
            }
        }
        for id in &self.parallel {
            stage(id, "the parallel tracks", &mut reasons);
        }
        let mut placed: HashSet<&str> = HashSet::new();
        for p in &self.placements {
            if !known.contains(&p.artifact) {
                reasons.push(format!(
                    "the artifact {:?} is not in the profile",
                    p.artifact
                ));
            }
            if !placed.insert(&p.artifact) {
                reasons.push(format!("the artifact {:?} is placed twice", p.artifact));
            }
            for n in &p.next {
                if n == &p.artifact || !known.contains(n) {
                    reasons.push(format!(
                        "a next step of {:?} names {:?}, which is not another artifact of the profile",
                        p.artifact, n
                    ));
                }
            }
            if let Zone::Stage { id } = &p.zone {
                stage(
                    id,
                    &format!("the placement of {:?}", p.artifact),
                    &mut reasons,
                );
            }
        }
        reasons.extend(self.family_problems());
        reasons
    }
}
