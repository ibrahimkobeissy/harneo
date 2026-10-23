//! Usage and status in `Library::list`. Rows go straight into the index; uses go straight into
//! the attached usage database.

use std::time::Instant;

use rusqlite::params;
use serde_json::json;

use crate::library::{Filter, Library, Query, SortKey};
use crate::usage::{Status, UsageBrief};

const DAY: i64 = 86_400;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn lib() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().expect("dir");
    let lib = Library::open(dir.path()).expect("open");
    lib.store
        .conn
        .execute(
            "INSERT INTO sources (id, kind, location, branch) VALUES ('s', 'local', '/x/src', NULL)",
            [],
        )
        .expect("source");
    (dir, lib)
}

/// An artifact imported `age_days` ago; `trust` of `Some` with `findings` set stores a finding.
fn add(lib: &Library, id: &str, kind: &str, age_days: i64, trust: Option<u8>) {
    let findings = trust.map(|_| "[{\"id\":\"f\"}]");
    lib.store
        .conn
        .execute(
            "INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact,
                                    trust_score, findings)
             VALUES (?1, 's', ?2, ?1, 'p', ?3, '{\"diagnostics\":[]}', ?4, ?5)",
            params![id, kind, now() - age_days * DAY, trust, findings],
        )
        .expect("artifact");
}

fn used(lib: &Library, session: &str, kind: &str, name: &str, days_ago: i64) {
    let day: String = lib
        .store
        .conn
        .query_row(
            "SELECT date('now', ?1)",
            params![format!("-{days_ago} days")],
            |r| r.get(0),
        )
        .expect("day");
    lib.store
        .conn
        .execute(
            "INSERT INTO usage.uses VALUES (?1, ?2, '/p', ?3, ?4)",
            params![session, day, kind, name],
        )
        .expect("use");
}

fn outdate(lib: &Library, id: &str) {
    lib.store
        .conn
        .execute(
            "INSERT INTO upstream (artifact_id, state, checked_at) VALUES (?1, 'outdated', 1)",
            params![id],
        )
        .expect("upstream");
}

fn status_of(lib: &Library, id: &str) -> Status {
    let page = lib.list(&Query::default()).expect("list");
    page.items
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.status)
        .expect("item")
}

#[test]
fn usage_status_precedence() {
    let (_d, lib) = lib();
    add(&lib, "bad", "skill", 100, Some(40));
    outdate(&lib, "bad"); // findings beat outdated
    add(&lib, "stale", "skill", 100, None);
    outdate(&lib, "stale"); // outdated beats unused
    add(&lib, "old", "skill", 100, None); // never used, old
    add(&lib, "fresh", "skill", 3, None); // never used, new
    add(&lib, "busy", "skill", 100, None);
    used(&lib, "s1", "skill", "busy", 2);
    add(&lib, "ancient", "skill", 100, None);
    used(&lib, "s1", "skill", "ancient", 70); // only outside the window: still unused
    add(&lib, "rule", "rule", 100, None); // untracked: never unused
    add(&lib, "okish", "skill", 100, Some(75)); // findings but trust >= 60
    used(&lib, "s2", "skill", "okish", 1);

    let want = [
        ("bad", Status::Findings),
        ("stale", Status::Outdated),
        ("old", Status::Unused),
        ("fresh", Status::NotUsedYet),
        ("busy", Status::Current),
        ("ancient", Status::Unused),
        ("rule", Status::Current),
        ("okish", Status::Current),
    ];
    for (id, status) in want {
        assert_eq!(status_of(&lib, id), status, "{id}");
    }
}

#[test]
fn usage_window_follows_the_setting() {
    let (_d, lib) = lib();
    add(&lib, "mid", "skill", 20, None); // 3 weeks old
    assert_eq!(status_of(&lib, "mid"), Status::NotUsedYet);
    lib.settings_set("usage.unusedWeeks", &json!(2))
        .expect("set");
    assert_eq!(status_of(&lib, "mid"), Status::Unused);
    used(&lib, "s", "skill", "mid", 25); // outside 2 weeks
    assert_eq!(status_of(&lib, "mid"), Status::Unused);
    used(&lib, "t", "skill", "mid", 5); // inside 2 weeks
    assert_eq!(status_of(&lib, "mid"), Status::Current);
    lib.settings_set("usage.unusedWeeks", &json!("junk"))
        .expect("set");
    assert_eq!(status_of(&lib, "mid"), Status::Current);
}

#[test]
fn usage_brief_is_none_for_untracked_kinds() {
    let (_d, lib) = lib();
    add(&lib, "sk", "skill", 100, None);
    add(&lib, "ru", "rule", 100, None);
    used(&lib, "a", "skill", "sk", 3);
    used(&lib, "b", "skill", "sk", 10);
    used(&lib, "c", "skill", "sk", 100);
    let page = lib.list(&Query::default()).expect("list");
    let find = |id: &str| page.items.iter().find(|i| i.id == id).expect("item");
    let brief = find("sk").usage.clone().expect("tracked");
    assert_eq!(brief.sessions, 2);
    assert!(brief.last_used.is_some());
    assert_eq!(find("ru").usage, None);
}

#[test]
fn usage_without_any_use_counts_zero_sessions() {
    let (_d, lib) = lib();
    add(&lib, "sk", "skill", 100, None);
    let page = lib.list(&Query::default()).expect("list");
    assert_eq!(
        page.items[0].usage,
        Some(UsageBrief {
            sessions: 0,
            last_used: None
        })
    );
    assert_eq!(page.items[0].status, Status::Unused);
}

#[test]
fn usage_sorts_by_sessions_with_untracked_last() {
    let (_d, lib) = lib();
    add(&lib, "a", "skill", 100, None);
    add(&lib, "b", "skill", 100, None);
    add(&lib, "r", "rule", 100, None);
    used(&lib, "1", "skill", "b", 1);
    used(&lib, "2", "skill", "b", 2);
    used(&lib, "1", "skill", "a", 1);
    let ids = |descending| {
        let q = Query {
            sort: SortKey::Sessions,
            descending,
            ..Query::default()
        };
        lib.list(&q)
            .expect("list")
            .items
            .into_iter()
            .map(|i| i.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(true), ["b", "a", "r"]);
    assert_eq!(ids(false), ["a", "b", "r"]);
}

#[test]
fn usage_filters_and_counts_by_status() {
    let (_d, lib) = lib();
    add(&lib, "old", "skill", 100, None);
    add(&lib, "fresh", "skill", 1, None);
    add(&lib, "busy", "skill", 100, None);
    used(&lib, "s", "skill", "busy", 1);
    let q = Query {
        filter: Filter {
            statuses: vec![Status::Unused, Status::NotUsedYet],
            ..Filter::default()
        },
        ..Query::default()
    };
    let page = lib.list(&q).expect("list");
    let mut ids: Vec<_> = page.items.iter().map(|i| i.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["fresh", "old"]);
    assert_eq!(page.total, 2);
    // The status facet ignores its own filter, so every status keeps its count.
    let counts: Vec<_> = page
        .facets
        .statuses
        .iter()
        .map(|c| (c.status, c.count))
        .collect();
    assert_eq!(
        counts,
        [
            (Status::Unused, 1),
            (Status::NotUsedYet, 1),
            (Status::Current, 1),
        ]
    );
}

#[test]
fn usage_status_rule_works_in_a_smart_collection() {
    let (_d, lib) = lib();
    add(&lib, "old", "skill", 100, None);
    add(&lib, "busy", "skill", 100, None);
    used(&lib, "s", "skill", "busy", 1);
    let rule = Filter {
        statuses: vec![Status::Unused],
        ..Filter::default()
    };
    let c = lib
        .create_smart_collection("clean up", rule)
        .expect("create");
    assert_eq!(c.count, 1);
    let page = lib.collection_page(c.id, &Query::default()).expect("page");
    assert_eq!(page.items[0].id, "old");
}

#[test]
fn usage_page_with_usage_stays_under_100_ms_at_2000_artifacts() {
    let (_d, lib) = lib();
    {
        let conn = &lib.store.conn;
        conn.execute_batch("BEGIN").expect("begin");
        for i in 0..2000 {
            let kind = ["skill", "command", "agent", "rule"][i % 4];
            conn.execute(
                "INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
                 VALUES (?1, 's', ?2, ?3, 'p', ?4, '{\"diagnostics\":[]}')",
                params![format!("id{i:05}"), kind, format!("n{i}"), now() - 100 * DAY],
            )
            .expect("artifact");
        }
        for i in 0..2000 {
            for s in 0..6 {
                conn.execute(
                    "INSERT OR IGNORE INTO usage.uses VALUES (?1, date('now', ?2), '/p', ?3, ?4)",
                    params![
                        format!("s{s}"),
                        format!("-{} days", (i + s) % 60),
                        ["skill", "command", "agent", "rule"][i % 4],
                        format!("n{i}")
                    ],
                )
                .expect("use");
            }
        }
        conn.execute_batch("COMMIT").expect("commit");
    }
    // Best of three, so a loaded machine does not fail a budget the query meets.
    let best = |q: &Query| {
        (0..3)
            .map(|_| {
                let started = Instant::now();
                let page = lib.list(q).expect("list");
                (started.elapsed(), page.total)
            })
            .min()
            .expect("runs")
    };
    for sort in [SortKey::Name, SortKey::Sessions] {
        let q = Query {
            sort,
            descending: true,
            ..Query::default()
        };
        let (took, total) = best(&q);
        assert_eq!(total, 2000);
        assert!(
            took < crate::test_support::budget(std::time::Duration::from_millis(100)),
            "{sort:?} took {took:?}"
        );
    }
    let filtered = Query {
        filter: Filter {
            statuses: vec![Status::Unused],
            ..Filter::default()
        },
        ..Query::default()
    };
    let (took, _) = best(&filtered);
    assert!(
        took < crate::test_support::budget(std::time::Duration::from_millis(100)),
        "status filter took {took:?}"
    );
}
