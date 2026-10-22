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

/// The user's Claude Code home, `~/.claude`: the "your own harness" import source.
pub fn claude_home() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".claude"))
        .ok_or(Error::NoHomeDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_home_is_dot_claude_in_the_home_dir() {
        assert!(claude_home().is_ok_and(|dir| dir.ends_with(".claude")));
    }

    #[test]
    fn data_dir_ends_in_app_id() {
        assert!(data_dir().is_ok_and(|dir| dir.ends_with("io.harneo")));
    }
}
