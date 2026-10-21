//! Thin IPC commands: convert arguments and results, never hold logic (that is `harneo-core`).

use serde::Serialize;
use ts_rs::TS;

/// What the About panel shows. `data_dir` crosses IPC as a string, since a path is not JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
}

/// Same version and data dir the `harneo` CLI reports, so both front ends agree.
#[tauri::command]
pub fn app_info() -> Result<AppInfo, String> {
    let data_dir = harneo_core::paths::data_dir().map_err(|e| e.to_string())?;
    Ok(AppInfo {
        version: harneo_core::version().into(),
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_matches_core() -> Result<(), String> {
        let info = app_info()?;
        assert_eq!(info.version, harneo_core::version());
        let expected = harneo_core::paths::data_dir().map_err(|e| e.to_string())?;
        assert_eq!(info.data_dir, expected.to_string_lossy());
        Ok(())
    }

    #[test]
    fn app_info_serializes_camel_case() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(AppInfo {
            version: "1.0.0".into(),
            data_dir: "/x".into(),
        })?;
        assert_eq!(json["dataDir"], "/x");
        assert_eq!(json["version"], "1.0.0");
        Ok(())
    }
}
