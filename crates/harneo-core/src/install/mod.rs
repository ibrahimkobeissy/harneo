//! Installing a profile into a project. This module holds the content side: [`Library::render`]
//! turns a profile into the exact bytes of every file it writes. Applying them to disk, backups
//! and the registry come with later tickets.

mod render;
#[cfg(test)]
mod render_tests;

pub use render::{Rendered, RenderedFile};
