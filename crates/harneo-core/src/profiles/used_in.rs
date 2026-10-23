//! Which profiles hold an artifact.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{resolve, row, spec_of, ProfileId};
use crate::error::Result;
use crate::library::Library;

/// A profile whose current version installs an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProfileUse {
    pub id: ProfileId,
    pub name: String,
    /// The profile's current version.
    pub version: u32,
}

impl Library {
    /// The profiles (by name) whose current version installs `artifact`: picked, the Settings
    /// template, an MCP server, or pulled in by the bundle of a picked artifact.
    pub fn profiles_using(&self, artifact: &str) -> Result<Vec<ProfileUse>> {
        let conn = &self.store.conn;
        let ids = conn
            .prepare("SELECT id FROM profiles ORDER BY name COLLATE NOCASE")?
            .query_map([], |r| r.get::<_, ProfileId>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::new();
        for id in ids {
            let found = row(conn, id)?;
            let spec = spec_of(conn, id, found.current_version)?;
            if resolve::resolve(conn, &spec)?
                .artifacts
                .iter()
                .any(|i| i.id == artifact)
            {
                out.push(ProfileUse {
                    id,
                    name: found.name,
                    version: found.current_version,
                });
            }
        }
        Ok(out)
    }
}
