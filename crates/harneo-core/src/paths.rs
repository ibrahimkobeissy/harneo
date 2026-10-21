//! Where Harneo keeps its files on disk.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Bundle identifier, also the name of the data folder. Matches the Tauri app id.
const APP_ID: &str = "io.harneo";

/// Harneo's data folder: the library, profiles and backups live here.
/// Same rule as Tauri's `app_data_dir`, so the app and the CLI share one library.
pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|base| base.join(APP_ID))
        .ok_or(Error::NoDataDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_in_app_id() {
        assert!(data_dir().is_ok_and(|dir| dir.ends_with("io.harneo")));
    }
}
