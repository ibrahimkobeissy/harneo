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

/// Version 4: update checking. `artifacts.copy_digest` is the digest of the stored copy's files
/// (what local edits are measured against; NULL until recorded), `updated_at` Unix seconds of the
/// last update. `sources.checked_at` is the last update check, `remote_commit` the commit the
/// remote had then. `upstream` holds one row per artifact whose source differs from the
/// library: `outdated` (its own files changed), `removed` (gone at the source) or `skipped`
/// (the user kept their copy for this `upstream_digest`).
const V4: &str = "
ALTER TABLE artifacts ADD COLUMN copy_digest TEXT;
ALTER TABLE artifacts ADD COLUMN updated_at  INTEGER;
ALTER TABLE sources   ADD COLUMN checked_at    INTEGER;
ALTER TABLE sources   ADD COLUMN remote_commit TEXT;

CREATE TABLE upstream (
    artifact_id     TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE CASCADE,
    state           TEXT NOT NULL CHECK (state IN ('outdated', 'removed', 'skipped')),
    upstream_digest TEXT,
    upstream_commit TEXT,
    from_version    TEXT,
    to_version      TEXT,
    checked_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX upstream_state ON upstream(state);
";

/// Version 5: profiles. `profiles.current_version` is the newest row of `profile_versions`; a
/// version's `spec` is the JSON of a `ProfileSpec` and is never edited. `artifacts.manual_only`
/// is the library default for skills (1 = installed with `disable-model-invocation`).
const V5: &str = "
CREATE TABLE profiles (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description     TEXT NOT NULL DEFAULT '',
    current_version INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
) STRICT;

CREATE TABLE profile_versions (
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    version    INTEGER NOT NULL,
    spec       TEXT NOT NULL,
    saved_at   INTEGER NOT NULL,
    PRIMARY KEY (profile_id, version)
) STRICT;

ALTER TABLE artifacts ADD COLUMN manual_only INTEGER NOT NULL DEFAULT 0;
";

/// Version 6: the install registry. A project is a folder Harneo installed a profile version
/// into (`path` is canonical and unique); `installed_files` is its manifest (`digest` is the
/// SHA-256 of what Harneo wrote, `artifact_id` is NULL for generated files); `backups` is one row
/// per backup set (`folder` is relative to the data folder). A profile in use cannot be deleted.
const V6: &str = "
CREATE TABLE projects (
    id              INTEGER PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    profile_id      INTEGER NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    profile_version INTEGER NOT NULL,
    applied_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX projects_profile ON projects(profile_id);

CREATE TABLE installed_files (
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    digest      TEXT NOT NULL,
    artifact_id TEXT,
    PRIMARY KEY (project_id, path)
) STRICT;

CREATE TABLE backups (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    reason     TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    folder     TEXT NOT NULL
) STRICT;
CREATE INDEX backups_project ON backups(project_id, created_at);
";

/// Version 7: a removed project stays in the registry so its backup sets stay listed and
/// restorable. `removed_at` is set when the profile is removed; the project then has no
/// manifest and no profile link (`profile_id` NULL, `profile_name` keeps the name), and its
/// path may be installed into again as a new project. `path` is unique among live projects only.
/// The tables are rebuilt (children first, so nothing cascades) to change the constraints.
const V7: &str = "
CREATE TABLE installed_files_old AS SELECT * FROM installed_files;
CREATE TABLE backups_old AS SELECT * FROM backups;
CREATE TABLE projects_old AS SELECT * FROM projects;
DROP TABLE installed_files;
DROP TABLE backups;
DROP TABLE projects;

CREATE TABLE projects (
    id              INTEGER PRIMARY KEY,
    path            TEXT NOT NULL,
    name            TEXT NOT NULL,
    profile_id      INTEGER REFERENCES profiles(id) ON DELETE RESTRICT,
    profile_version INTEGER NOT NULL,
    applied_at      INTEGER NOT NULL,
    profile_name    TEXT,
    removed_at      INTEGER,
    CHECK ((removed_at IS NULL) = (profile_id IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX projects_live_path ON projects(path) WHERE removed_at IS NULL;
CREATE INDEX projects_profile ON projects(profile_id);

CREATE TABLE installed_files (
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    digest      TEXT NOT NULL,
    artifact_id TEXT,
    PRIMARY KEY (project_id, path)
) STRICT;

CREATE TABLE backups (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    reason     TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    folder     TEXT NOT NULL
) STRICT;
CREATE INDEX backups_project ON backups(project_id, created_at);

INSERT INTO projects (id, path, name, profile_id, profile_version, applied_at)
    SELECT id, path, name, profile_id, profile_version, applied_at FROM projects_old;
INSERT INTO installed_files SELECT * FROM installed_files_old;
INSERT INTO backups SELECT * FROM backups_old;
DROP TABLE installed_files_old;
DROP TABLE backups_old;
DROP TABLE projects_old;
";

/// Version 8: audit phase 2 (Claude's confirmation). `dismissed` is the JSON list of findings
/// Claude judged not to be a threat, `confirmation` the phase 2 status, model and time.
/// `audit_verdicts` caches each verdict by a hash of (rule, file, line text, context), so an
/// unchanged finding never asks Claude again.
const V8: &str = "
ALTER TABLE artifacts ADD COLUMN dismissed    TEXT;
ALTER TABLE artifacts ADD COLUMN confirmation TEXT;

CREATE TABLE audit_verdicts (
    key        TEXT PRIMARY KEY,
    threat     INTEGER NOT NULL CHECK (threat IN (0, 1)),
    reason     TEXT NOT NULL,
    model      TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

UPDATE artifacts SET
    dismissed = '[]',
    confirmation = CASE WHEN findings = '[]'
        THEN '{\"status\":\"confirmed\",\"model\":null,\"at\":null,\"detail\":null}'
        ELSE '{\"status\":\"pending\",\"model\":null,\"at\":null,\"detail\":null}' END
    WHERE findings IS NOT NULL;
";

/// Version 9: the layout resolver's stored layout. `sources.layout_version` is 0 for a source
/// imported before the resolver and the resolver's version after; `layout_ignore` is the JSON
/// list of paths Claude said are not artifacts. Each artifact's `layoutBy` and `layoutReason`
/// live in its stored `artifact` JSON; a row without them counts as placed by a convention
/// (a manifest for a plugin). `upstream` gains the `layout_changed` state, and SQLite cannot
/// alter a CHECK, so the table is rebuilt with its rows.
const V9: &str = "
ALTER TABLE sources ADD COLUMN layout_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sources ADD COLUMN layout_ignore  TEXT;

CREATE TABLE upstream_new (
    artifact_id     TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE CASCADE,
    state           TEXT NOT NULL
        CHECK (state IN ('outdated', 'removed', 'skipped', 'layout_changed')),
    upstream_digest TEXT,
    upstream_commit TEXT,
    from_version    TEXT,
    to_version      TEXT,
    checked_at      INTEGER NOT NULL
) STRICT;
INSERT INTO upstream_new SELECT * FROM upstream;
DROP TABLE upstream;
ALTER TABLE upstream_new RENAME TO upstream;
CREATE INDEX upstream_state ON upstream(state);
";

/// Large files: an import can opt in to copying files over the scan's per-file limit (up to
/// the copy ceiling), remembered per source so updates keep them; an artifact's audit lists
/// the files it did not read because of their size.
const V10: &str = "
ALTER TABLE sources   ADD COLUMN large_files INTEGER NOT NULL DEFAULT 0;
ALTER TABLE artifacts ADD COLUMN unaudited   TEXT;
";

/// Unpacked plugins: `sources.unpack` is the JSON list of plugin folders the user took apart at
/// import (`NULL` for none), replayed by update checks so the parts keep updating as artifacts.
const V11: &str = "
ALTER TABLE sources ADD COLUMN unpack TEXT;
";

/// Harnmap: a profile's `category` (picks the map template; existing profiles are `software`)
/// and Custom's `category_note`, the draws of each profile's map (the highest `no` is the
/// current map, older rows are frozen) and each candidate's latest Harnfit verdict.
const V12: &str = "
ALTER TABLE profiles ADD COLUMN category      TEXT NOT NULL DEFAULT 'software';
ALTER TABLE profiles ADD COLUMN category_note TEXT;

CREATE TABLE harnmap_draws (
    id            INTEGER PRIMARY KEY,
    profile_id    INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    no            INTEGER NOT NULL,
    created_at    INTEGER NOT NULL,
    model         TEXT NOT NULL,
    duration_ms   INTEGER NOT NULL,
    description   TEXT NOT NULL,
    category      TEXT NOT NULL,
    map           TEXT NOT NULL,
    basis         TEXT NOT NULL,
    restored_from INTEGER,
    UNIQUE (profile_id, no)
) STRICT;

CREATE TABLE harnfit_fits (
    profile_id  INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    candidate   TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    model       TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    verdict     TEXT NOT NULL,
    basis       TEXT NOT NULL,
    PRIMARY KEY (profile_id, candidate)
) STRICT;
";

/// Forward-only: `MIGRATIONS[n]` takes the schema from version `n` to `n + 1`.
const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12];

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
        if !(name.starts_with(".clone-")
            || name.starts_with(".import-")
            || name.starts_with(".update-"))
        {
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

    /// A version 3 database with a source and an artifact gains the update columns and table.
    #[test]
    fn a_v3_database_migrates_to_the_update_schema_keeping_its_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v3 db");
            for sql in [V1, V2, V3] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location, commit_sha) VALUES ('s1', 'git', 'u', 'abc');
                 INSERT INTO artifacts (id, source_id, kind, name, source_path, digest, imported_at, artifact)
                 VALUES ('a1', 's1', 'agent', 'old', 'agents/old.md', 'd1', 5, '{}');
                 PRAGMA user_version = 3;",
            )
            .expect("v3 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let row: (String, String, Option<String>, Option<i64>) = store
            .conn
            .query_row(
                "SELECT a.name, a.digest, a.copy_digest, a.updated_at FROM artifacts a",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("row survives");
        assert_eq!(row, ("old".to_owned(), "d1".to_owned(), None, None));
        let source: (String, Option<i64>, Option<String>) = store
            .conn
            .query_row(
                "SELECT commit_sha, checked_at, remote_commit FROM sources",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("source survives");
        assert_eq!(source, ("abc".to_owned(), None, None));
        store
            .conn
            .execute(
                "INSERT INTO upstream (artifact_id, state, checked_at) VALUES ('a1', 'outdated', 1)",
                [],
            )
            .expect("upstream row");
        assert!(store
            .conn
            .execute(
                "INSERT INTO upstream (artifact_id, state, checked_at) VALUES ('a1', 'bogus', 1)",
                [],
            )
            .is_err());
        store
            .conn
            .execute("DELETE FROM artifacts", [])
            .expect("delete");
        assert_eq!(
            count(&store, "upstream"),
            0,
            "upstream goes with the artifact"
        );
    }

    /// A version 7 database with audited artifacts migrates to version 8: findings stay, and each
    /// artifact gets a phase 2 status (pending with findings, confirmed without).
    #[test]
    fn a_v7_database_migrates_to_audit_phase_two() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v7 db");
            for sql in [V1, V2, V3, V4, V5, V6, V7] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location) VALUES ('s', 'local', '/x');
                 INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at,
                     artifact, replaced_secret_keys, trust_score, tier, findings, reach, audited_at)
                 VALUES ('a', 's', 'skill', 'a', 'a', 1, '{}', '[]', 75, 'review', '[{}]', '{}', 1),
                        ('b', 's', 'skill', 'b', 'b', 1, '{}', '[]', 100, 'trusted', '[]', '{}', 1),
                        ('c', 's', 'skill', 'c', 'c', 1, '{}', '[]', NULL, NULL, NULL, NULL, NULL);
                 PRAGMA user_version = 7;",
            )
            .expect("v7 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let status = |id: &str| -> Option<String> {
            store
                .conn
                .query_row(
                    "SELECT json_extract(confirmation, '$.status') FROM artifacts WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .expect("row")
        };
        assert_eq!(status("a").as_deref(), Some("pending"));
        assert_eq!(status("b").as_deref(), Some("confirmed"));
        assert_eq!(status("c"), None, "never audited stays so");
        assert_eq!(count(&store, "audit_verdicts"), 0);
    }

    /// A version 10 database migrates to version 11: sources gain `unpack`, empty for the ones
    /// imported before.
    #[test]
    fn a_v10_database_migrates_to_unpacked_plugins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v10 db");
            for sql in [V1, V2, V3, V4, V5, V6, V7, V8, V9, V10] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location, large_files) VALUES ('s', 'local', '/x', 1);
                 PRAGMA user_version = 10;",
            )
            .expect("v10 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let source: (i64, Option<String>) = store
            .conn
            .query_row("SELECT large_files, unpack FROM sources", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("source survives");
        assert_eq!(source, (1, None));
    }

    /// A version 11 database migrates to version 12: existing profiles become `software`, and
    /// the Harnmap tables exist and cascade with their profile.
    #[test]
    fn a_v11_database_migrates_to_harnmap() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v11 db");
            for sql in [V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO profiles (id, name, description, current_version, created_at, updated_at)
                     VALUES (1, 'Old', '', 1, 0, 0);
                 PRAGMA user_version = 11;",
            )
            .expect("v11 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let profile: (String, Option<String>) = store
            .conn
            .query_row("SELECT category, category_note FROM profiles", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("profile survives");
        assert_eq!(profile, ("software".to_owned(), None));
        store
            .conn
            .execute_batch(
                "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms,
                     description, category, map, basis) VALUES (1, 1, 0, 'opus', 5, '', 'software', '{}', '{}');
                 INSERT INTO harnfit_fits (profile_id, candidate, created_at, model, duration_ms,
                     verdict, basis) VALUES (1, 'a', 0, 'opus', 5, '{}', '{}');",
            )
            .expect("the new tables take rows");
        assert!(store
            .conn
            .execute_batch(
                "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms,
                     description, category, map, basis) VALUES (1, 1, 0, 'opus', 5, '', 'software', '{}', '{}');",
            )
            .is_err());
        store
            .conn
            .execute("DELETE FROM profiles", [])
            .expect("delete profile");
        let left: i64 = store
            .conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM harnmap_draws) + (SELECT COUNT(*) FROM harnfit_fits)",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(left, 0);
    }

    /// A version 8 database migrates to version 9: sources get a layout version of 0 and no
    /// ignore list, upstream rows survive the rebuild, and `layout_changed` becomes valid.
    #[test]
    fn a_v8_database_migrates_to_the_stored_layout() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v8 db");
            for sql in [V1, V2, V3, V4, V5, V6, V7, V8] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location) VALUES ('s', 'local', '/x');
                 INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
                 VALUES ('a', 's', 'skill', 'a', 'a', 1, '{}');
                 INSERT INTO upstream (artifact_id, state, from_version, checked_at)
                 VALUES ('a', 'removed', '1.0', 7);
                 PRAGMA user_version = 8;",
            )
            .expect("v8 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let source: (i64, Option<String>) = store
            .conn
            .query_row(
                "SELECT layout_version, layout_ignore FROM sources",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("source survives");
        assert_eq!(source, (0, None));
        let kept: (String, Option<String>, i64) = store
            .conn
            .query_row(
                "SELECT state, from_version, checked_at FROM upstream WHERE artifact_id = 'a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("upstream row survives");
        assert_eq!(kept, ("removed".to_owned(), Some("1.0".to_owned()), 7));
        store
            .conn
            .execute(
                "UPDATE upstream SET state = 'layout_changed' WHERE artifact_id = 'a'",
                [],
            )
            .expect("the new state is valid");
        assert!(store
            .conn
            .execute(
                "UPDATE upstream SET state = 'bogus' WHERE artifact_id = 'a'",
                []
            )
            .is_err());
        store
            .conn
            .execute("DELETE FROM artifacts WHERE id = 'a'", [])
            .expect("delete");
        assert_eq!(
            count(&store, "upstream"),
            0,
            "upstream still goes with its artifact"
        );
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

    /// A version 4 database with an artifact gains the profile tables and the manual-only column.
    #[test]
    fn a_v4_database_migrates_to_profiles_keeping_its_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v4 db");
            for sql in [V1, V2, V3, V4] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO sources (id, kind, location) VALUES ('s1', 'local', '/x');
                 INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
                 VALUES ('a1', 's1', 'skill', 'old', 'skills/old', 5, '{}');
                 PRAGMA user_version = 4;",
            )
            .expect("v4 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        let row: (String, i64) = store
            .conn
            .query_row("SELECT name, manual_only FROM artifacts", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("row survives");
        assert_eq!(row, ("old".to_owned(), 0));
        assert_eq!(count(&store, "profiles"), 0);
        assert_eq!(count(&store, "profile_versions"), 0);
    }

    /// A version 5 database with a profile gains the install registry, keeping its data.
    #[test]
    fn a_v5_database_migrates_to_the_install_registry_keeping_its_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v5 db");
            for sql in [V1, V2, V3, V4, V5] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO profiles (id, name, current_version, created_at, updated_at)
                 VALUES (1, 'web', 1, 5, 5);
                 INSERT INTO profile_versions (profile_id, version, spec, saved_at)
                 VALUES (1, 1, '{}', 5);
                 PRAGMA user_version = 5;",
            )
            .expect("v5 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        assert_eq!(count(&store, "profiles"), 1);
        assert_eq!(count(&store, "projects"), 0);
        assert_eq!(count(&store, "installed_files"), 0);
        assert_eq!(count(&store, "backups"), 0);
        store
            .conn
            .execute(
                "INSERT INTO projects (path, name, profile_id, profile_version, applied_at)
                 VALUES ('/p', 'p', 1, 1, 9)",
                [],
            )
            .expect("a project can use the old profile");
        assert!(
            store
                .conn
                .execute("DELETE FROM profiles WHERE id = 1", [])
                .is_err(),
            "a profile in use is protected by the schema too"
        );
    }
    /// A version 6 database with a project, its manifest and a backup migrates to version 7
    /// keeping all three, and a removed project can then keep its backups.
    #[test]
    fn a_v6_database_migrates_to_removed_projects_keeping_its_data() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join(LIBRARY_DIR);
        fs::create_dir_all(root.join(ARTIFACTS_DIR)).expect("mkdir");
        {
            let conn = Connection::open(root.join(DB_FILE)).expect("v6 db");
            for sql in [V1, V2, V3, V4, V5, V6] {
                conn.execute_batch(sql).expect("schema");
            }
            conn.execute_batch(
                "INSERT INTO profiles (id, name, current_version, created_at, updated_at)
                 VALUES (1, 'web', 1, 5, 5);
                 INSERT INTO profile_versions (profile_id, version, spec, saved_at)
                 VALUES (1, 1, '{}', 5);
                 INSERT INTO projects (id, path, name, profile_id, profile_version, applied_at)
                 VALUES (7, '/p', 'p', 1, 1, 9);
                 INSERT INTO installed_files (project_id, path, digest) VALUES (7, 'a.md', 'd');
                 INSERT INTO backups (project_id, created_at, reason, file_count, folder)
                 VALUES (7, 9, 'r', 1, 'backups/7/x');
                 PRAGMA user_version = 6;",
            )
            .expect("v6 rows");
        }
        let store = Store::open(dir.path()).expect("migrates");
        assert_eq!(store.schema_version().expect("v"), SCHEMA_VERSION);
        assert_eq!(count(&store, "projects"), 1);
        assert_eq!(count(&store, "installed_files"), 1);
        assert_eq!(count(&store, "backups"), 1);
        // A removal keeps the row without a profile link; the path is free again.
        store
            .conn
            .execute(
                "UPDATE projects SET removed_at = 10, profile_id = NULL, profile_name = 'web'
                 WHERE id = 7",
                [],
            )
            .expect("remove");
        store
            .conn
            .execute(
                "INSERT INTO projects (path, name, profile_id, profile_version, applied_at)
                 VALUES ('/p', 'p', 1, 1, 11)",
                [],
            )
            .expect("the path can be installed into again");
        assert!(
            store
                .conn
                .execute(
                    "INSERT INTO projects (path, name, profile_id, profile_version, applied_at)
                     VALUES ('/p', 'p', 1, 1, 12)",
                    [],
                )
                .is_err(),
            "a live path stays unique"
        );
        assert_eq!(
            count(&store, "backups"),
            1,
            "the removed project keeps its backups"
        );
    }
}
