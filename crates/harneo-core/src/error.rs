//! The error type shared by every core module, so the app and the CLI report
//! failures the same way.

/// Everything that can go wrong in `harneo-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The OS gave no per-user data location, so there is nowhere to keep the library.
    #[error("cannot determine the user data directory on this system")]
    NoDataDir,
    /// The folder to scan is missing or is not a folder.
    #[error("cannot scan {path}: not a folder")]
    NotAFolder {
        /// The path as the caller gave it.
        path: String,
    },
    /// A read failed on a folder that was being scanned.
    #[error("cannot read {path}: {source}")]
    Io {
        /// What was being read, relative to the scan root.
        path: String,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// The library index could not be opened, migrated or queried.
    #[error("library database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// The library was written by a newer Harneo than this one.
    #[error(
        "the library uses schema version {found}, but this Harneo only understands up to \
         {supported}; update Harneo to open it"
    )]
    SchemaTooNew {
        /// The version stored in the database.
        found: u32,
        /// The newest version this build can read.
        supported: u32,
    },
}

/// Result alias used across `harneo-core`.
pub type Result<T> = std::result::Result<T, Error>;
