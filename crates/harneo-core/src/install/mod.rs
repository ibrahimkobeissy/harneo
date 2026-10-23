//! Installing a profile into a project. `render` turns a profile into the exact bytes of every
//! file it writes; `plan` says what an install will do; `apply` does it with backups and a
//! rollback; `registry` remembers the projects, their manifests and the backup sets.

mod apply;
#[cfg(test)]
mod apply_tests;
mod backup;
#[cfg(test)]
mod fixture;
mod plan;
#[cfg(test)]
mod plan_tests;
mod registry;
#[cfg(test)]
mod registry_tests;
mod render;
#[cfg(test)]
mod render_tests;

pub use apply::ApplyReport;
pub use plan::{Plan, Write, WriteAction};
pub(crate) use registry::{project_count, project_names};
pub use registry::{Backup, InstalledFile, Project, ProjectFile, MAX_FILE_BYTES};
pub use render::{Rendered, RenderedFile};
