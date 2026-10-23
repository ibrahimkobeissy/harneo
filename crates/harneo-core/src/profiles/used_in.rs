//! Which profiles hold an artifact.

use std::collections::BTreeMap;

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
    /// For every artifact some profile's current version installs, those profiles (by name).
    /// One pass over the profiles, where [`Library::profiles_using`] resolves them per call.
    pub(crate) fn profile_membership(&self) -> Result<BTreeMap<String, Vec<ProfileUse>>> {
        let conn = &self.store.conn;
        let ids = conn
            .prepare("SELECT id FROM profiles ORDER BY name COLLATE NOCASE")?
            .query_map([], |r| r.get::<_, ProfileId>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out: BTreeMap<String, Vec<ProfileUse>> = BTreeMap::new();
        for id in ids {
            let found = row(conn, id)?;
            let spec = spec_of(conn, id, found.current_version)?;
            for item in resolve::resolve(conn, &spec)?.artifacts {
                out.entry(item.id).or_default().push(ProfileUse {
                    id,
                    name: found.name.clone(),
                    version: found.current_version,
                });
            }
        }
        Ok(out)
    }

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
