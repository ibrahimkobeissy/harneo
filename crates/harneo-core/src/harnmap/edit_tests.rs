//! Edits of the current map: each one applied and saved, older draws untouched.

use super::draw_tests::{draw, fixture, Fixture};
use super::*;
use crate::artifact::ArtifactKind;
use crate::error::Error;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn drawn() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    Ok(f)
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

fn stage(id: &str) -> Zone {
    Zone::Stage { id: id.to_owned() }
}

#[test]
fn move_pins_and_reorders_and_older_draws_never_change() -> TestResult {
    let f = drawn()?;
    let before = f.library.harnmap_draw_at(f.profile, 1)?;
    let tdd = f.id("tdd");
    let review = f.id("review");
    // review joins Build in front of tdd.
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Move {
            artifact: review.clone(),
            zone: stage("build"),
            order: 0,
        },
    )?;
    let view = f.library.harnmap(f.profile)?;
    let r = at(&f, "review")?;
    let t = at(&f, "tdd")?;
    assert_eq!((r.order, t.order), (0, 1));
    assert!(r.pinned && r.by == PlacedBy::User && r.why == "You moved it here.");
    assert!(!t.pinned);
    assert_eq!(view.no, 1);
    assert_eq!(
        view.doc
            .placements
            .iter()
            .filter(|p| p.artifact == tdd)
            .count(),
        1
    );
    // Persisted: a re-read gives the same; the (only) draw is the edited one.
    assert_eq!(f.library.harnmap_draw_at(f.profile, 1)?, view.doc);
    assert_ne!(before, view.doc);

    // A second draw is current now; edits leave draw 1 alone.
    draw(&f)?;
    let first = f.library.harnmap_draw_at(f.profile, 1)?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Note {
            artifact: tdd,
            text: "x".repeat(100),
        },
    )?;
    assert_eq!(f.library.harnmap_draw_at(f.profile, 1)?, first);
    assert_eq!(at(&f, "tdd")?.note.chars().count(), MAX_NOTE);
    Ok(())
}

#[test]
fn place_remove_unpin_and_note() -> TestResult {
    let f = drawn()?;
    let leaky = f.id("leaky");
    let view = f.library.harnmap(f.profile)?;
    assert!(view.unplaced.contains(&leaky));
    let view = f.library.harnmap_edit(
        f.profile,
        MapEdit::Place {
            artifact: leaky.clone(),
            zone: Zone::Toolbelt,
            note: "  handy  ".to_owned(),
        },
    )?;
    assert!(!view.unplaced.contains(&leaky));
    let p = at(&f, "leaky")?;
    assert_eq!(
        (p.note.as_str(), p.why.as_str()),
        ("handy", "You placed it here.")
    );
    assert!(p.pinned && p.by == PlacedBy::User);
    // Placing twice, or an artifact outside the profile, is refused.
    let twice = f.library.harnmap_edit(
        f.profile,
        MapEdit::Place {
            artifact: leaky.clone(),
            zone: Zone::Floor,
            note: String::new(),
        },
    );
    assert!(matches!(twice, Err(Error::InvalidMapEdit { .. })));
    let alien = f.library.harnmap_edit(
        f.profile,
        MapEdit::Place {
            artifact: "nope".to_owned(),
            zone: Zone::Floor,
            note: String::new(),
        },
    );
    assert!(matches!(alien, Err(Error::InvalidMapEdit { .. })));

    f.library.harnmap_edit(
        f.profile,
        MapEdit::Unpin {
            artifact: leaky.clone(),
        },
    )?;
    assert!(!at(&f, "leaky")?.pinned);
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Note {
            artifact: leaky.clone(),
            text: "new".to_owned(),
        },
    )?;
    assert_eq!(at(&f, "leaky")?.note, "new");
    let view = f.library.harnmap_edit(
        f.profile,
        MapEdit::Remove {
            artifact: leaky.clone(),
        },
    )?;
    assert!(view.unplaced.contains(&leaky));
    // Editing something that is not on the map is refused.
    for edit in [
        MapEdit::Remove {
            artifact: leaky.clone(),
        },
        MapEdit::Unpin {
            artifact: leaky.clone(),
        },
        MapEdit::Note {
            artifact: leaky.clone(),
            text: String::new(),
        },
        MapEdit::Move {
            artifact: leaky.clone(),
            zone: Zone::Floor,
            order: 0,
        },
    ] {
        assert!(matches!(
            f.library.harnmap_edit(f.profile, edit),
            Err(Error::InvalidMapEdit { .. })
        ));
    }
    Ok(())
}

#[test]
fn moving_to_an_unknown_stage_is_refused_and_changes_nothing() -> TestResult {
    let f = drawn()?;
    let before = f.library.harnmap(f.profile)?.doc;
    let refused = f.library.harnmap_edit(
        f.profile,
        MapEdit::Move {
            artifact: f.id("tdd"),
            zone: stage("nowhere"),
            order: 0,
        },
    );
    assert!(matches!(refused, Err(Error::InvalidMapEdit { reason }) if reason.contains("nowhere")));
    assert_eq!(f.library.harnmap(f.profile)?.doc, before);
    Ok(())
}

#[test]
fn hide_and_show_artifacts_and_types() -> TestResult {
    let f = drawn()?;
    let tdd = f.id("tdd");
    let hidden = |f: &Fixture| f.library.harnmap(f.profile).map(|v| v.doc);
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Hide {
            artifact: tdd.clone(),
        },
    )?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Hide {
            artifact: tdd.clone(),
        },
    )?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::HideType {
            kind: ArtifactKind::Rule,
        },
    )?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::HideType {
            kind: ArtifactKind::Rule,
        },
    )?;
    let doc = hidden(&f)?;
    assert_eq!(doc.hidden, vec![tdd.clone()]);
    assert_eq!(doc.hidden_types, vec![ArtifactKind::Rule]);
    f.library
        .harnmap_edit(f.profile, MapEdit::Show { artifact: tdd })?;
    f.library.harnmap_edit(
        f.profile,
        MapEdit::ShowType {
            kind: ArtifactKind::Rule,
        },
    )?;
    let doc = hidden(&f)?;
    assert!(doc.hidden.is_empty() && doc.hidden_types.is_empty());
    Ok(())
}

#[test]
fn stages_can_be_renamed_added_and_moved() -> TestResult {
    let f = drawn()?;
    let ids = |f: &Fixture| {
        f.library.harnmap(f.profile).map(|v| {
            v.doc
                .stages
                .iter()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>()
        })
    };
    let view = f.library.harnmap_edit(
        f.profile,
        MapEdit::RenameStage {
            id: "plan".to_owned(),
            name: "  Planning ".to_owned(),
        },
    )?;
    let plan = view
        .doc
        .stages
        .iter()
        .find(|s| s.id == "plan")
        .ok_or("plan")?;
    assert_eq!(
        (plan.name.as_str(), plan.origin),
        ("Planning", StageOrigin::User)
    );
    let empty = f.library.harnmap_edit(
        f.profile,
        MapEdit::RenameStage {
            id: "plan".to_owned(),
            name: " ".to_owned(),
        },
    );
    assert!(matches!(empty, Err(Error::InvalidMapEdit { .. })));
    let unknown = f.library.harnmap_edit(
        f.profile,
        MapEdit::RenameStage {
            id: "zzz".to_owned(),
            name: "A".to_owned(),
        },
    );
    assert!(matches!(unknown, Err(Error::InvalidMapEdit { .. })));

    f.library.harnmap_edit(
        f.profile,
        MapEdit::AddStage {
            after: Some("plan".to_owned()),
            name: "Demo".to_owned(),
        },
    )?;
    // A second stage of the same name gets another id; no `after` appends.
    let view = f.library.harnmap_edit(
        f.profile,
        MapEdit::AddStage {
            after: None,
            name: "Demo".to_owned(),
        },
    )?;
    let all = ids(&f)?;
    assert_eq!(all[3], "demo");
    assert_eq!(all.last().map(String::as_str), Some("demo-2"));
    assert!(
        view.doc
            .stages
            .iter()
            .filter(|s| s.origin == StageOrigin::User)
            .count()
            >= 3
    );
    let bad_after = f.library.harnmap_edit(
        f.profile,
        MapEdit::AddStage {
            after: Some("zzz".to_owned()),
            name: "X".to_owned(),
        },
    );
    assert!(matches!(bad_after, Err(Error::InvalidMapEdit { .. })));
    let blank = f.library.harnmap_edit(
        f.profile,
        MapEdit::AddStage {
            after: None,
            name: String::new(),
        },
    );
    assert!(matches!(blank, Err(Error::InvalidMapEdit { .. })));
    // 7 + 2 = 9 stages; three more reach the limit of 12, one more is refused.
    for n in 0..3 {
        f.library.harnmap_edit(
            f.profile,
            MapEdit::AddStage {
                after: None,
                name: format!("S{n}"),
            },
        )?;
    }
    let full = f.library.harnmap_edit(
        f.profile,
        MapEdit::AddStage {
            after: None,
            name: "Over".to_owned(),
        },
    );
    assert!(matches!(full, Err(Error::InvalidMapEdit { .. })));

    f.library.harnmap_edit(
        f.profile,
        MapEdit::MoveStage {
            id: "ship".to_owned(),
            to: 0,
        },
    )?;
    assert_eq!(ids(&f)?[0], "ship");
    f.library.harnmap_edit(
        f.profile,
        MapEdit::MoveStage {
            id: "ship".to_owned(),
            to: 99,
        },
    )?;
    assert_eq!(ids(&f)?.last().map(String::as_str), Some("ship"));
    let unknown = f.library.harnmap_edit(
        f.profile,
        MapEdit::MoveStage {
            id: "zzz".to_owned(),
            to: 1,
        },
    );
    assert!(matches!(unknown, Err(Error::InvalidMapEdit { .. })));
    Ok(())
}

#[test]
fn removing_a_stage_unpins_its_stations_and_drops_its_loops() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let stages: Vec<serde_json::Value> =
        ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"]
            .iter()
            .map(|n| serde_json::json!({"name": n}))
            .collect();
    let on = |short: &str, zone: &str| serde_json::json!({"artifact": short, "zone": zone, "note": "", "why": "", "role": "", "when": "", "hands_off": ""});
    f.claude(&[serde_json::json!({
        "stages": stages,
        "loops": [{"from": "review", "to": "build", "label": "changes requested"}],
        "placements": [on("a1", "stage:build"), on("a2", "stage:review")],
    })])?;
    draw(&f)?;
    let tdd = f.id("tdd");
    let review = f.id("review");
    // The Review -> Build loop touches Build; pin tdd on Build.
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Move {
            artifact: tdd.clone(),
            zone: stage("build"),
            order: 0,
        },
    )?;
    assert!(f
        .library
        .harnmap(f.profile)?
        .doc
        .loops
        .iter()
        .any(|l| l.to == "build"));
    let view = f.library.harnmap_edit(
        f.profile,
        MapEdit::RemoveStage {
            id: "build".to_owned(),
        },
    )?;
    assert!(view.doc.stages.iter().all(|s| s.id != "build"));
    assert!(view
        .doc
        .loops
        .iter()
        .all(|l| l.from != "build" && l.to != "build"));
    assert!(view.doc.placements.iter().all(|p| p.artifact != tdd));
    assert!(view.unplaced.contains(&tdd));
    assert!(view.doc.placements.iter().any(|p| p.artifact == review));
    let none = f.library.harnmap_edit(
        f.profile,
        MapEdit::RemoveStage {
            id: "build".to_owned(),
        },
    );
    assert!(matches!(none, Err(Error::InvalidMapEdit { .. })));
    Ok(())
}

#[test]
fn removing_a_stage_leaves_phases_and_parallel_sound() -> TestResult {
    let mut f = fixture(CategoryId::Video, None)?;
    let stages: Vec<serde_json::Value> = [
        "Concept",
        "Script",
        "Storyboard",
        "Shoot & assets",
        "Edit",
        "Sound & captions",
        "Publish",
    ]
    .iter()
    .map(|n| serde_json::json!({"name": n}))
    .collect();
    f.claude(&[serde_json::json!({"stages": stages, "loops": [], "placements": []})])?;
    draw(&f)?;
    let view = f.library.harnmap(f.profile)?;
    assert!(!view.doc.parallel.is_empty() && !view.doc.phases.is_empty());
    let mut view = view;
    for id in view
        .doc
        .stages
        .iter()
        .map(|s| s.id.clone())
        .collect::<Vec<_>>()
    {
        if id != "edit" {
            view = f
                .library
                .harnmap_edit(f.profile, MapEdit::RemoveStage { id })?;
        }
    }
    assert_eq!(view.doc.stages.len(), 1);
    assert!(view.doc.phases.iter().all(|p| p.stages == ["edit"]));
    let last = f.library.harnmap_edit(
        f.profile,
        MapEdit::RemoveStage {
            id: "edit".to_owned(),
        },
    );
    assert!(matches!(last, Err(Error::InvalidMapEdit { .. })));
    Ok(())
}
