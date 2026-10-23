//! What the library queries know about usage: a library artifact's sessions and last use, and
//! its [`Status`].
//!
//! The usage index is its own database, so the library connection ATTACHes it as `usage` (see
//! [`attach`]) and the list queries join against it in SQL, which keeps a page at 2,000
//! artifacts fast. Everything time-dependent (the 8-week window, the unused window) is computed
//! by SQLite from `'now'`, so the SQL fragments carry no parameters.

use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{UsageIndex, MIGRATIONS, WEEKS};
use crate::artifact::ArtifactKind;
use crate::error::Result;

/// The setting holding the unused window, in weeks.
pub const UNUSED_WEEKS_KEY: &str = "usage.unusedWeeks";
/// The default unused window, in weeks.
pub const DEFAULT_UNUSED_WEEKS: i64 = 8;

/// Kinds whose use shows in Claude's logs, in the form the library stores them.
const TRACKED: &str = "('skill', 'command', 'agent', 'mcp', 'plugin')";

/// Whether the logs record when an artifact of `kind` is used.
pub fn is_tracked(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Skill
            | ArtifactKind::Command
            | ArtifactKind::Agent
            | ArtifactKind::Mcp
            | ArtifactKind::Plugin
    )
}

/// How healthy an artifact looks, from its audit, update state and use. The first that applies
/// wins, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Status {
    /// Audit findings and a trust score under 60.
    Findings,
    /// Its source changed since import.
    Outdated,
    /// Tracked, no sessions in the unused window, and imported before that window.
    Unused,
    /// Tracked, no sessions, imported within the window.
    NotUsedYet,
    Current,
}

impl Status {
    pub const ALL: [Status; 5] = [
        Status::Findings,
        Status::Outdated,
        Status::Unused,
        Status::NotUsedYet,
        Status::Current,
    ];

    /// The camelCase name used in SQL results and in smart-collection rules.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Findings => "findings",
            Status::Outdated => "outdated",
            Status::Unused => "unused",
            Status::NotUsedYet => "notUsedYet",
            Status::Current => "current",
        }
    }

    pub(crate) fn parse(text: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.as_str() == text)
            .unwrap_or(Status::Current)
    }
}

/// Usage on a listed artifact; absent for kinds the logs do not track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UsageBrief {
    /// Distinct sessions in the last 8 weeks.
    pub sessions: u32,
    /// The most recent day (`YYYY-MM-DD`) it was used, over everything indexed.
    pub last_used: Option<String>,
}

/// Attaches the usage index of `data_dir` to `conn` as `usage`. When it cannot be opened (a
/// newer schema, a read-only folder) an empty in-memory index stands in, so the library still
/// opens and every tracked artifact counts as unused.
pub(crate) fn attach(conn: &Connection, data_dir: &Path) -> Result<()> {
    let file = UsageIndex::open(data_dir).map(|index| index.db);
    match file {
        Ok(path)
            if conn
                .execute("ATTACH DATABASE ?1 AS usage", [path.to_string_lossy()])
                .is_ok() =>
        {
            Ok(())
        }
        _ => {
            conn.execute_batch("ATTACH DATABASE ':memory:' AS usage")?;
            conn.execute_batch(MIGRATIONS[0])?;
            Ok(())
        }
    }
}

/// The unused window in weeks, from the setting (at least 1; 8 when unset or not a number).
fn window_weeks() -> String {
    format!(
        "MAX(1, COALESCE(NULLIF(CAST((SELECT value FROM settings WHERE key = '{UNUSED_WEEKS_KEY}') \
         AS INTEGER), 0), {DEFAULT_UNUSED_WEEKS}))"
    )
}

/// The first day inside the 8-week window.
fn window_start() -> String {
    format!("date('now', '-{} days')", WEEKS * 7 - 1)
}

/// Whether the artifact row aliased `a` has an outdated source (see `updates`).
pub(crate) fn outdated_sql(a: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM upstream u WHERE u.artifact_id = {a}.id AND u.state = 'outdated')"
    )
}

/// Whether the artifact row aliased `a` is of a tracked kind.
pub(crate) fn tracked_sql(a: &str) -> String {
    format!("{a}.kind IN {TRACKED}")
}

/// Distinct sessions in the last 8 weeks that used the artifact row aliased `a`.
pub(crate) fn sessions_sql(a: &str) -> String {
    format!(
        "(SELECT COUNT(DISTINCT u.session_id) FROM usage.uses u \
         WHERE u.kind = {a}.kind AND u.name = {a}.name AND u.day >= {})",
        window_start()
    )
}

/// The most recent day the artifact row aliased `a` was used.
pub(crate) fn last_used_sql(a: &str) -> String {
    format!("(SELECT MAX(u.day) FROM usage.uses u WHERE u.kind = {a}.kind AND u.name = {a}.name)")
}

/// The status of the artifact row aliased `a` (columns `id`, `kind`, `name`, `trust_score`,
/// `findings`, `imported_at`), as a camelCase string.
pub(crate) fn status_sql(a: &str) -> String {
    let weeks = window_weeks();
    format!(
        "(CASE \
         WHEN {a}.trust_score < 60 AND COALESCE(json_array_length({a}.findings), 0) > 0 \
              THEN 'findings' \
         WHEN {outdated} THEN 'outdated' \
         WHEN {tracked} AND NOT EXISTS (SELECT 1 FROM usage.uses u \
              WHERE u.kind = {a}.kind AND u.name = {a}.name \
                AND u.day >= date('now', '-' || ({weeks} * 7 - 1) || ' days')) \
              THEN (CASE WHEN {a}.imported_at < CAST(strftime('%s', 'now') AS INTEGER) - {weeks} * 604800 \
                         THEN 'unused' ELSE 'notUsedYet' END) \
         ELSE 'current' END)",
        outdated = outdated_sql(a),
        tracked = tracked_sql(a),
    )
}
