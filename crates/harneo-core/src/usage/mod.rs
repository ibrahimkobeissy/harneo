//! What the user actually uses, read from Claude's own session logs. Names only: the indexer
//! keeps the session id, day, project path and the name of each skill, subagent, command, MCP
//! server and plugin used, and never a message, prompt, tool input or output.
//!
//! The index lives in its own database, `<data dir>/usage/usage.db`, apart from the library.
//! A `UsageIndex` is a cheap handle: each call opens its own connection, so a long index run
//! never blocks a query (the database is in WAL mode).

mod cleanup;
#[cfg(test)]
mod cleanup_tests;
mod index;
#[cfg(test)]
mod index_tests;
mod parse;
#[cfg(test)]
mod parse_tests;
#[cfg(test)]
mod queries_tests;
mod status;
#[cfg(test)]
mod status_tests;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

pub use cleanup::{
    Cleanup, CleanupItem, Duplicate, NotSwitched, PruneFailure, PrunePlan, PruneProfile,
    PruneProject, PruneReport, PruneSwitched, REPEATED_IN,
};
pub use index::{IndexProgress, IndexReport};
pub(crate) use status::{
    attach, last_used_sql, outdated_sql, sessions_sql, status_sql, tracked_sql,
};
pub use status::{is_tracked, Status, UsageBrief, DEFAULT_UNUSED_WEEKS, UNUSED_WEEKS_KEY};

use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};

const USAGE_DIR: &str = "usage";
const DB_FILE: &str = "usage.db";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Weeks the usage window covers.
pub const WEEKS: usize = 8;

/// Version 2 adds `log_files.head_hash`: a hash of the first bytes read, to notice a file
/// rewritten to the same size.
///
/// Version 1: the log files already read and the uses found in them.
///
/// - `log_files`: per file, the size and mtime (ns) last seen, the byte `offset` read up to
///   (always just after a newline), and the lines parsed and failed so far.
/// - `uses`: one row per session, kind, name and day; `project_path` is the session's `cwd`
///   (empty when unknown). Kind is the artifact kind in camelCase.
const MIGRATIONS: &[&str] = &[
    "
CREATE TABLE log_files (
    path         TEXT PRIMARY KEY,
    size         INTEGER NOT NULL,
    mtime        INTEGER NOT NULL,
    offset       INTEGER NOT NULL,
    parsed_lines INTEGER NOT NULL DEFAULT 0,
    failed_lines INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE uses (
    session_id   TEXT NOT NULL,
    day          TEXT NOT NULL,
    project_path TEXT NOT NULL DEFAULT '',
    kind         TEXT NOT NULL,
    name         TEXT NOT NULL,
    PRIMARY KEY (session_id, kind, name, day)
) STRICT;
CREATE INDEX uses_kind_name ON uses(kind, name, day);
CREATE INDEX uses_day ON uses(day);
",
    "
ALTER TABLE log_files ADD COLUMN head_hash INTEGER NOT NULL DEFAULT 0;
",
];

/// Usage of one artifact (matched by kind and name) over the last 8 weeks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Usage {
    /// Distinct sessions in the last 8 weeks.
    pub sessions_8w: u32,
    /// Sessions per week, oldest first; the last entry is the week ending today.
    pub weekly: [u32; WEEKS],
    /// The most recent day (`YYYY-MM-DD`) it was used, over everything indexed.
    pub last_used: Option<String>,
    /// Project folders it was used in, over everything indexed, sorted.
    pub projects: Vec<String>,
}

/// Sessions per week over all tracked uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct UsageSummary {
    /// Sessions in the week ending today.
    pub this_week: u32,
    /// Change against the week before, in percent; none when that week had no sessions.
    pub change_pct: Option<i32>,
    pub weekly: [u32; WEEKS],
}

/// Handle to the usage index.
#[derive(Debug, Clone)]
pub struct UsageIndex {
    db: PathBuf,
    /// Held for the length of an index run, so two runs never overlap.
    running: Arc<Mutex<()>>,
}

impl UsageIndex {
    /// Opens (creating and migrating) `<data_dir>/usage/usage.db`.
    pub fn open(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join(USAGE_DIR);
        fs::create_dir_all(&dir).map_err(|source| Error::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let this = Self {
            db: dir.join(DB_FILE),
            running: Arc::new(Mutex::new(())),
        };
        let mut conn = this.connect()?;
        migrate(&mut conn)?;
        Ok(this)
    }

    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))?;
        Ok(conn)
    }

    /// Reads new log lines under `logs_root` (the app passes `~/.claude/projects`), calling
    /// `progress` as files are done. A missing root is an empty index, not an error.
    pub fn index(
        &self,
        logs_root: &Path,
        mut progress: impl FnMut(IndexProgress),
    ) -> Result<IndexReport> {
        // A run that finds another one going waits, then finds little new to read.
        let _guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let mut conn = self.connect()?;
        index::run(&mut conn, logs_root, &mut progress)
    }

    /// Usage of the artifact of `kind` named `name`, as of today.
    pub fn usage_of(&self, kind: ArtifactKind, name: &str) -> Result<Usage> {
        self.usage_of_at(kind, name, today())
    }

    /// Sessions per week over every tracked use, as of today.
    pub fn usage_summary(&self) -> Result<UsageSummary> {
        self.usage_summary_at(today())
    }

    pub(crate) fn usage_of_at(&self, kind: ArtifactKind, name: &str, today: i64) -> Result<Usage> {
        let conn = self.connect()?;
        let kind = kind_key(kind);
        let mut stmt = conn.prepare(
            "SELECT session_id, day, project_path FROM uses WHERE kind = ?1 AND name = ?2",
        )?;
        let rows = stmt.query_map(params![kind, name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut weeks: [HashSet<String>; WEEKS] = Default::default();
        let mut all = HashSet::new();
        let mut projects = std::collections::BTreeSet::new();
        let mut last: Option<String> = None;
        for row in rows {
            let (session, day, project) = row?;
            if !project.is_empty() {
                projects.insert(project);
            }
            if last.as_deref().is_none_or(|l| day.as_str() > l) {
                last = Some(day.clone());
            }
            if let Some(bucket) = day_number(&day).and_then(|d| week_bucket(today, d)) {
                all.insert(session.clone());
                weeks[bucket].insert(session);
            }
        }
        Ok(Usage {
            sessions_8w: all.len() as u32,
            weekly: weeks.map(|w| w.len() as u32),
            last_used: last,
            projects: projects.into_iter().collect(),
        })
    }

    pub(crate) fn usage_summary_at(&self, today: i64) -> Result<UsageSummary> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT DISTINCT session_id, day FROM uses")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut weeks: [HashSet<String>; WEEKS] = Default::default();
        for row in rows {
            let (session, day) = row?;
            if let Some(bucket) = day_number(&day).and_then(|d| week_bucket(today, d)) {
                weeks[bucket].insert(session);
            }
        }
        let weekly = weeks.map(|w| w.len() as u32);
        let (this_week, before) = (weekly[WEEKS - 1], weekly[WEEKS - 2]);
        let change_pct = (before > 0).then(|| {
            let delta = f64::from(this_week) - f64::from(before);
            (delta / f64::from(before) * 100.0).round() as i32
        });
        Ok(UsageSummary {
            this_week,
            change_pct,
            weekly,
        })
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let found: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let supported = MIGRATIONS.len() as u32;
    if found > supported {
        return Err(Error::SchemaTooNew { found, supported });
    }
    for (n, sql) in MIGRATIONS.iter().enumerate().skip(found as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", n as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// The key a kind is stored under (its camelCase name).
fn kind_key(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::ClaudeMd => "claudeMd",
        ArtifactKind::Rule => "rule",
        ArtifactKind::Skill => "skill",
        ArtifactKind::Command => "command",
        ArtifactKind::Agent => "agent",
        ArtifactKind::Settings => "settings",
        ArtifactKind::Mcp => "mcp",
        ArtifactKind::OutputStyle => "outputStyle",
        ArtifactKind::Workflow => "workflow",
        ArtifactKind::Plugin => "plugin",
    }
}

/// Days since 1970-01-01 for today (UTC).
fn today() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` day.
pub(crate) fn day_number(day: &str) -> Option<i64> {
    let year: i64 = day.get(0..4)?.parse().ok()?;
    let month: i64 = day.get(5..7)?.parse().ok()?;
    let d: i64 = day.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Which of the 8 weekly buckets (0 = oldest, 7 = the week ending `today`) a day falls in;
/// none when older than 8 weeks. A day after `today` (clock skew) counts as today.
fn week_bucket(today: i64, day: i64) -> Option<usize> {
    let ago = (today - day).max(0);
    let week = (ago / 7) as usize;
    (week < WEEKS).then(|| WEEKS - 1 - week)
}
