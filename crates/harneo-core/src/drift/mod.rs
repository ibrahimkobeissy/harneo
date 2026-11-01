//! Drift: keeping registered projects honest. How a project's files differ from what Harneo
//! wrote and the two ways out (re-apply or promote), plus the live rescan watcher.

pub mod check;
#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod fixture;
pub mod promote;
#[cfg(test)]
mod promote_settings_tests;
#[cfg(test)]
mod promote_tests;
pub mod watch;
#[cfg(test)]
mod watch_tests;

pub use check::{DriftChange, DriftFile, DriftReport, DriftStatus, DriftSummary, ProjectDrift};
pub use promote::{AddedPreview, PromoteReport};
