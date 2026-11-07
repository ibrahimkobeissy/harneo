//! Harnmap: a profile seen as a factory. A profile's category picks a [template](templates).

mod draw;
#[cfg(test)]
mod draw_tests;
mod facts;
#[cfg(test)]
mod facts_tests;
mod map;
#[cfg(test)]
mod map_tests;
mod templates;
#[cfg(test)]
mod templates_tests;

pub use draw::{
    Basis, BasisArtifact, DrawFailure, DrawJob, DrawOutcome, DrawRegistry, DrawResult, MapView,
    DRAW_TIMEOUT,
};
pub use facts::{Fact, FactKind, Facts};
pub use map::{
    Loop, MapDoc, MapPhase, PlacedBy, Placement, Stage, StageId, StageOrigin, TemplateChange, Zone,
    MAX_NOTE, MAX_TOOLTIP, MAX_WHY,
};
pub use templates::{
    templates, Category, CategoryId, Shape, Template, TemplateLoop, TemplatePhase, TemplateStage,
};
