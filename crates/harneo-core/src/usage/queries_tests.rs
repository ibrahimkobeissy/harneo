use rusqlite::params;

use super::{day_number, week_bucket, UsageIndex, WEEKS};
use crate::artifact::ArtifactKind;

fn setup() -> (tempfile::TempDir, UsageIndex) {
    let dir = tempfile::tempdir().expect("temp dir");
    let index = UsageIndex::open(dir.path()).expect("open");
    (dir, index)
}

fn add(index: &UsageIndex, session: &str, day: &str, kind: &str, name: &str, project: &str) {
    index
        .connect()
        .expect("conn")
        .execute(
            "INSERT OR IGNORE INTO uses VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session, day, project, kind, name],
        )
        .expect("insert");
}

#[test]
fn day_numbers_follow_the_calendar() {
    assert_eq!(day_number("1970-01-01"), Some(0));
    assert_eq!(day_number("2000-03-01"), Some(11_017));
    assert_eq!(day_number("2026-10-23"), Some(20_749));
    assert_eq!(day_number("2026-13-01"), None);
    assert_eq!(day_number("nope"), None);
}

#[test]
fn weeks_run_oldest_first_and_stop_at_eight() {
    assert_eq!(week_bucket(100, 100), Some(WEEKS - 1));
    assert_eq!(week_bucket(100, 94), Some(WEEKS - 1));
    assert_eq!(week_bucket(100, 93), Some(WEEKS - 2));
    assert_eq!(week_bucket(100, 100 - 55), Some(0));
    assert_eq!(week_bucket(100, 100 - 56), None);
    assert_eq!(
        week_bucket(100, 105),
        Some(WEEKS - 1),
        "clock skew counts as today"
    );
}

#[test]
fn usage_of_buckets_sessions_per_week_and_keeps_old_last_used() {
    let (_dir, index) = setup();
    let today = day_number("2026-10-23").expect("day");
    add(&index, "a", "2026-10-23", "skill", "x", "/p1");
    add(&index, "b", "2026-10-22", "skill", "x", "/p2");
    add(&index, "b", "2026-10-21", "skill", "x", "/p2");
    add(&index, "c", "2026-10-10", "skill", "x", "");
    add(&index, "d", "2026-08-01", "skill", "x", "/old");
    add(&index, "e", "2026-10-23", "agent", "x", "/p1");
    let u = index
        .usage_of_at(ArtifactKind::Skill, "x", today)
        .expect("usage");
    assert_eq!(u.sessions_8w, 3);
    assert_eq!(u.weekly, [0, 0, 0, 0, 0, 0, 1, 2]);
    assert_eq!(u.last_used.as_deref(), Some("2026-10-23"));
    assert_eq!(u.projects, vec!["/old", "/p1", "/p2"]);

    let none = index
        .usage_of_at(ArtifactKind::Skill, "ghost", today)
        .expect("usage");
    assert_eq!((none.sessions_8w, none.last_used), (0, None));
    assert!(index.usage_of(ArtifactKind::Skill, "x").is_ok());
}

#[test]
fn only_old_use_gives_a_last_used_but_no_sessions() {
    let (_dir, index) = setup();
    add(&index, "d", "2026-01-01", "skill", "old", "/p");
    let today = day_number("2026-10-23").expect("day");
    let u = index
        .usage_of_at(ArtifactKind::Skill, "old", today)
        .expect("usage");
    assert_eq!(
        (u.sessions_8w, u.last_used.as_deref()),
        (0, Some("2026-01-01"))
    );
}

#[test]
fn the_summary_counts_distinct_sessions_and_the_change() {
    let (_dir, index) = setup();
    let today = day_number("2026-10-23").expect("day");
    let empty = index.usage_summary_at(today).expect("summary");
    assert_eq!((empty.this_week, empty.change_pct), (0, None));
    // this week: s1 (two kinds, one session), s2; last week: s3.
    add(&index, "s1", "2026-10-23", "skill", "a", "");
    add(&index, "s1", "2026-10-23", "mcp", "b", "");
    add(&index, "s2", "2026-10-20", "command", "c", "");
    add(&index, "s3", "2026-10-14", "agent", "d", "");
    let s = index.usage_summary_at(today).expect("summary");
    assert_eq!((s.this_week, s.change_pct), (2, Some(100)));
    assert_eq!(s.weekly[6], 1);
    assert!(index.usage_summary().is_ok());
}

#[test]
fn a_newer_database_is_refused_and_reopening_keeps_data() {
    let (dir, index) = setup();
    add(&index, "a", "2026-10-23", "skill", "x", "");
    assert_eq!(
        UsageIndex::open(dir.path())
            .expect("reopen")
            .usage_summary_at(20_749)
            .expect("s")
            .this_week,
        1
    );
    index
        .connect()
        .expect("conn")
        .pragma_update(None, "user_version", 99)
        .expect("bump");
    assert!(matches!(
        UsageIndex::open(dir.path()),
        Err(crate::Error::SchemaTooNew { found: 99, .. })
    ));
}
