//! The SQLite connection, the schema and its forward-only migrations.
//!
//! The schema version lives in `PRAGMA user_version`. `MIGRATIONS[n]` upgrades a database from
//! version `n` to `n + 1`, so the current version is the length of that list. Released
//! migrations are never edited: a schema change is a new entry at the end. A database newer
//! than this build is refused rather than guessed at.

use std::fs;
use std::path::Path;
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

/// Forward-only: `MIGRATIONS[n]` takes the schema from version `n` to `n + 1`.
const MIGRATIONS: &[&str] = &[V1];

/// The schema version this build writes.
pub(crate) const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// The open index database.
#[derive(Debug)]
pub(crate) struct Store {
    conn: Connection,
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
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

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
