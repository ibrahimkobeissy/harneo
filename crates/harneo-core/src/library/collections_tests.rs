use std::fs;

use super::*;
use crate::artifact::ArtifactKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Setup {
    _data: tempfile::TempDir,
    src: tempfile::TempDir,
    library: Library,
    source: Source,
}

fn write_agent(root: &Path, name: &str) -> TestResult {
    fs::create_dir_all(root.join("agents"))?;
    fs::write(
        root.join(format!("agents/{name}.md")),
        format!("---\nname: {name}\ndescription: About {name}\n---\nGo.\n"),
    )?;
    Ok(())
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    for name in ["alpha", "bravo", "charlie"] {
        write_agent(src.path(), name)?;
    }
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: src.path().to_path_buf(),
    };
    Ok(Setup {
        _data: data,
        src,
        library,
        source,
    })
}

/// Imports the named artifacts and returns their ids in the same order.
fn import(
    s: &mut Setup,
    names: &[&str],
) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
    let preview = s.library.preview(&s.source)?;
    let ids: Vec<String> = names
        .iter()
        .filter_map(|n| {
            preview
                .candidates
                .iter()
                .find(|c| c.artifact.name == *n)
                .map(|c| c.id.clone())
        })
        .collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    Ok(ids)
}

fn page_names(
    library: &Library,
    id: i64,
    query: &Query,
) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(library
        .collection_page(id, query)?
        .items
        .into_iter()
        .map(|i| i.name)
        .collect())
}

fn by_position() -> Query {
    Query {
        sort: SortKey::Position,
        ..Query::default()
    }
}

#[test]
fn names_are_validated_and_unique_ignoring_case() -> TestResult {
    let s = setup()?;
    let c = s.library.create_manual_collection("  Daily ")?;
    assert_eq!(c.name, "Daily");
    assert!(matches!(
        s.library.create_manual_collection("daily"),
        Err(Error::CollectionExists { .. })
    ));
    assert!(matches!(
        s.library.create_manual_collection("  "),
        Err(Error::InvalidCollection { .. })
    ));
    assert!(matches!(
        s.library.create_manual_collection(&"x".repeat(61)),
        Err(Error::InvalidCollection { .. })
    ));
    s.library.create_manual_collection(&"x".repeat(60))?;
    let other = s.library.create_manual_collection("other")?;
    assert!(matches!(
        s.library.rename_collection(other.id, "DAILY"),
        Err(Error::CollectionExists { .. })
    ));
    assert_eq!(s.library.rename_collection(c.id, "DAILY")?.name, "DAILY");
    assert!(matches!(
        s.library.rename_collection(999, "z"),
        Err(Error::CollectionNotFound { .. })
    ));
    s.library.delete_collection(other.id)?;
    assert!(matches!(
        s.library.delete_collection(other.id),
        Err(Error::CollectionNotFound { .. })
    ));
    Ok(())
}

#[test]
fn manual_collection_keeps_order_and_add_is_idempotent() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["charlie", "alpha", "bravo"])?;
    let c = s.library.create_manual_collection("mine")?;
    for id in &ids {
        s.library.add_to_collection(c.id, id)?;
    }
    s.library.add_to_collection(c.id, &ids[0])?;
    assert_eq!(
        page_names(&s.library, c.id, &by_position())?,
        ["charlie", "alpha", "bravo"]
    );
    let desc = Query {
        descending: true,
        ..by_position()
    };
    assert_eq!(
        page_names(&s.library, c.id, &desc)?,
        ["bravo", "alpha", "charlie"]
    );
    assert_eq!(
        page_names(&s.library, c.id, &Query::default())?,
        ["alpha", "bravo", "charlie"]
    );

    let new_order = vec![ids[2].clone(), ids[0].clone(), ids[1].clone()];
    s.library.reorder_collection(c.id, &new_order)?;
    assert_eq!(
        page_names(&s.library, c.id, &by_position())?,
        ["bravo", "charlie", "alpha"]
    );

    s.library.remove_from_collection(c.id, &ids[0])?;
    s.library.remove_from_collection(c.id, &ids[0])?;
    assert_eq!(
        page_names(&s.library, c.id, &by_position())?,
        ["bravo", "alpha"]
    );
    // A new member goes to the end.
    s.library.add_to_collection(c.id, &ids[0])?;
    assert_eq!(
        page_names(&s.library, c.id, &by_position())?,
        ["bravo", "alpha", "charlie"]
    );
    assert_eq!(s.library.collections()?[0].count, 3);

    assert!(matches!(
        s.library.add_to_collection(c.id, "0000000000000000"),
        Err(Error::NotInLibrary { .. })
    ));
    Ok(())
}

#[test]
fn reorder_must_be_a_permutation_of_the_members() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["alpha", "bravo", "charlie"])?;
    let c = s.library.create_manual_collection("mine")?;
    s.library.add_to_collection(c.id, &ids[0])?;
    s.library.add_to_collection(c.id, &ids[1])?;
    let bad = [
        vec![ids[0].clone()],
        vec![ids[0].clone(), ids[0].clone()],
        vec![ids[0].clone(), ids[2].clone()],
        vec![ids[0].clone(), ids[1].clone(), ids[2].clone()],
    ];
    for order in bad {
        assert!(matches!(
            s.library.reorder_collection(c.id, &order),
            Err(Error::InvalidCollection { .. })
        ));
    }
    assert_eq!(
        page_names(&s.library, c.id, &by_position())?,
        ["alpha", "bravo"]
    );
    Ok(())
}

#[test]
fn removing_from_the_library_and_deleting_collections() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["alpha", "bravo"])?;
    let c = s.library.create_manual_collection("mine")?;
    s.library.add_to_collection(c.id, &ids[0])?;
    s.library.add_to_collection(c.id, &ids[1])?;
    s.library.remove(&ids[0])?;
    assert_eq!(page_names(&s.library, c.id, &by_position())?, ["bravo"]);
    assert_eq!(s.library.collections()?[0].count, 1);
    s.library.delete_collection(c.id)?;
    assert_eq!(s.library.list(&Query::default())?.total, 1);
    Ok(())
}

#[test]
fn smart_collection_picks_up_later_imports() -> TestResult {
    let mut s = setup()?;
    import(&mut s, &["alpha"])?;
    let rule = Filter {
        types: vec![ArtifactKind::Agent],
        ..Filter::default()
    };
    let c = s.library.create_smart_collection("agents", rule.clone())?;
    assert_eq!(
        (c.kind, c.count, c.rule.as_ref()),
        (CollectionKind::Smart, 1, Some(&rule))
    );
    import(&mut s, &["bravo", "charlie"])?;
    assert_eq!(s.library.collections()?[0].count, 3);
    assert_eq!(
        page_names(&s.library, c.id, &Query::default())?,
        ["alpha", "bravo", "charlie"]
    );
    // The first import's source is already known; a file added to the folder is a new import.
    write_agent(s.src.path(), "delta")?;
    import(&mut s, &["delta"])?;
    assert_eq!(s.library.collections()?[0].count, 4);
    Ok(())
}

#[test]
fn smart_rule_is_and_across_and_or_within() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["alpha", "bravo", "charlie"])?;
    let red = s.library.create_tag("red", TagColor::Cyan)?;
    let blue = s.library.create_tag("blue", TagColor::Sky)?;
    s.library.tag_artifact(&ids[0], red.id)?;
    s.library.tag_artifact(&ids[1], blue.id)?;
    s.library.tag_artifact(&ids[2], blue.id)?;

    let either = Filter {
        tags: vec!["red".into(), "blue".into()],
        ..Filter::default()
    };
    let c = s.library.create_smart_collection("either", either)?;
    assert_eq!(c.count, 3, "OR within the tag row");

    let both_rows = Filter {
        tags: vec!["red".into(), "blue".into()],
        text: Some("charlie".into()),
        ..Filter::default()
    };
    let d = s.library.create_smart_collection("both", both_rows)?;
    assert_eq!(d.count, 1, "AND across rows");
    assert_eq!(
        page_names(&s.library, d.id, &Query::default())?,
        ["charlie"]
    );

    // The query's own filter is ANDed with the rule, facets included.
    let q = Query {
        filter: Filter {
            tags: vec!["red".into()],
            ..Filter::default()
        },
        ..Query::default()
    };
    let page = s.library.collection_page(c.id, &q)?;
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].name, "alpha");
    let blue_facet = page.facets.tags.iter().find(|t| t.name == "blue");
    assert_eq!(blue_facet.map(|t| t.count), Some(2));

    // Editing the rule re-evaluates.
    let narrowed = Filter {
        tags: vec!["red".into()],
        ..Filter::default()
    };
    assert_eq!(s.library.update_rule(c.id, narrowed)?.count, 1);
    Ok(())
}

#[test]
fn rules_and_kinds_are_enforced() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["alpha"])?;
    assert!(matches!(
        s.library
            .create_smart_collection("empty", Filter::default()),
        Err(Error::InvalidCollection { .. })
    ));
    let blank = Filter {
        text: Some("  ".into()),
        ..Filter::default()
    };
    assert!(matches!(
        s.library.create_smart_collection("blank", blank),
        Err(Error::InvalidCollection { .. })
    ));
    let manual = s.library.create_manual_collection("m")?;
    let rule = Filter {
        text: Some("a".into()),
        ..Filter::default()
    };
    let smart = s.library.create_smart_collection("s", rule.clone())?;
    assert!(matches!(
        s.library.update_rule(manual.id, rule.clone()),
        Err(Error::InvalidCollection { .. })
    ));
    assert!(matches!(
        s.library.update_rule(smart.id, Filter::default()),
        Err(Error::InvalidCollection { .. })
    ));
    assert!(matches!(
        s.library.add_to_collection(smart.id, &ids[0]),
        Err(Error::InvalidCollection { .. })
    ));
    assert!(matches!(
        s.library.reorder_collection(smart.id, &[]),
        Err(Error::InvalidCollection { .. })
    ));
    assert!(matches!(
        s.library.collection_page(999, &Query::default()),
        Err(Error::CollectionNotFound { .. })
    ));
    // Position on a smart collection sorts by name.
    assert_eq!(page_names(&s.library, smart.id, &by_position())?, ["alpha"]);
    // Collections list by name with their counts.
    let all = s.library.collections()?;
    assert_eq!(
        all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["m", "s"]
    );
    Ok(())
}

#[test]
fn an_artifact_lists_the_manual_collections_that_hold_it() -> TestResult {
    let mut s = setup()?;
    let ids = import(&mut s, &["alpha", "bravo"])?;
    let one = s.library.create_manual_collection("one")?;
    let two = s.library.create_manual_collection("two")?;
    s.library.add_to_collection(one.id, &ids[0])?;
    s.library.add_to_collection(two.id, &ids[0])?;
    s.library.add_to_collection(two.id, &ids[1])?;
    assert_eq!(s.library.artifact_collections(&ids[0])?, [one.id, two.id]);
    assert_eq!(s.library.artifact_collections(&ids[1])?, [two.id]);
    s.library.remove_from_collection(one.id, &ids[0])?;
    assert_eq!(s.library.artifact_collections(&ids[0])?, [two.id]);
    assert!(s.library.artifact_collections("nope")?.is_empty());
    Ok(())
}
