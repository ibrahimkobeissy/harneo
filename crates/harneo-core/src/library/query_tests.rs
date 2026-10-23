//! Tests for `Library::list`. Rows are inserted straight into the index.

use std::time::Instant;

use rusqlite::params;

use super::query::filter_clause;
use super::*;
use crate::artifact::ArtifactKind;

struct Row<'a> {
    id: &'a str,
    source: &'a str,
    kind: &'a str,
    name: &'a str,
    desc: Option<&'a str>,
    at: i64,
}

fn lib() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().expect("dir");
    let lib = Library::open(dir.path()).expect("open");
    (dir, lib)
}

fn source(lib: &Library, id: &str, kind: &str, location: &str) {
    lib.store
        .conn
        .execute(
            "INSERT OR IGNORE INTO sources (id, kind, location, branch) VALUES (?1, ?2, ?3, ?4)",
            params![id, kind, location, (kind == "git").then_some("main")],
        )
        .expect("source");
}

fn add(lib: &Library, r: &Row) {
    lib.store
        .conn
        .execute(
            "INSERT INTO artifacts (id, source_id, kind, name, description, source_path, imported_at, artifact)
             VALUES (?1, ?2, ?3, ?4, ?5, 'p', ?6, '{\"diagnostics\":[]}')",
            params![r.id, r.source, r.kind, r.name, r.desc, r.at],
        )
        .expect("artifact");
}

fn tag(lib: &Library, name: &str, artifacts: &[&str]) {
    lib.store
        .conn
        .execute(
            "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, '#abc')",
            params![name],
        )
        .expect("tag");
    for a in artifacts {
        lib.store
            .conn
            .execute(
                "INSERT INTO artifact_tags (artifact_id, tag_id)
                 SELECT ?1, id FROM tags WHERE name = ?2",
                params![a, name],
            )
            .expect("link");
    }
}

/// Five artifacts over two sources and three tags.
fn fixture() -> (tempfile::TempDir, Library) {
    let (dir, lib) = lib();
    source(&lib, "s1", "local", "/home/me/work");
    source(&lib, "s2", "git", "https://github.com/acme/kit.git");
    for r in [
        Row {
            id: "a1",
            source: "s1",
            kind: "agent",
            name: "Reviewer",
            desc: Some("Reviews code"),
            at: 50,
        },
        Row {
            id: "a2",
            source: "s1",
            kind: "skill",
            name: "deploy",
            desc: None,
            at: 40,
        },
        Row {
            id: "a3",
            source: "s2",
            kind: "skill",
            name: "Audit",
            desc: Some("100% safe_ty"),
            at: 30,
        },
        Row {
            id: "a4",
            source: "s2",
            kind: "rule",
            name: "audit-log",
            desc: Some("Logs"),
            at: 30,
        },
        Row {
            id: "a5",
            source: "s1",
            kind: "mcp",
            name: "Zed",
            desc: None,
            at: 10,
        },
    ] {
        add(&lib, &r);
    }
    tag(&lib, "Review", &["a1", "a3"]);
    tag(&lib, "ops", &["a2", "a3", "a4"]);
    tag(&lib, "unused", &[]);
    (dir, lib)
}

fn ids(page: &Page) -> Vec<&str> {
    page.items.iter().map(|i| i.id.as_str()).collect()
}

fn q(filter: Filter) -> Query {
    Query {
        filter,
        ..Query::default()
    }
}

fn sorted(sort: SortKey, descending: bool) -> Query {
    Query {
        sort,
        descending,
        ..Query::default()
    }
}

#[test]
fn default_query_lists_everything_by_name() {
    let (_d, lib) = fixture();
    let page = lib.list(&Query::default()).expect("list");
    assert_eq!(ids(&page), ["a3", "a4", "a2", "a1", "a5"]);
    assert_eq!(page.total, 5);
}

#[test]
fn items_carry_what_a_card_needs() {
    let (_d, lib) = fixture();
    let page = lib
        .list(&q(Filter {
            text: Some("safe".into()),
            ..Filter::default()
        }))
        .expect("list");
    let item = &page.items[0];
    assert_eq!(item.id, "a3");
    assert_eq!(item.kind, ArtifactKind::Skill);
    assert_eq!(item.source.label, "acme/kit");
    assert_eq!(item.source.branch.as_deref(), Some("main"));
    let tags: Vec<&str> = item.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tags, ["ops", "Review"]);
    assert_eq!(item.tags[0].color, "#abc");
    assert_eq!(item.imported_at, 30);
}

#[test]
fn diagnostic_counts_come_from_the_stored_artifact() {
    let (_d, lib) = fixture();
    lib.store
        .conn
        .execute(
            "UPDATE artifacts SET artifact = ?1 WHERE id = 'a1'",
            params![r#"{"diagnostics":[{"severity":"error"},{"severity":"warning"},{"severity":"warning"}]}"#],
        )
        .expect("update");
    let page = lib
        .list(&q(Filter {
            text: Some("reviewer".into()),
            ..Filter::default()
        }))
        .expect("list");
    assert_eq!(
        (page.items[0].error_count, page.items[0].warning_count),
        (1, 2)
    );
}

#[test]
fn file_and_reference_counts_come_from_the_stored_artifact() {
    let (_d, lib) = fixture();
    lib.store
        .conn
        .execute(
            "UPDATE artifacts SET artifact = ?1 WHERE id = 'a1'",
            params![r#"{"files":["a.md","b.md","c.md"],"references":[{"resolved":true},{"resolved":false},{"resolved":true}]}"#],
        )
        .expect("update");
    let page = lib
        .list(&q(Filter {
            text: Some("reviewer".into()),
            ..Filter::default()
        }))
        .expect("list");
    let i = &page.items[0];
    assert_eq!(
        (i.file_count, i.reference_count, i.resolved_references),
        (3, 3, 2)
    );
    assert_eq!(page.unfiltered_total, 5);
    assert_eq!(page.total, 1);
}

#[test]
fn filter_by_types_ors_within() {
    let (_d, lib) = fixture();
    let f = Filter {
        types: vec![ArtifactKind::Rule, ArtifactKind::Mcp],
        ..Filter::default()
    };
    let page = lib.list(&q(f)).expect("list");
    assert_eq!(ids(&page), ["a4", "a5"]);
}

#[test]
fn filter_by_tags_ors_within_and_ignores_case() {
    let (_d, lib) = fixture();
    let f = Filter {
        tags: vec!["REVIEW".into(), "unused".into()],
        ..Filter::default()
    };
    let page = lib.list(&q(f)).expect("list");
    assert_eq!(ids(&page), ["a3", "a1"]);
    assert_eq!(page.total, 2);
}

#[test]
fn filter_by_sources() {
    let (_d, lib) = fixture();
    let f = Filter {
        sources: vec!["s2".into()],
        ..Filter::default()
    };
    assert_eq!(ids(&lib.list(&q(f)).expect("list")), ["a3", "a4"]);
}

#[test]
fn filters_and_across() {
    let (_d, lib) = fixture();
    let f = Filter {
        types: vec![ArtifactKind::Skill],
        tags: vec!["ops".into()],
        sources: vec!["s2".into()],
        text: None,
        ..Filter::default()
    };
    assert_eq!(ids(&lib.list(&q(f)).expect("list")), ["a3"]);
}

#[test]
fn text_matches_name_description_source_and_tags_case_insensitively() {
    let (_d, lib) = fixture();
    let by = |t: &str| {
        let page = lib
            .list(&q(Filter {
                text: Some(t.into()),
                ..Filter::default()
            }))
            .expect("list");
        ids(&page)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(by("DEPLOY"), ["a2"], "name");
    assert_eq!(by("reviews c"), ["a1"], "description");
    assert_eq!(by("WORK"), ["a2", "a1", "a5"], "local source basename");
    assert_eq!(by("acme/kit"), ["a3", "a4"], "git source");
    assert_eq!(by("rEvIeW"), ["a3", "a1"], "tag name and name/description");
    assert_eq!(
        by("home"),
        Vec::<String>::new(),
        "parent folders are not the label"
    );
    assert_eq!(
        by("  "),
        ["a3", "a4", "a2", "a1", "a5"],
        "blank text is no filter"
    );
}

#[test]
fn text_wildcards_are_literal() {
    let (_d, lib) = fixture();
    let by = |t: &str| {
        let page = lib
            .list(&q(Filter {
                text: Some(t.into()),
                ..Filter::default()
            }))
            .expect("list");
        ids(&page)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(by("%"), ["a3"]);
    assert_eq!(by("e_t"), ["a3"]);
    assert_eq!(by("_"), ["a3"]);
    assert!(by("a%y").is_empty());
    assert!(by("\\").is_empty());
    assert!(by("'; DROP TABLE artifacts; --").is_empty());
    assert_eq!(lib.list(&Query::default()).expect("still there").total, 5);
}

#[test]
fn every_sort_key_and_direction() {
    let (_d, lib) = fixture();
    let run = |s, d| {
        let page = lib.list(&sorted(s, d)).expect("list");
        ids(&page)
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(run(SortKey::Name, false), ["a3", "a4", "a2", "a1", "a5"]);
    assert_eq!(run(SortKey::Name, true), ["a5", "a1", "a2", "a4", "a3"]);
    // mcp < agent? stored names: agent, mcp, rule, skill, skill
    assert_eq!(run(SortKey::Kind, false), ["a1", "a5", "a4", "a2", "a3"]);
    assert_eq!(run(SortKey::Kind, true), ["a2", "a3", "a4", "a5", "a1"]);
    // labels: kit... "acme/kit" < "work"
    assert_eq!(run(SortKey::Source, false), ["a3", "a4", "a1", "a2", "a5"]);
    assert_eq!(run(SortKey::Source, true), ["a1", "a2", "a5", "a3", "a4"]);
    // tag counts: a3=2; a1,a2,a4=1; a5=0
    assert_eq!(
        run(SortKey::TagCount, false),
        ["a5", "a1", "a2", "a4", "a3"]
    );
    assert_eq!(run(SortKey::TagCount, true), ["a3", "a1", "a2", "a4", "a5"]);
    // imported: a5=10, a3=a4=30, a2=40, a1=50; ties by id ascending in both directions
    assert_eq!(
        run(SortKey::ImportedAt, false),
        ["a5", "a3", "a4", "a2", "a1"]
    );
    assert_eq!(
        run(SortKey::ImportedAt, true),
        ["a1", "a2", "a3", "a4", "a5"]
    );
}

#[test]
fn pages_and_total_at_the_edges() {
    let (_d, lib) = fixture();
    let page = |offset, limit| {
        lib.list(&Query {
            offset,
            limit,
            ..Query::default()
        })
        .expect("list")
    };
    let first = page(0, 2);
    assert_eq!((ids(&first), first.total), (vec!["a3", "a4"], 5));
    assert_eq!(ids(&page(2, 2)), ["a2", "a1"]);
    assert_eq!(ids(&page(4, 2)), ["a5"], "short last page");
    let past = page(5, 2);
    assert!(past.items.is_empty());
    assert_eq!(past.total, 5, "total survives an empty page");
    assert_eq!(past.facets.kinds.iter().map(|k| k.count).sum::<u32>(), 5);
    let zero = page(0, 0);
    assert!(zero.items.is_empty());
    assert_eq!(zero.total, 5);
    assert_eq!(page(0, 5).items.len(), 5);
    assert_eq!(
        page(0, u32::MAX).items.len(),
        5,
        "huge limit is clamped, not an error"
    );
    assert_eq!(page(u32::MAX, 5).items.len(), 0);
}

#[test]
fn empty_library_and_no_match() {
    let (_d, lib) = lib();
    let page = lib.list(&Query::default()).expect("list");
    assert_eq!((page.items.len(), page.total), (0, 0));
    assert_eq!(page.facets, Facets::default());
    let (_d, lib) = fixture();
    let none = lib
        .list(&q(Filter {
            text: Some("zzz".into()),
            ..Filter::default()
        }))
        .expect("list");
    assert_eq!(none.total, 0);
    assert!(none.facets.kinds.is_empty());
    assert!(none.facets.tags.iter().all(|t| t.count == 0));
}

fn kinds(page: &Page) -> Vec<(ArtifactKind, u32)> {
    page.facets
        .kinds
        .iter()
        .map(|k| (k.kind, k.count))
        .collect()
}
fn tags(page: &Page) -> Vec<(&str, u32)> {
    page.facets
        .tags
        .iter()
        .map(|t| (t.name.as_str(), t.count))
        .collect()
}
fn sources(page: &Page) -> Vec<(&str, u32)> {
    page.facets
        .sources
        .iter()
        .map(|s| (s.source.id.as_str(), s.count))
        .collect()
}

#[test]
fn unfiltered_facets_count_everything() {
    let (_d, lib) = fixture();
    let page = lib.list(&Query::default()).expect("list");
    assert_eq!(
        kinds(&page),
        [
            (ArtifactKind::Rule, 1),
            (ArtifactKind::Skill, 2),
            (ArtifactKind::Agent, 1),
            (ArtifactKind::Mcp, 1)
        ]
    );
    assert_eq!(tags(&page), [("ops", 3), ("Review", 2), ("unused", 0)]);
    assert_eq!(sources(&page), [("s2", 2), ("s1", 3)]);
    assert_eq!(page.facets.sources[0].source.label, "acme/kit");
}

#[test]
fn a_facet_ignores_its_own_filter_but_obeys_the_others() {
    let (_d, lib) = fixture();
    // Tag "ops" selected: kinds/sources shrink to ops artifacts; tag counts do not shrink to it.
    let f = Filter {
        tags: vec!["ops".into()],
        ..Filter::default()
    };
    let page = lib.list(&q(f)).expect("list");
    assert_eq!(page.total, 3);
    assert_eq!(
        kinds(&page),
        [(ArtifactKind::Rule, 1), (ArtifactKind::Skill, 2)]
    );
    assert_eq!(sources(&page), [("s2", 2), ("s1", 1)]);
    assert_eq!(tags(&page), [("ops", 3), ("Review", 2), ("unused", 0)]);

    // Type + source selected: the tag facet counts within both, the kind facet within source only.
    let f = Filter {
        types: vec![ArtifactKind::Skill],
        sources: vec!["s2".into()],
        ..Filter::default()
    };
    let page = lib.list(&q(f)).expect("list");
    assert_eq!(ids(&page), ["a3"]);
    assert_eq!(tags(&page), [("ops", 1), ("Review", 1), ("unused", 0)]);
    assert_eq!(
        kinds(&page),
        [(ArtifactKind::Rule, 1), (ArtifactKind::Skill, 1)]
    );
    assert_eq!(sources(&page), [("s2", 1), ("s1", 1)]);
}

#[test]
fn facets_follow_the_text_filter_and_keep_selected_zero_values() {
    let (_d, lib) = fixture();
    let f = Filter {
        types: vec![ArtifactKind::Plugin],
        text: Some("audit".into()),
        ..Filter::default()
    };
    let page = lib.list(&q(f)).expect("list");
    assert_eq!(page.total, 0);
    assert_eq!(
        kinds(&page),
        [
            (ArtifactKind::Rule, 1),
            (ArtifactKind::Skill, 1),
            (ArtifactKind::Plugin, 0)
        ]
    );
    assert_eq!(tags(&page), [("ops", 0), ("Review", 0), ("unused", 0)]);
}

#[test]
fn filter_clause_is_reusable_and_only_binds_values() {
    let (clause, params) = filter_clause(&Filter::default()).expect("clause");
    assert_eq!((clause.as_str(), params.len()), ("1", 0));
    let f = Filter {
        types: vec![ArtifactKind::Skill],
        tags: vec!["a".into(), "b".into()],
        sources: vec!["x".into()],
        text: Some("it's".into()),
        ..Filter::default()
    };
    let (clause, params) = filter_clause(&f).expect("clause");
    assert!(!clause.contains("it's") && !clause.contains("skill"));
    assert_eq!(params.len(), 1 + 2 + 1 + 4);
    let (_d, lib) = fixture();
    let n: i64 = lib
        .store
        .conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id WHERE {clause}"
            ),
            rusqlite::params_from_iter(params.iter()),
            |r| r.get(0),
        )
        .expect("runs");
    assert_eq!(n, 0);
}

#[test]
fn query_round_trips_over_ipc_with_defaults() {
    let q: Query = serde_json::from_str(r#"{"sort":"tagCount","descending":true}"#).expect("json");
    assert_eq!(q.sort, SortKey::TagCount);
    assert_eq!(q.limit, 50);
    let back: Query = serde_json::from_str(&serde_json::to_string(&q).expect("ser")).expect("de");
    assert_eq!(back, q);
}

#[test]
fn a_2000_artifact_library_answers_a_filtered_sorted_page_under_100_ms() {
    let (_d, lib) = lib();
    for s in 0..10 {
        source(&lib, &format!("s{s}"), "local", &format!("/w/project{s}"));
    }
    let kinds = ["agent", "skill", "rule", "command", "mcp"];
    let conn = &lib.store.conn;
    conn.execute_batch("BEGIN").expect("begin");
    for i in 0..2000 {
        conn.execute(
            "INSERT INTO artifacts (id, source_id, kind, name, description, source_path, imported_at, artifact)
             VALUES (?1, ?2, ?3, ?4, ?5, 'p', ?6, '{\"diagnostics\":[]}')",
            params![
                format!("{i:016x}"),
                format!("s{}", i % 10),
                kinds[i % 5],
                format!("artifact-{i}"),
                format!("does thing number {i}"),
                i as i64
            ],
        )
        .expect("insert");
    }
    for t in 0..20 {
        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?1, '#abc')",
            params![format!("tag{t}")],
        )
        .expect("tag");
    }
    conn.execute(
        "INSERT INTO artifact_tags SELECT a.id, t.id FROM artifacts a JOIN tags t
         ON (CAST(substr(a.id, 13, 4) AS INTEGER) + t.id) % 3 = 0",
        [],
    )
    .expect("link");
    conn.execute_batch("COMMIT").expect("commit");

    let query = Query {
        filter: Filter {
            types: vec![ArtifactKind::Skill, ArtifactKind::Agent],
            tags: vec!["tag3".into(), "tag4".into()],
            sources: vec!["s1".into(), "s2".into(), "s3".into()],
            text: Some("thing".into()),
            ..Filter::default()
        },
        sort: SortKey::TagCount,
        descending: true,
        offset: 10,
        limit: 25,
    };
    // Best of five: the bar is the query's own speed, not the load other tests or builds put
    // on the machine at the same moment (the threshold itself stays 100 ms).
    let mut elapsed = std::time::Duration::MAX;
    let mut page = lib.list(&query).expect("list");
    for _ in 0..5 {
        let start = Instant::now();
        page = lib.list(&query).expect("list");
        elapsed = elapsed.min(start.elapsed());
    }
    assert!(
        page.total > 25,
        "the filter should leave a few hundred rows: {}",
        page.total
    );
    assert_eq!(page.items.len(), 25);
    assert!(
        elapsed < crate::test_support::budget(std::time::Duration::from_millis(100)),
        "list took {elapsed:?} on 2,000 artifacts"
    );
}
