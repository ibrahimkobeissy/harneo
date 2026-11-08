//! Parent and child: Claude's reading kept sound, and the edits that move a family together.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::draw_tests::{draw, fixture, placement, Fixture};
use super::*;
use crate::artifact::ArtifactKind;
use crate::error::Error;
use crate::library::ArtifactId;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn stage_names() -> Vec<Value> {
    ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"]
        .iter()
        .map(|n| json!({"name": n}))
        .collect()
}

/// The fixture's skills tdd (a1), review (a2), leaky (a3) and CLAUDE.md (a4).
fn drawn_with(placements: Vec<Value>) -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let mut f = fixture(CategoryId::Software, None)?;
    f.claude(&[json!({"stages": stage_names(), "loops": [], "placements": placements})])?;
    draw(&f)?;
    Ok(f)
}

fn child_of(short: &str, zone: &str, parent: &str) -> Value {
    let mut p = placement(short, zone);
    p["parent"] = json!(parent);
    p
}

fn at(f: &Fixture, name: &str) -> std::result::Result<Placement, Box<dyn std::error::Error>> {
    let id = f.id(name);
    Ok(f.library
        .harnmap(f.profile)?
        .doc
        .placements
        .into_iter()
        .find(|p| p.artifact == id)
        .ok_or("not placed")?)
}

fn family() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    // review and leaky run through tdd, whatever zone Claude put them in.
    drawn_with(vec![
        placement("a1", "stage:Build"),
        child_of("a2", "stage:Review", "a1"),
        child_of("a3", "floor", "a1"),
        placement("a4", "floor"),
    ])
}

fn stage(id: &str) -> Zone {
    Zone::Stage { id: id.to_owned() }
}

#[test]
fn a_named_parent_is_stored_and_the_child_takes_the_parents_zone() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let mut third = child_of("a3", "floor", "a1");
    third["also_run_by"] = json!(["a2", "a1", "a3", "a99", "a2", "a4"]);
    f.claude(&[json!({
        "stages": stage_names(),
        "loops": [],
        "placements": [
            placement("a1", "stage:Build"),
            child_of("a2", "stage:Review", "a1"),
            third,
            placement("a4", "floor"),
        ],
    })])?;
    draw(&f)?;
    let tdd = at(&f, "tdd")?;
    let review = at(&f, "review")?;
    let leaky = at(&f, "leaky")?;
    assert_eq!(review.parent.as_deref(), Some(tdd.artifact.as_str()));
    assert_eq!(leaky.parent.as_deref(), Some(tdd.artifact.as_str()));
    assert_eq!(review.zone, tdd.zone);
    assert_eq!(leaky.zone, tdd.zone);
    // A child draws right after its parent.
    assert_eq!(
        [tdd.order, review.order, leaky.order],
        [0, 1, 2],
        "parent first, then its children"
    );
    // Self, the parent, unknown ids and repeats are dropped; the CLAUDE.md is a real artifact.
    assert_eq!(
        leaky.also_run_by,
        [review.artifact, at(&f, "CLAUDE.md")?.artifact]
    );
    assert!(tdd.parent.is_none() && tdd.also_run_by.is_empty());
    assert!(f.stdin(0)?.contains("not meant to be started on its own"));
    Ok(())
}

#[test]
fn invalid_parents_are_dropped_not_fatal() -> TestResult {
    // A CLAUDE.md cannot be a parent, a99 is unknown, a3 is its own parent.
    let f = drawn_with(vec![
        child_of("a1", "stage:Build", "a4"),
        child_of("a2", "stage:Review", "a99"),
        child_of("a3", "floor", "a3"),
        placement("a4", "floor"),
    ])?;
    for name in ["tdd", "review", "leaky"] {
        assert!(at(&f, name)?.parent.is_none(), "{name}");
    }
    assert_eq!(at(&f, "review")?.zone, stage("review"));
    assert_eq!(f.calls(), 1, "no retry for a dropped parent");
    Ok(())
}

#[test]
fn an_unplaced_parent_is_dropped() -> TestResult {
    let f = drawn_with(vec![
        placement("a1", "stage:Build"),
        child_of("a2", "stage:Review", "a3"),
    ])?;
    assert!(at(&f, "review")?.parent.is_none());
    Ok(())
}

#[test]
fn in_a_cycle_the_later_placement_loses_its_parent() -> TestResult {
    let f = drawn_with(vec![
        child_of("a1", "stage:Build", "a2"),
        child_of("a2", "stage:Review", "a1"),
    ])?;
    let review = at(&f, "review")?;
    assert_eq!(at(&f, "tdd")?.parent, Some(review.artifact.clone()));
    assert!(review.parent.is_none());
    assert_eq!(at(&f, "tdd")?.zone, review.zone);
    Ok(())
}

#[test]
fn a_redraw_keeps_a_detached_child_detached() -> TestResult {
    let f = family()?;
    let review = f.id("review");
    f.library
        .harnmap_edit(f.profile, MapEdit::Detach { artifact: review })?;
    draw(&f)?;
    let review = at(&f, "review")?;
    assert!(review.parent.is_none() && review.pinned);
    assert!(at(&f, "leaky")?.parent.is_some());
    Ok(())
}

#[test]
fn moving_a_parent_moves_its_children() -> TestResult {
    let f = family()?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Move {
            artifact: f.id("tdd"),
            zone: stage("test"),
            order: 0,
        },
    )?;
    for name in ["tdd", "review", "leaky"] {
        assert_eq!(at(&f, name)?.zone, stage("test"), "{name}");
    }
    let (review, leaky) = (at(&f, "review")?, at(&f, "leaky")?);
    assert!(review.parent.is_some() && leaky.parent.is_some());
    assert!(!review.pinned, "only the parent was moved by the user");
    assert_eq!([at(&f, "tdd")?.order, review.order, leaky.order], [0, 1, 2]);
    Ok(())
}

#[test]
fn moving_a_child_or_detaching_it_clears_the_parent_and_pins_it() -> TestResult {
    let f = family()?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Move {
            artifact: f.id("review"),
            zone: stage("ship"),
            order: 0,
        },
    )?;
    let review = at(&f, "review")?;
    assert!(review.parent.is_none() && review.pinned);
    assert_eq!(review.zone, stage("ship"));
    assert_eq!(at(&f, "leaky")?.zone, stage("build"), "its sibling stays");

    f.library.harnmap_edit(
        f.profile,
        MapEdit::Detach {
            artifact: f.id("leaky"),
        },
    )?;
    let (tdd, leaky) = (at(&f, "tdd")?, at(&f, "leaky")?);
    assert!(leaky.parent.is_none() && leaky.pinned && leaky.by == PlacedBy::User);
    assert_eq!((leaky.zone.clone(), leaky.order), (tdd.zone, tdd.order + 1));
    // A placement that is not a child cannot be detached.
    let refused = f.library.harnmap_edit(
        f.profile,
        MapEdit::Detach {
            artifact: f.id("tdd"),
        },
    );
    assert!(matches!(refused, Err(Error::InvalidMapEdit { .. })));
    Ok(())
}

#[test]
fn removing_or_hiding_a_parent_frees_its_children_in_place() -> TestResult {
    let f = family()?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Remove {
            artifact: f.id("tdd"),
        },
    )?;
    for name in ["review", "leaky"] {
        let p = at(&f, name)?;
        assert!(p.parent.is_none() && p.also_run_by.is_empty(), "{name}");
        assert_eq!(p.zone, stage("build"), "{name}");
    }

    let f = family()?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Hide {
            artifact: f.id("tdd"),
        },
    )?;
    let review = at(&f, "review")?;
    assert!(review.parent.is_none());
    assert_eq!(review.zone, stage("build"));
    assert!(
        at(&f, "tdd")?.order < review.order,
        "still after the parent"
    );
    Ok(())
}

#[test]
fn removing_a_stage_sends_parent_and_children_to_unplaced() -> TestResult {
    let f = family()?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::RemoveStage {
            id: "build".to_owned(),
        },
    )?;
    let view = f.library.harnmap(f.profile)?;
    let left: Vec<&str> = view
        .doc
        .placements
        .iter()
        .map(|p| p.artifact.as_str())
        .collect();
    assert_eq!(left, [f.id("CLAUDE.md")]);
    assert!(view.doc.validate(&known_ids(&f)).is_empty());
    Ok(())
}

fn known_ids(f: &Fixture) -> std::collections::HashSet<ArtifactId> {
    ["tdd", "review", "leaky", "CLAUDE.md"]
        .iter()
        .map(|n| f.id(n))
        .collect()
}

fn doc_of(rows: &[(&str, Option<&str>, Zone)]) -> MapDoc {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    for (n, (id, parent, zone)) in rows.iter().enumerate() {
        doc.placements.push(Placement {
            artifact: (*id).to_owned(),
            zone: zone.clone(),
            order: u32::try_from(n).unwrap_or(0),
            note: String::new(),
            why: String::new(),
            role: String::new(),
            when: String::new(),
            hands_off: String::new(),
            next: Vec::new(),
            parent: parent.map(str::to_owned),
            also_run_by: Vec::new(),
            by: PlacedBy::Claude,
            pinned: false,
        });
    }
    doc
}

fn kinds() -> HashMap<ArtifactId, ArtifactKind> {
    [
        ("cmd", ArtifactKind::Command),
        ("cmd2", ArtifactKind::Command),
        ("s1", ArtifactKind::Skill),
        ("s2", ArtifactKind::Skill),
        ("ag", ArtifactKind::Agent),
        ("ag2", ArtifactKind::Agent),
        ("rule", ArtifactKind::Rule),
    ]
    .into_iter()
    .map(|(id, kind)| (id.to_owned(), kind))
    .collect()
}

#[test]
fn nesting_stops_at_two_levels() {
    let floor = Zone::Floor;
    let mut doc = doc_of(&[
        ("cmd", None, floor.clone()),
        ("s1", Some("cmd"), floor.clone()),
        ("ag", Some("s1"), floor.clone()),
        ("ag2", Some("ag"), floor.clone()),
        ("s2", Some("ag2"), floor),
    ]);
    doc.normalize_family(&kinds());
    let parents: Vec<Option<&str>> = doc.placements.iter().map(|p| p.parent.as_deref()).collect();
    // ag2 under ag would be a third level, so it stands alone; s2 then nests under it.
    assert_eq!(parents, [None, Some("cmd"), Some("s1"), None, Some("ag2")]);
    assert!(doc.family_problems().is_empty());
}

#[test]
fn only_commands_skills_and_subagents_parent_skills_and_subagents() {
    let floor = Zone::Floor;
    let mut doc = doc_of(&[
        ("rule", None, floor.clone()),
        ("cmd", None, floor.clone()),
        ("cmd2", Some("cmd"), floor.clone()),
        ("s1", Some("rule"), floor.clone()),
        ("ag", Some("s2"), floor.clone()),
        ("s2", None, floor),
    ]);
    doc.normalize_family(&kinds());
    let parents: Vec<Option<&str>> = doc.placements.iter().map(|p| p.parent.as_deref()).collect();
    assert_eq!(parents, [None, None, None, None, Some("s2"), None]);
}

#[test]
fn also_run_by_needs_a_parent_and_keeps_at_most_three() {
    let floor = Zone::Floor;
    let mut doc = doc_of(&[
        ("cmd", None, floor.clone()),
        ("cmd2", None, floor.clone()),
        ("s2", None, floor.clone()),
        ("ag2", None, floor.clone()),
        ("s1", Some("cmd"), floor.clone()),
        ("ag", None, floor),
    ]);
    doc.placements[4].also_run_by = ["cmd", "cmd2", "s2", "s1", "ag2", "ag", "ghost"]
        .map(str::to_owned)
        .to_vec();
    doc.placements[5].also_run_by = vec!["cmd".to_owned()];
    doc.normalize_family(&kinds());
    assert_eq!(doc.placements[4].also_run_by, ["cmd2", "s2", "ag2"]);
    assert!(doc.placements[5].also_run_by.is_empty());
}

#[test]
fn validation_refuses_a_child_outside_its_parents_zone() {
    let mut doc = doc_of(&[
        ("cmd", None, Zone::Floor),
        ("s1", Some("cmd"), Zone::Toolbelt),
        ("s2", Some("ghost"), Zone::Floor),
    ]);
    let known = ["cmd", "s1", "s2"].map(str::to_owned).into_iter().collect();
    let reasons = doc.validate(&known);
    assert_eq!(reasons.len(), 2, "{reasons:?}");
    doc.normalize_family(&kinds());
    assert!(doc.validate(&known).is_empty());
}

#[test]
fn dropping_an_artifact_frees_its_children_and_prunes_also_run_by() {
    let mut doc = doc_of(&[
        ("cmd", None, Zone::Floor),
        ("s1", Some("cmd"), Zone::Floor),
        ("s2", Some("cmd"), Zone::Floor),
    ]);
    doc.placements[2].also_run_by = vec!["cmd2".to_owned(), "s1".to_owned()];
    doc.drop_artifact("s1");
    assert_eq!(doc.placements[1].also_run_by, ["cmd2"]);
    doc.drop_artifact("cmd");
    assert!(doc.placements[0].parent.is_none());
    assert!(doc.placements[0].also_run_by.is_empty());
}

#[test]
fn a_map_drawn_before_parents_existed_reads_with_none() -> Result<(), serde_json::Error> {
    let p: Placement = serde_json::from_value(json!({
        "artifact": "a", "zone": {"type": "floor"}, "order": 0, "note": "", "why": "",
        "role": "", "when": "", "handsOff": "", "by": "claude", "pinned": false
    }))?;
    assert!(p.parent.is_none() && p.also_run_by.is_empty());
    Ok(())
}
