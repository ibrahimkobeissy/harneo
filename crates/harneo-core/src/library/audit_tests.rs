//! The audit inside the library: preview carries it, import stores it, re-audit refreshes it,
//! and the list filters, sorts and counts by it.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use rusqlite::params;

use super::*;
use crate::artifact::ArtifactKind;
use crate::audit::Tier;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

/// A clean agent, a skill with a hidden character in its SKILL.md and a clean script.
fn fixture(root: &Path) -> io::Result<()> {
    write(
        root,
        "agents/scout.md",
        "---\nname: scout\ndescription: Finds things\n---\nLook around.\n",
    )?;
    write(
        root,
        "skills/notes/SKILL.md",
        &format!(
            "---\nname: notes\ndescription: Takes notes\n---\nWrite.{}\n",
            '\u{200B}'
        ),
    )?;
    write(root, "skills/notes/scripts/run.sh", "echo notes\n")
}

struct Setup {
    data: tempfile::TempDir,
    source_dir: tempfile::TempDir,
    library: Library,
    source: Source,
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    fixture(source_dir.path())?;
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: source_dir.path().to_path_buf(),
    };
    Ok(Setup {
        data,
        source_dir,
        library,
        source,
    })
}

fn by_name<'a>(preview: &'a Preview, name: &str) -> Result<&'a Candidate> {
    preview
        .candidates
        .iter()
        .find(|c| c.artifact.name == name)
        .ok_or_else(|| Error::NotInPreview {
            id: name.to_owned(),
        })
}

fn import_all(s: &mut Setup) -> std::result::Result<Preview, Box<dyn std::error::Error>> {
    let preview = s.library.preview(&s.source)?;
    let ids: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    Ok(preview)
}

#[test]
fn preview_carries_the_audit_and_import_keeps_it_across_a_reopen() -> TestResult {
    let mut s = setup()?;
    let preview = import_all(&mut s)?;
    let scout = by_name(&preview, "scout")?;
    let notes = by_name(&preview, "notes")?;
    assert_eq!((scout.audit.score, scout.audit.tier), (100, Tier::Trusted));
    assert!(scout.audit.findings.is_empty());
    assert_eq!(notes.audit.findings.len(), 1);
    assert_eq!(notes.audit.findings[0].rule, "hidden-unicode");
    assert_eq!(notes.audit.findings[0].file, "skills/notes/SKILL.md");
    assert_eq!(notes.audit.score, 85);

    let detail = s.library.detail(&notes.id)?;
    assert_eq!(detail.audit.as_ref(), Some(&notes.audit));
    assert!(detail.audited_at.is_some());

    let data = s.data.path().to_path_buf();
    drop(s.library);
    let reopened = Library::open(&data)?;
    assert_eq!(
        reopened.detail(&notes.id)?.audit.as_ref(),
        Some(&notes.audit)
    );
    let page = reopened.list(&Query::default())?;
    let item = page.items.iter().find(|i| i.id == notes.id).ok_or("item")?;
    assert_eq!(
        (item.trust_score, item.tier, item.finding_count),
        (Some(85), Some(Tier::Trusted), 1)
    );
    Ok(())
}

#[test]
fn reaudit_refreshes_stale_scores_from_the_stored_copy_and_settles() -> TestResult {
    let mut s = setup()?;
    let preview = import_all(&mut s)?;
    let notes = by_name(&preview, "notes")?.id.clone();
    let scout = by_name(&preview, "scout")?.id.clone();

    // Nothing changed since import: the audit of the copy equals the audit of the source.
    let same = s.library.reaudit_all()?;
    assert_eq!((same.audited, same.changed), (2, 0));
    assert_eq!(
        s.library.detail(&notes)?.audit.map(|a| a.findings),
        Some(by_name(&preview, "notes")?.audit.findings.clone()),
        "findings keep their source paths"
    );

    // A pre-audit row (v1 data) counts as changed, a stale score is corrected.
    s.library.store.conn.execute(
        "UPDATE artifacts SET trust_score = NULL, tier = NULL, findings = NULL, reach = NULL,
                audited_at = NULL WHERE id = ?1",
        params![scout],
    )?;
    s.library.store.conn.execute(
        "UPDATE artifacts SET trust_score = 3 WHERE id = ?1",
        params![notes],
    )?;
    assert!(s.library.detail(&scout)?.audit.is_none());
    let report = s.library.reaudit_all()?;
    assert_eq!((report.audited, report.changed), (2, 2));
    assert_eq!(s.library.detail(&scout)?.audit.map(|a| a.score), Some(100));
    assert_eq!(s.library.detail(&notes)?.audit.map(|a| a.score), Some(85));

    // Editing the stored copy changes the result.
    fs::write(
        s.data
            .path()
            .join("library/artifacts")
            .join(&scout)
            .join("scout.md"),
        format!("---\nname: scout\n---\nHi{}\n", '\u{202E}'),
    )?;
    let report = s.library.reaudit_all()?;
    assert_eq!((report.audited, report.changed), (2, 1));
    assert!(s
        .library
        .detail(&scout)?
        .audit
        .is_some_and(|a| a.score < 100));
    let _ = &s.source_dir;
    Ok(())
}

#[test]
fn reaudit_skips_an_artifact_whose_copy_is_missing() -> TestResult {
    let mut s = setup()?;
    let preview = import_all(&mut s)?;
    let scout = by_name(&preview, "scout")?.id.clone();
    fs::remove_dir_all(s.data.path().join("library/artifacts").join(&scout))?;
    let report = s.library.reaudit_all()?;
    assert_eq!(report.audited, 1);
    Ok(())
}

/// Sets the audit columns straight in the index: `(id, score, tier, finding count)`.
fn set_audit(lib: &Library, id: &str, score: Option<u8>, tier: Option<&str>, findings: usize) {
    let json = serde_json::to_string(&vec![0; findings]).unwrap_or_default();
    let _ = lib.store.conn.execute(
        "UPDATE artifacts SET trust_score = ?2, tier = ?3, findings = ?4 WHERE id = ?1",
        params![id, score, tier, score.map(|_| json)],
    );
}

fn seeded() -> std::result::Result<(Setup, Vec<String>), Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    for (name, _) in [("a", 0), ("b", 0), ("c", 0), ("d", 0)] {
        write(
            source_dir.path(),
            &format!("agents/{name}.md"),
            &format!("---\nname: {name}\ndescription: x\n---\nGo.\n"),
        )?;
    }
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: source_dir.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<ArtifactId> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let id_of = |n: &str| by_name(&preview, n).map(|c| c.id.clone());
    let (a, b, c, d) = (id_of("a")?, id_of("b")?, id_of("c")?, id_of("d")?);
    set_audit(&library, &a, Some(95), Some("trusted"), 0);
    set_audit(&library, &b, Some(70), Some("review"), 2);
    set_audit(&library, &c, Some(20), Some("dangerous"), 5);
    set_audit(&library, &d, None, None, 0);
    Ok((
        Setup {
            data,
            source_dir,
            library,
            source,
        },
        vec![a, b, c, d],
    ))
}

fn names(page: &Page) -> Vec<&str> {
    page.items.iter().map(|i| i.name.as_str()).collect()
}

#[test]
fn trust_filters_and_unaudited_rows_never_match() -> TestResult {
    let (s, _) = seeded()?;
    let list = |filter: Filter| {
        s.library.list(&Query {
            filter,
            ..Query::default()
        })
    };
    assert_eq!(
        names(&list(Filter {
            trust_min: Some(70),
            ..Filter::default()
        })?),
        ["a", "b"]
    );
    assert_eq!(
        names(&list(Filter {
            trust_min: Some(0),
            ..Filter::default()
        })?),
        ["a", "b", "c"]
    );
    assert_eq!(
        names(&list(Filter {
            tiers: vec![Tier::Review, Tier::Dangerous],
            ..Filter::default()
        })?),
        ["b", "c"]
    );
    assert_eq!(
        names(&list(Filter {
            has_findings: true,
            ..Filter::default()
        })?),
        ["b", "c"]
    );
    let both = Filter {
        has_findings: true,
        trust_min: Some(50),
        ..Filter::default()
    };
    assert_eq!(names(&list(both)?), ["b"]);
    Ok(())
}

#[test]
fn sort_by_trust_puts_unaudited_last_both_ways() -> TestResult {
    let (s, _) = seeded()?;
    let sorted = |descending| {
        s.library.list(&Query {
            sort: SortKey::Trust,
            descending,
            ..Query::default()
        })
    };
    assert_eq!(names(&sorted(false)?), ["c", "b", "a", "d"]);
    assert_eq!(names(&sorted(true)?), ["a", "b", "c", "d"]);
    Ok(())
}

#[test]
fn tier_facets_count_over_the_rest_of_the_filter() -> TestResult {
    let (s, _) = seeded()?;
    let page = s.library.list(&Query::default())?;
    let tiers: Vec<(Tier, u32)> = page
        .facets
        .tiers
        .iter()
        .map(|t| (t.tier, t.count))
        .collect();
    assert_eq!(
        tiers,
        [(Tier::Trusted, 1), (Tier::Review, 1), (Tier::Dangerous, 1)]
    );
    // Picking a tier does not hide the others; the other filters still narrow the counts.
    let picked = s.library.list(&Query {
        filter: Filter {
            tiers: vec![Tier::Trusted, Tier::Risky],
            has_findings: true,
            ..Filter::default()
        },
        ..Query::default()
    })?;
    assert_eq!(picked.total, 0);
    let tiers: Vec<(Tier, u32)> = picked
        .facets
        .tiers
        .iter()
        .map(|t| (t.tier, t.count))
        .collect();
    assert_eq!(
        tiers,
        [
            (Tier::Trusted, 0),
            (Tier::Review, 1),
            (Tier::Risky, 0),
            (Tier::Dangerous, 1)
        ]
    );
    Ok(())
}

#[test]
fn a_smart_collection_can_rule_on_trust_alone() -> TestResult {
    let (s, _) = seeded()?;
    let collection = s.library.create_smart_collection(
        "risky stuff",
        Filter {
            tiers: vec![Tier::Dangerous],
            ..Filter::default()
        },
    )?;
    let page = s
        .library
        .collection_page(collection.id, &Query::default())?;
    assert_eq!(names(&page), ["c"]);
    let _ = (&s.data, &s.source_dir, &s.source);
    Ok(())
}

#[test]
fn listing_2000_audited_artifacts_by_trust_stays_fast() -> TestResult {
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let conn = &library.store.conn;
    conn.execute_batch(
        "INSERT INTO sources (id, kind, location) VALUES ('s', 'local', '/x'); BEGIN",
    )?;
    for n in 0..2000 {
        let tier = ["trusted", "review", "risky", "dangerous"][n % 4];
        conn.execute(
            "INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact,
                                    trust_score, tier, findings, reach, audited_at)
             VALUES (?1, 's', 'agent', ?2, 'p', 0, '{\"diagnostics\":[]}', ?3, ?4, '[1]', '{}', 0)",
            params![
                format!("id{n:05}"),
                format!("thing {n}"),
                (n % 101) as i64,
                tier
            ],
        )?;
    }
    conn.execute_batch("COMMIT")?;
    let query = Query {
        filter: Filter {
            trust_min: Some(30),
            tiers: vec![Tier::Trusted, Tier::Review, Tier::Risky],
            has_findings: true,
            ..Filter::default()
        },
        sort: SortKey::Trust,
        offset: 10,
        limit: 25,
        ..Query::default()
    };
    let mut elapsed = std::time::Duration::MAX;
    let mut page = library.list(&query)?;
    for _ in 0..5 {
        let start = Instant::now();
        page = library.list(&query)?;
        elapsed = elapsed.min(start.elapsed());
    }
    assert_eq!(page.items.len(), 25);
    assert!(page.items.iter().all(|i| i.kind == ArtifactKind::Agent));
    assert!(elapsed.as_millis() < 100, "list took {elapsed:?}");
    Ok(())
}
