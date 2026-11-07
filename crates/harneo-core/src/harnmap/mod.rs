//! Harnmap: a profile seen as a factory. A profile's category picks a [template](templates).

mod draw;
#[cfg(test)]
mod draw_tests;
mod edit;
#[cfg(test)]
mod edit_tests;
mod facts;
#[cfg(test)]
mod facts_tests;
mod fit;
#[cfg(test)]
mod fit_tests;
mod map;
#[cfg(test)]
mod map_tests;
mod portable;
mod templates;
#[cfg(test)]
mod templates_tests;
mod view;
#[cfg(test)]
mod view_tests;

pub use draw::{
    Basis, BasisArtifact, DrawFailure, DrawJob, DrawOutcome, DrawRegistry, DrawResult, DRAW_TIMEOUT,
};
pub use edit::MapEdit;
pub use facts::{Fact, FactKind, Facts, Hook};
pub use fit::{
    ArtifactFit, Candidate, CandidateInfo, FitChanges, FitJob, FitQuote, FitResult, FitSummary,
    FitVerdict, FitView, MissingMcpServer, Verdict, CANDIDATE_CHARS, TOTAL_CHARS,
};
pub use map::{
    Loop, MapDoc, MapPhase, PlacedBy, Placement, Stage, StageId, StageOrigin, TemplateChange, Zone,
    MAX_NOTE, MAX_TOOLTIP, MAX_WHY,
};
pub(crate) use portable::{ArtifactKey, FileFit, FileHarnmap, PreparedMap};
pub use templates::{
    templates, Category, CategoryId, Shape, Template, TemplateLoop, TemplatePhase, TemplateStage,
};
pub use view::{ArtifactInfo, CategoryChange, DrawSummary, MapView, RemovedArtifact, Staleness};
