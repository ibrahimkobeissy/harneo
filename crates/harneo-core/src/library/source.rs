//! Where an import comes from.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ids;
use crate::error::Result;

/// A place artifacts can be imported from. Git joins as another variant, so match on it with a
/// wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
#[non_exhaustive]
pub enum Source {
    /// A folder on this machine (`~/.claude`, a project, a clone).
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Local {
        /// The folder, as the user gave it.
        #[ts(type = "string")]
        path: PathBuf,
    },
}

impl Source {
    /// The stable key the ids are derived from (see [`ids`]).
    pub(crate) fn identity(&self) -> Result<String> {
        match self {
            Self::Local { path } => ids::local_source_key(path),
        }
    }
}
