//! Every artifact's stored audit in one read, for the Security page.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{records, ArtifactId, Library};
use crate::artifact::ArtifactKind;
use crate::audit::Audit;
use crate::error::Result;

/// One artifact with its stored audit; `audit` is `None` until it is audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ArtifactAudit {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    pub audit: Option<Audit>,
}

impl Library {
    /// The stored audit of every artifact, sorted by name then id.
    pub fn audits(&self) -> Result<Vec<ArtifactAudit>> {
        let conn = &self.store.conn;
        let mut stmt = conn.prepare("SELECT id, kind, name FROM artifacts ORDER BY name, id")?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map(params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, kind, name) in rows {
            let audit = records::load_audit(conn, &id)?.map(|(audit, _)| audit);
            out.push(ArtifactAudit {
                kind: serde_json::from_value(serde_json::Value::String(kind))?,
                id,
                name,
                audit,
            });
        }
        Ok(out)
    }
}
