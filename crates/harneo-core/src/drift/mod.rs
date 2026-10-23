//! Drift: keeping registered projects honest. The live rescan watcher is here; the drift check
//! itself lands with its own ticket.

pub mod watch;

#[cfg(test)]
mod watch_tests;
//! Drift: how a project's files differ from what Harneo wrote, and the two ways out.

pub mod check;
#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod fixture;
pub mod promote;
#[cfg(test)]
mod promote_tests;

pub use check::{DriftChange, DriftFile, DriftReport, DriftStatus, DriftSummary, ProjectDrift};
pub use promote::{AddedPreview, PromoteReport};
