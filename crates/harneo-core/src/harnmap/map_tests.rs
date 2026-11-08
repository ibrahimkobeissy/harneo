use std::collections::HashSet;

use super::*;
use crate::artifact::ArtifactKind;
use crate::library::ArtifactId;

fn known(ids: &[&str]) -> HashSet<ArtifactId> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

fn place(artifact: &str, zone: Zone) -> Placement {
    Placement {
        artifact: artifact.to_owned(),
        zone,
        order: 0,
        note: String::new(),
        why: String::new(),
        role: String::new(),
        when: String::new(),
        hands_off: String::new(),
        next: Vec::new(),
        parent: None,
        also_run_by: Vec::new(),
        by: PlacedBy::Claude,
        pinned: false,
    }
}

fn on(stage: &str) -> Zone {
    Zone::Stage {
        id: stage.to_owned(),
    }
}

#[test]
fn every_template_makes_a_valid_empty_map() {
    for id in CategoryId::ALL {
        let doc = MapDoc::from_template(id);
        assert!(doc.validate(&known(&[])).is_empty(), "{id:?}");
        assert!(!doc.stages.is_empty());
        assert!(doc.stages.iter().all(|s| s.origin == StageOrigin::Template));
    }
}

#[test]
fn a_template_map_copies_stages_loops_phases_and_parallel_tracks() {
    let video = MapDoc::from_template(CategoryId::Video);
    assert_eq!(video.shape, Shape::Parallel);
    assert_eq!(video.parallel, ["edit", "sound-captions"]);
    assert_eq!(video.phases.len(), 3);
    assert_eq!(video.phases[0].stages, ["concept", "script", "storyboard"]);
    let software = MapDoc::from_template(CategoryId::Software);
    assert_eq!(software.shape, Shape::Line);
    assert_eq!(software.loops.len(), 1);
    assert_eq!(software.loops[0].label, "changes requested");
    assert!(software.parallel.is_empty() && software.phases.is_empty());
    let custom = MapDoc::from_template(CategoryId::Custom);
    assert_eq!(custom.shape, Shape::Line);
    assert_eq!(custom.stages.len(), 1);
    assert_eq!(custom.stages[0].id, "open");
}

#[test]
fn an_unknown_artifact_a_duplicate_and_an_unknown_stage_are_refused() {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    doc.placements = vec![
        place("a1", on("build")),
        place("a1", Zone::Toolbelt),
        place("ghost", Zone::Floor),
        place("a2", on("nowhere")),
    ];
    let reasons = doc.validate(&known(&["a1", "a2"]));
    assert_eq!(reasons.len(), 3, "{reasons:?}");
    assert!(reasons
        .iter()
        .any(|r| r.contains("ghost") && r.contains("not in the profile")));
    assert!(reasons
        .iter()
        .any(|r| r.contains("a1") && r.contains("twice")));
    assert!(reasons.iter().any(|r| r.contains("nowhere")));
}

#[test]
fn loops_phases_parallel_and_stage_ids_are_checked() {
    let mut doc = MapDoc::from_template(CategoryId::Video);
    doc.loops.push(Loop {
        from: "edit".into(),
        to: "gone".into(),
        label: String::new(),
    });
    doc.phases[0].stages.push("lost".into());
    doc.parallel.push("missing".into());
    doc.stages.push(doc.stages[0].clone());
    let reasons = doc.validate(&known(&[]));
    assert_eq!(reasons.len(), 4, "{reasons:?}");
    assert!(reasons.iter().any(|r| r.contains("twice")));
    assert!(reasons.iter().any(|r| r.contains("gone")));
    assert!(reasons.iter().any(|r| r.contains("lost")));
    assert!(reasons.iter().any(|r| r.contains("missing")));
}

#[test]
fn text_is_cut_to_the_limits_on_character_boundaries() {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    let mut p = place("a1", Zone::Floor);
    p.note = "é".repeat(MAX_NOTE + 5);
    p.why = "x".repeat(MAX_WHY + 1);
    p.role = "日".repeat(MAX_TOOLTIP + 1);
    p.when = "short".into();
    p.hands_off = "y".repeat(MAX_TOOLTIP);
    doc.placements = vec![p];
    doc.cut_to_limits();
    let p = &doc.placements[0];
    assert_eq!(p.note.chars().count(), MAX_NOTE);
    assert_eq!(p.why.chars().count(), MAX_WHY);
    assert_eq!(p.role.chars().count(), MAX_TOOLTIP);
    assert_eq!(p.when, "short");
    assert_eq!(p.hands_off.chars().count(), MAX_TOOLTIP);
}

#[test]
fn the_json_shape_is_camel_case_and_round_trips() -> Result<(), serde_json::Error> {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    doc.placements = vec![place("a1", on("build"))];
    doc.hidden_types = vec![ArtifactKind::OutputStyle];
    doc.changes = vec![TemplateChange::StageRenamed {
        id: "spec".into(),
        from: "Spec".into(),
        to: "Design".into(),
    }];
    let json = serde_json::to_value(&doc)?;
    assert_eq!(json["placements"][0]["handsOff"], "");
    assert_eq!(json["placements"][0]["zone"]["type"], "stage");
    assert_eq!(json["hiddenTypes"][0], "outputStyle");
    assert_eq!(json["stages"][0]["origin"], "template");
    assert_eq!(json["changes"][0]["type"], "stageRenamed");
    assert_eq!(serde_json::from_value::<MapDoc>(json)?, doc);
    Ok(())
}

#[test]
fn a_next_step_must_name_another_artifact_of_the_profile() {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    let mut p = place("a", Zone::Floor);
    p.next = vec!["b".into(), "a".into(), "ghost".into()];
    doc.placements.push(p);
    doc.placements.push(place("b", Zone::Toolbelt));
    let reasons = doc.validate(&known(&["a", "b"]));
    assert_eq!(reasons.len(), 2, "{reasons:?}");
    assert!(reasons.iter().all(|r| r.contains("next step")));
}

#[test]
fn a_map_drawn_before_next_steps_existed_still_reads() -> Result<(), serde_json::Error> {
    let p: Placement = serde_json::from_value(serde_json::json!({
        "artifact": "a", "zone": {"type": "floor"}, "order": 0, "note": "", "why": "",
        "role": "", "when": "", "handsOff": "", "by": "claude", "pinned": false
    }))?;
    assert!(p.next.is_empty());
    Ok(())
}

#[test]
fn dropping_an_artifact_also_drops_it_from_next_steps() {
    let mut doc = MapDoc::from_template(CategoryId::Software);
    let mut p = place("a", Zone::Floor);
    p.next = vec!["b".into(), "c".into()];
    doc.placements.push(p);
    doc.placements.push(place("b", Zone::Toolbelt));
    doc.drop_artifact("b");
    assert_eq!(doc.placements[0].next, ["c"]);
}
