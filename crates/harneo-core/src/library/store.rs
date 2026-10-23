//! The SQLite connection, the schema and its forward-only migrations.
//!
//! The schema version lives in `PRAGMA user_version`. `MIGRATIONS[n]` upgrades a database from
//! version `n` to `n + 1`, so the current version is the length of that list. Released
//! migrations are never edited: a schema change is a new entry at the end. A database newer
//! than this build is refused rather than guessed at.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Folder (under the data dir) holding the index and the artifact copies.
const LIBRARY_DIR: &str = "library";
const DB_FILE: &str = "library.db";
const ARTIFACTS_DIR: &str = "artifacts";

/// Waits this long when the app and the CLI touch the database at the same moment.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Version 1: the whole library schema.
///
/// - `sources`: where artifacts came from. `location` is the absolute path (local) or the URL
///   (git); `branch`, `subfolder` and `commit_sha` are only set for git.
/// - `artifacts`: the scanned `Artifact` as JSON plus the columns the list filters and sorts on.
///   `source_path` is the artifact's path in its source, `digest` the scan digest at import,
///   `imported_at` Unix seconds, `replaced_secret_keys` a JSON array of key names.
/// - `tags` and `artifact_tags`; `collections` (manual, or smart with a JSON `rule`) and
///   `collection_items` (manual membership, ordered by `position`).
///
/// Deleting a source removes its artifacts; deleting an artifact, tag or collection removes the
/// rows that point at it.
const V1: &str = "
CREATE TABLE sources (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL CHECK (kind IN ('local', 'git')),
    location   TEXT NOT NULL,
    branch     TEXT,
    subfolder  TEXT,
    commit_sha TEXT
) STRICT;

CREATE TABLE artifacts (
    id                   TEXT PRIMARY KEY,
    source_id            TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    kind                 TEXT NOT NULL,
    name                 TEXT NOT NULL,
    description          TEXT,
    source_path          TEXT NOT NULL,
    digest               TEXT,
    imported_at          INTEGER NOT NULL,
    replaced_secret_keys TEXT NOT NULL DEFAULT '[]',
    artifact             TEXT NOT NULL
) STRICT;
CREATE INDEX artifacts_kind ON artifacts(kind);
CREATE INDEX artifacts_source ON artifacts(source_id);
CREATE INDEX artifacts_name ON artifacts(name COLLATE NOCASE);
CREATE INDEX artifacts_imported_at ON artifacts(imported_at);

CREATE TABLE tags (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL
) STRICT;

CREATE TABLE artifact_tags (
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (artifact_id, tag_id)
) STRICT;
CREATE INDEX artifact_tags_tag ON artifact_tags(tag_id);

CREATE TABLE collections (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    kind TEXT NOT NULL CHECK (kind IN ('manual', 'smart')),
    rule TEXT,
    CHECK ((kind = 'smart') = (rule IS NOT NULL))
) STRICT;

CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    artifact_id   TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    PRIMARY KEY (collection_id, artifact_id)
) STRICT;
CREATE INDEX collection_items_order ON collection_items(collection_id, position);
CREATE INDEX collection_items_artifact ON collection_items(artifact_id);
";

/// Version 2: the audit result on each artifact row. All five columns are NULL until the
/// artifact is audited (a row imported before this version shows as not audited until the next
/// re-audit). `trust_score` is 0 to 100, `tier` the camelCase tier name, `findings` and `reach`
/// the JSON of the audit's lists, `audited_at` Unix seconds.
const V2: &str = "
ALTER TABLE artifacts ADD COLUMN trust_score INTEGER;
ALTER TABLE artifacts ADD COLUMN tier        TEXT;
ALTER TABLE artifacts ADD COLUMN findings    TEXT;
ALTER TABLE artifacts ADD COLUMN reach       TEXT;
ALTER TABLE artifacts ADD COLUMN audited_at  INTEGER;
CREATE INDEX artifacts_trust_score ON artifacts(trust_score);
CREATE INDEX artifacts_tier ON artifacts(tier);
";

/// Version 3: the settings store and the links between artifacts. `settings.value` is JSON,
/// keys are namespaced by module. A link is directed (`from_id` needs `to_id`); `status` keeps
/// rejected proposals so a re-run does not propose them again. No self-links; links go with
/// either artifact.
const V3: &str = "
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE links (
    from_id    TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    to_id      TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    origin     TEXT NOT NULL CHECK (origin IN ('reference', 'ai', 'manual')),
    reason     TEXT NOT NULL DEFAULT '',
    status     TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed', 'rejected')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (from_id, to_id),
    CHECK (from_id <> to_id)
) STRICT;
CREATE INDEX links_to ON links(to_id);
";

/// Forward-only: `MIGRATIONS[n]` takes the schema from version `n` to `n + 1`.
const MIGRATIONS: &[&str] = &[V1, V2, V3];

/// The schema version this build writes.
pub(crate) const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// The open index database.
#[derive(Debug)]
pub(crate) struct Store {
    pub(crate) conn: Connection,
    root: PathBuf,
}

impl Store {
    /// Creates `library/` and `library/artifacts/` under `data_dir` if missing, opens the
    /// database and brings it to [`SCHEMA_VERSION`].
    pub(crate) fn open(data_dir: &Path) -> Result<Self> {
        let root = data_dir.join(LIBRARY_DIR);
        let artifacts = root.join(ARTIFACTS_DIR);
        fs::create_dir_all(&artifacts).map_err(|source| Error::Io {
            path: artifacts.display().to_string(),
            source,
        })?;
        let conn = Connection::open(root.join(DB_FILE))?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        sweep_stale(&root, STALE_AFTER);
        let mut store = Self { conn, root };
        store.migrate()?;
        Ok(store)
    }

    /// `<data dir>/library`.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// `<data dir>/library/artifacts`, one folder per artifact id.
    pub(crate) fn artifacts_dir(&self) -> PathBuf {
        self.root.join(ARTIFACTS_DIR)
    }

    pub(crate) fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// Applies each missing migration in its own transaction, so a failure leaves the
    /// database at the last good version.
    fn migrate(&mut self) -> Result<()> {
        let found = self.schema_version()?;
        if found > SCHEMA_VERSION {
            return Err(Error::SchemaTooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        for (version, sql) in (1u32..).zip(MIGRATIONS).skip(found as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
        }
        Ok(())
    }
}

/// Temporary folders older than this are left over from a crash, never from a live import.
const STALE_AFTER: Duration = Duration::from_secs(3600);

/// Removes clone and staging folders a crashed run left in `<data dir>/library`. Parked
/// `.remove-*` folders are kept: their rows may still exist, so deleting them could lose data.
fn sweep_stale(root: &Path, older_than: Duration) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(".clone-") || name.starts_with(".import-")) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= older_than);
        if stale && entry.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn open_sweeps_stale_clone_and_staging_folders_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        for name in [".clone-a", ".import-b", ".remove-c", "keep"] {
            fs::create_dir_all(root.join(name)).expect("mkdir");
        }
        sweep_stale(&root, Duration::ZERO);
        assert!(!root.join(".clone-a").exists());
        assert!(!root.join(".import-b").exists());
        assert!(root.join(".remove-c").exists());
        assert!(root.join("keep").exists());
        fs::create_dir_all(root.join(".clone-fresh")).expect("mkdir");
        Store::open(dir.path()).expect("open");
        assert!(
            root.join(".clone-fresh").exists(),
            "a fresh clone may belong to a live import"
        );
    }

    fn open() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open(dir.path()).expect("open");
        (dir, store)
    }

    fn add_artifact(store: &Store, source: &str, id: &str) {
        store
            .conn
            .execute(
                "INSERT OR IGNORE INTO sources (id, kind, location) VALUES (?1, 'local', '/x')",
                params![source],
            )
            .expect("source");
        store
            .conn
            .execute(
                "INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
                 VALUES (?1, ?2, 'agent', 'n', 'p', 0, '{}')",
                params![id, source],
            )
            .expect("artifact");
    }

    fn count(store: &Store, table: &str) -> i64 {
        store
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .expect("count")
    }

    #[test]
    fn open_creates_folders_and_schema_at_current_version() {
        let (dir, store) = open();
        assert!(dir.path().join("library/artifacts").is_dir());
        assert!(dir.path().join("library/library.db").is_file());
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        for table in [
            "sources",
            "artifacts",
            "tags",
            "artifact_tags",
            "collections",
            "collection_items",
        ] {
            assert_eq!(count(&store, table), 0, "{table}");
        }
    }

    #[test]
    fn reopening_leaves_schema_and_data_untouched() {
        let (dir, store) = open();
        add_artifact(&store, "s1", "a1");
        let schema = |s: &Store| -> Vec<(String, String)> {
            let mut stmt = s
                .conn
                .prepare("SELECT name, sql FROM sqlite_master ORDER BY name")
                .expect("prepare");
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get(0)?,
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    ))
                })
                .expect("query");
            rows.map(|r| r.expect("row")).collect()
        };
        let before = schema(&store);
        drop(store);

        let again = Store::open(dir.path()).expect("reopen");
        assert_eq!(again.schema_version().expect("v"), SCHEMA_VERSION);
        assert_eq!(schema(&again), before);
        assert_eq!(count(&again, "artifacts"), 1);
    }

    /// The version 1 schema and a row in it, as a build before the audit wrote them.
    #[test]
    fn a_v1_database_opens_keeps_its_data_and_shows_as_not_audited() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v1 db");
            conn.execute_batch(V1).expect("v1 schema");
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location) VALUES ('s1', 'local', '/x');
                 INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
                 VALUES ('a1', 's1', 'agent', 'old', 'agents/old.md', 5, '{}');
                 PRAGMA user_version = 1;",
            )
            .expect("v1 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let row: (String, i64, Option<i64>, Option<String>, Option<String>) = store
            .conn
            .query_row(
                "SELECT name, imported_at, trust_score, tier, findings FROM artifacts",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .expect("row survives");
        assert_eq!(row, ("old".to_owned(), 5, None, None, None));
    }

    #[test]
    fn a_newer_database_is_refused() {
        let (dir, store) = open();
        store
            .conn
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("bump");
        drop(store);
        let err = Store::open(dir.path()).expect_err("too new");
        assert!(matches!(err, Error::SchemaTooNew { .. }));
        assert!(err.to_string().contains("update Harneo"));
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let (_dir, store) = open();
        let orphan = store.conn.execute(
            "INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
             VALUES ('a', 'missing', 'agent', 'n', 'p', 0, '{}')",
            [],
        );
        assert!(orphan.is_err());
    }

    #[test]
    fn deleting_a_source_cascades_to_artifacts_tags_and_collection_items() {
        let (_dir, store) = open();
        add_artifact(&store, "s1", "a1");
        store
            .conn
            .execute_batch(
                "INSERT INTO tags (id, name, color) VALUES (1, 'x', 'blue');
                 INSERT INTO artifact_tags VALUES ('a1', 1);
                 INSERT INTO collections (id, name, kind) VALUES (1, 'c', 'manual');
                 INSERT INTO collection_items VALUES (1, 'a1', 0);",
            )
            .expect("rows");
        store
            .conn
            .execute("DELETE FROM sources", [])
            .expect("delete");
        assert_eq!(count(&store, "artifacts"), 0);
        assert_eq!(count(&store, "artifact_tags"), 0);
        assert_eq!(count(&store, "collection_items"), 0);
        assert_eq!(count(&store, "tags"), 1);
        assert_eq!(count(&store, "collections"), 1);
    }

    #[test]
    fn deleting_a_tag_or_collection_keeps_the_artifacts() {
        let (_dir, store) = open();
        add_artifact(&store, "s1", "a1");
        store
            .conn
            .execute_batch(
                "INSERT INTO tags (id, name, color) VALUES (1, 'x', 'blue');
                 INSERT INTO artifact_tags VALUES ('a1', 1);
                 INSERT INTO collections (id, name, kind) VALUES (1, 'c', 'manual');
                 INSERT INTO collection_items VALUES (1, 'a1', 0);
                 DELETE FROM tags;
                 DELETE FROM collections;",
            )
            .expect("rows");
        assert_eq!(count(&store, "artifacts"), 1);
        assert_eq!(count(&store, "artifact_tags"), 0);
        assert_eq!(count(&store, "collection_items"), 0);
    }

    #[test]
    fn collection_kind_and_rule_must_agree() {
        let (_dir, store) = open();
        let insert = |kind: &str, rule: Option<&str>| {
            store.conn.execute(
                "INSERT INTO collections (name, kind, rule) VALUES (?1, ?2, ?3)",
                params![format!("{kind}-{rule:?}"), kind, rule],
            )
        };
        assert!(insert("manual", None).is_ok());
        assert!(insert("smart", Some("{}")).is_ok());
        assert!(insert("manual", Some("{}")).is_err());
        assert!(insert("smart", None).is_err());
        assert!(insert("other", None).is_err());
    }

    #[test]
    fn tag_and_collection_names_are_unique_ignoring_case() {
        let (_dir, store) = open();
        store
            .conn
            .execute(
                "INSERT INTO tags (name, color) VALUES ('Review', 'red')",
                [],
            )
            .expect("tag");
        assert!(store
            .conn
            .execute(
                "INSERT INTO tags (name, color) VALUES ('review', 'blue')",
                []
            )
            .is_err());
    }
}
