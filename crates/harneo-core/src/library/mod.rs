//! The library: the user's imported artifacts, kept in the data folder as file copies plus a
//! SQLite index.
//!
//! ```text
//! <data dir>/library/library.db        the index (see `store`)
//! <data dir>/library/artifacts/<id>/   one copy per artifact (see `ids` for the id)
//! ```

pub mod ids;
mod store;

use std::path::Path;

use crate::error::Result;
use store::Store;

/// An open library. Opening creates the folders and the schema when they are missing.
#[derive(Debug)]
pub struct Library {
    store: Store,
}

impl Library {
    /// Opens (creating it if needed) the library under `data_dir` and migrates its schema
    /// forward to the current version.
    pub fn open(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            store: Store::open(data_dir)?,
        })
    }

    /// The schema version stored in the database (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<u32> {
        self.store.schema_version()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_reports_the_current_schema_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let library = Library::open(dir.path()).expect("open");
        assert_eq!(
            library.schema_version().expect("version"),
            store::SCHEMA_VERSION
        );
    }
}
