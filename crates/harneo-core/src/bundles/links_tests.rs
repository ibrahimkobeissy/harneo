use std::fs;
use std::path::Path;

use rusqlite::Connection;

use super::*;
use crate::library::{Query, Source, TagChoices};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn agent(root: &Path, name: &str, skills: &[&str]) -> std::io::Result<()> {
    let list = if skills.is_empty() {
        String::new()
    } else {
        format!("skills: [{}]\n", skills.join(", "))
    };
    write(
        root,
        &format!("agents/{name}.md"),
        &format!("---\nname: {name}\ndescription: {name} agent\n{list}---\nBody.\n"),
    )
}

fn skill(root: &Path, name: &str) -> std::io::Result<()> {
    write(
        root,
        &format!("skills/{name}/SKILL.md"),
        &format!("---\nname: {name}\ndescription: {name} skill\n---\nBody.\n"),
    )
}

fn import_all(
    library: &mut Library,
    root: &Path,
) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
    let source = Source::Local {
        path: root.to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(ids)
}

fn id_of(
    library: &Library,
    kind: ArtifactKind,
    name: &str,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    library
        .find(kind, name)?
        .ok_or_else(|| format!("{name} not in library").into())
}

struct Three {
    _data: tempfile::TempDir,
    _src: tempfile::TempDir,
    library: Library,
    ids: [String; 3],
}

type Made = std::result::Result<Three, Box<dyn std::error::Error>>;

/// Three agents a, b, c with no references, so links are entirely manual.
fn three() -> Made {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    for n in ["a", "b", "c"] {
        agent(src.path(), n, &[])?;
    }
    let mut library = Library::open(data.path())?;
    import_all(&mut library, src.path())?;
    let ids = [
        id_of(&library, ArtifactKind::Agent, "a")?,
        id_of(&library, ArtifactKind::Agent, "b")?,
        id_of(&library, ArtifactKind::Agent, "c")?,
    ];
    Ok(Three {
        _data: data,
        _src: src,
        library,
        ids,
    })
}

#[test]
fn a_v2_database_migrates_to_v3_and_keeps_its_data() -> TestResult {
    let data = tempfile::tempdir()?;
    let root = data.path().join("library");
    fs::create_dir_all(root.join("artifacts"))?;
    {
        let conn = Connection::open(root.join("library.db"))?;
        conn.execute_batch(
            "CREATE TABLE sources (id TEXT PRIMARY KEY, kind TEXT NOT NULL, location TEXT NOT NULL,
               branch TEXT, subfolder TEXT, commit_sha TEXT) STRICT;
             CREATE TABLE artifacts (id TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
               kind TEXT NOT NULL, name TEXT NOT NULL, description TEXT, source_path TEXT NOT NULL,
               digest TEXT, imported_at INTEGER NOT NULL, replaced_secret_keys TEXT NOT NULL DEFAULT '[]',
               artifact TEXT NOT NULL, trust_score INTEGER, tier TEXT, findings TEXT, reach TEXT, audited_at INTEGER) STRICT;
             INSERT INTO sources (id, kind, location) VALUES ('s1', 'local', '/x');
             INSERT INTO artifacts (id, source_id, kind, name, source_path, imported_at, artifact)
               VALUES ('a1', 's1', 'agent', 'old', 'agents/old.md', 5, '{}');
             PRAGMA user_version = 2;",
        )?;
    }
    let library = Library::open(data.path())?;
    assert_eq!(library.schema_version()?, 3);
    let name: String =
        library
            .store
            .conn
            .query_row("SELECT name FROM artifacts WHERE id = 'a1'", [], |r| {
                r.get(0)
            })?;
    assert_eq!(name, "old");
    assert_eq!(library.settings_get("x.y")?, None);
    Ok(())
}

#[test]
fn links_are_directed_and_listed_both_ways() -> TestResult {
    let Three {
        library,
        ids: [a, b, c],
        _data,
        _src,
    } = three()?;
    library.add_link(&a, &b, LinkOrigin::Manual, " needs b ")?;
    library.add_link(&c, &b, LinkOrigin::Ai, "")?;
    let of_b = library.links_of(&b)?;
    assert!(of_b.needs.is_empty());
    let from: Vec<&str> = of_b.needed_by.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(from, ["a", "c"]);
    assert_eq!(of_b.needed_by[0].reason, "needs b");
    assert_eq!(of_b.needed_by[0].origin, LinkOrigin::Manual);
    let of_a = library.links_of(&a)?;
    assert_eq!(of_a.needs.len(), 1);
    assert_eq!(of_a.needs[0].id, b);
    assert!(library.links_of("nope").is_err());
    Ok(())
}

#[test]
fn self_links_and_unknown_artifacts_are_refused() -> TestResult {
    let Three {
        library,
        ids: [a, ..],
        _data,
        _src,
    } = three()?;
    assert!(matches!(
        library.add_link(&a, &a, LinkOrigin::Manual, ""),
        Err(Error::InvalidLink { .. })
    ));
    assert!(matches!(
        library.add_link(&a, "nope", LinkOrigin::Manual, ""),
        Err(Error::NotInLibrary { .. })
    ));
    assert!(library.reject_link(&a, &a).is_err());
    Ok(())
}

#[test]
fn bundle_of_is_transitive_ordered_by_depth_then_name_and_survives_a_cycle() -> TestResult {
    let Three {
        library,
        ids: [a, b, c],
        _data,
        _src,
    } = three()?;
    library.add_link(&a, &c, LinkOrigin::Manual, "")?;
    library.add_link(&c, &b, LinkOrigin::Manual, "")?;
    assert_eq!(library.bundle_of(&a)?, [c.clone(), b.clone()]);
    assert!(library.bundle_of(&b)?.is_empty());
    library.add_link(&b, &a, LinkOrigin::Manual, "")?;
    assert_eq!(library.bundle_of(&a)?, [c.clone(), b.clone()]);
    assert_eq!(library.bundle_of(&b)?, [a.clone(), c.clone()]);
    let page = library.list(&Query::default())?;
    let sizes: Vec<u32> = page.items.iter().map(|i| i.bundle_size).collect();
    assert_eq!(sizes, [2, 2, 2]);
    assert!(library.bundle_of("nope").is_err());
    Ok(())
}

#[test]
fn rejected_links_stay_out_of_bundles_until_added_again() -> TestResult {
    let Three {
        library,
        ids: [a, b, _],
        _data,
        _src,
    } = three()?;
    library.reject_link(&a, &b)?;
    assert!(library.bundle_of(&a)?.is_empty());
    assert!(library.links_of(&a)?.needs.is_empty());
    library.add_link(&a, &b, LinkOrigin::Ai, "because")?;
    assert_eq!(library.bundle_of(&a)?, std::slice::from_ref(&b));
    library.reject_link(&a, &b)?;
    assert!(library.bundle_of(&a)?.is_empty());
    library.remove_link(&a, &b)?;
    library.remove_link(&a, &b)?;
    Ok(())
}

#[test]
fn a_confirmed_link_keeps_its_origin_when_added_again() -> TestResult {
    let Three {
        library,
        ids: [a, b, _],
        _data,
        _src,
    } = three()?;
    library.add_link(&a, &b, LinkOrigin::Reference, "first")?;
    library.add_link(&a, &b, LinkOrigin::Manual, "second")?;
    let link = &library.links_of(&a)?.needs[0];
    assert_eq!(
        (link.origin, link.reason.as_str()),
        (LinkOrigin::Reference, "first")
    );
    Ok(())
}

#[test]
fn links_go_when_either_artifact_is_removed() -> TestResult {
    let Three {
        mut library,
        ids: [a, b, c],
        _data,
        _src,
    } = three()?;
    library.add_link(&a, &b, LinkOrigin::Manual, "")?;
    library.add_link(&c, &a, LinkOrigin::Manual, "")?;
    library.remove(&a)?;
    let rows: i64 = library
        .store
        .conn
        .query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))?;
    assert_eq!(rows, 0);
    assert!(library.links_of(&b)?.needed_by.is_empty());
    Ok(())
}

#[test]
fn import_links_what_a_new_artifact_references() -> TestResult {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    skill(src.path(), "review")?;
    agent(src.path(), "boss", &["review", "missing"])?;
    let mut library = Library::open(data.path())?;
    import_all(&mut library, src.path())?;
    let boss = id_of(&library, ArtifactKind::Agent, "boss")?;
    let review = id_of(&library, ArtifactKind::Skill, "review")?;
    let links = library.detail(&boss)?.links;
    assert_eq!(links.needs.len(), 1);
    assert_eq!(links.needs[0].id, review);
    assert_eq!(links.needs[0].origin, LinkOrigin::Reference);
    assert_eq!(library.detail(&review)?.links.needed_by[0].id, boss);
    assert_eq!(library.bundle_of(&boss)?, [review]);
    Ok(())
}

#[test]
fn import_links_existing_artifacts_whose_reference_now_resolves() -> TestResult {
    let data = tempfile::tempdir()?;
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    agent(first.path(), "boss", &["review"])?;
    skill(second.path(), "review")?;
    let mut library = Library::open(data.path())?;
    import_all(&mut library, first.path())?;
    let boss = id_of(&library, ArtifactKind::Agent, "boss")?;
    assert!(library.links_of(&boss)?.needs.is_empty());
    import_all(&mut library, second.path())?;
    let review = id_of(&library, ArtifactKind::Skill, "review")?;
    assert_eq!(library.links_of(&boss)?.needs[0].id, review);
    assert_eq!(library.links_of(&review)?.needed_by[0].id, boss);
    Ok(())
}

#[test]
fn reference_links_never_overturn_a_rejected_pair() -> TestResult {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    skill(src.path(), "review")?;
    agent(src.path(), "boss", &["review"])?;
    let mut library = Library::open(data.path())?;
    import_all(&mut library, src.path())?;
    let boss = id_of(&library, ArtifactKind::Agent, "boss")?;
    let review = id_of(&library, ArtifactKind::Skill, "review")?;
    library.reject_link(&boss, &review)?;
    add_reference_links(&library.store.conn, &[&boss, &review], 0)?;
    assert!(library.links_of(&boss)?.needs.is_empty());
    Ok(())
}

#[test]
fn settings_round_trip_json_and_replace() -> TestResult {
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    assert_eq!(library.settings_get("bundles.model")?, None);
    library.settings_set("bundles.model", &serde_json::json!("haiku"))?;
    library.settings_set("bundles.detect", &serde_json::json!({"a": [1, true]}))?;
    assert_eq!(
        library.settings_get("bundles.model")?,
        Some(serde_json::json!("haiku"))
    );
    library.settings_set("bundles.model", &serde_json::json!("opus"))?;
    assert_eq!(
        library.settings_get("bundles.model")?,
        Some(serde_json::json!("opus"))
    );
    assert_eq!(
        library.settings_get("bundles.detect")?,
        Some(serde_json::json!({"a": [1, true]}))
    );
    assert!(library.settings_get("  ").is_err());
    assert!(library
        .settings_set(&"k".repeat(500), &serde_json::json!(1))
        .is_err());
    drop(library);
    let again = Library::open(data.path())?;
    assert_eq!(
        again.settings_get("bundles.model")?,
        Some(serde_json::json!("opus"))
    );
    Ok(())
}
