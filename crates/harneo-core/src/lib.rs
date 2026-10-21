//! Harneo's engine. Every piece of logic lives here so the desktop app and the
//! `harneo` CLI run the same code; they only convert arguments and results.

pub mod artifact;
pub mod error;
pub mod paths;

pub use error::{Error, Result};

/// The Harneo version, so the app's About panel and `harneo --version` agree.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
