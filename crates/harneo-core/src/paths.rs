//! Where Harneo keeps its files on disk.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Bundle identifier, also the name of the data folder. Matches the Tauri app id.
const APP_ID: &str = "io.harneo";

/// Harneo's data folder: the library, profiles and backups live here.
/// Same rule as Tauri's `app_data_dir`, so the app and the CLI share one library.
/// `HARNEO_DATA_DIR`, when set and not empty, replaces it (tests and scripts).
pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("HARNEO_DATA_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::data_dir()
        .map(|base| base.join(APP_ID))
        .ok_or(Error::NoDataDir)
}

/// The folder of Harneo's local log files (`harneo-YYYY-MM-DD.log`): `$XDG_STATE_HOME/harneo/logs`,
/// by default `~/.local/state/harneo/logs`; on macOS and Windows the local app-data folder.
/// `HARNEO_LOG_DIR`, when set and not empty, replaces it (tests and scripts).
pub fn log_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("HARNEO_LOG_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .map(|base| base.join("harneo").join("logs"))
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
    fn log_dir_ends_in_harneo_logs() {
        assert!(log_dir().is_ok_and(|dir| dir.ends_with("harneo/logs") || dir.is_absolute()));
    }

    #[test]
    fn data_dir_ends_in_app_id() {
        assert!(data_dir().is_ok_and(|dir| dir.ends_with("io.harneo")));
    }
}
