//! Installing a profile into a project. `render` turns a profile into the exact bytes of every
//! file it writes; `plan` says what an install will do; `apply` does it with backups and a
//! rollback; `registry` remembers the projects, their manifests and the backup sets.

mod apply;
#[cfg(test)]
mod apply_tests;
mod backup;
#[cfg(test)]
mod backup_tests;
mod compare;
#[cfg(test)]
mod compare_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod fixture_lifecycle;
mod plan;
#[cfg(test)]
mod plan_tests;
mod registry;
#[cfg(test)]
mod registry_tests;
mod removal;
#[cfg(test)]
mod removal_tests;
mod render;
#[cfg(test)]
mod render_tests;
mod switch;
#[cfg(test)]
mod switch_tests;

pub use apply::ApplyReport;
pub use compare::{Comparison, InProfile, InProject, Loss, LossUnit, RefitAction, Row, RowKind};
pub(crate) use backup::write_atomic;
pub(crate) use plan::{digest, project_folder, resolve_inside, safe_parts, utc_date};
pub use plan::{Plan, PlanKind, Write, WriteAction};
pub(crate) use registry::{project_count, project_names};
pub use registry::{Backup, InstalledFile, Project, ProjectFile, MAX_FILE_BYTES};
pub(crate) use render::with_harness_import;
pub use render::{Rendered, RenderedFile};
