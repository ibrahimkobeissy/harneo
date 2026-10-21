//! The error type shared by every core module, so the app and the CLI report
//! failures the same way.

/// Everything that can go wrong in `harneo-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The OS gave no per-user data location, so there is nowhere to keep the library.
    #[error("cannot determine the user data directory on this system")]
    NoDataDir,
}

/// Result alias used across `harneo-core`.
pub type Result<T> = std::result::Result<T, Error>;
