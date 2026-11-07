//! The map view, history, restore and staleness.

use serde_json::json;

use super::draw_tests::{draw, fixture, Fixture};
use super::*;
use crate::error::Error;
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn drawn() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    Ok(f)
}

fn spec_without(f: &Fixture, name: &str) -> std::result::Result<ProfileSpec, Error> {
    let mut spec = f.library.profile(f.profile, None)?.spec;
    let id = f.id(name);
    spec.items.retain(|i| *i != id);
    Ok(spec)
}

#[test]
fn the_view_has_facts_names_unplaced_and_no_staleness() -> TestResult {
    let f = drawn()?;
    let view = f.library.harnmap(f.profile)?;
    assert!(view.drawn && view.no == 1 && view.draw_count == 1);
    assert_eq!(view.artifacts.len(), 4);
    let info = view.artifacts.get(&f.id("tdd")).ok_or("tdd")?;
    assert_eq!(info.name, "tdd");
    assert_eq!(view.unplaced, vec![f.id("leaky")]);
    assert!(view.facts.contains_key(&f.id("CLAUDE.md")));
    assert!(view.stale.is_none());
    assert!(view.bundles.is_empty() && view.hooks.is_empty());
    Ok(())
}

#[test]
fn edits_need_a_draw() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let r = f.library.harnmap_edit(
        f.profile,
        MapEdit::Hide {
            artifact: f.id("tdd"),
        },
    );
    assert!(matches!(r, Err(Error::NoMap)));
    let skeleton = f.library.harnmap(f.profile)?;
    assert!(!skeleton.drawn && skeleton.stale.is_none() && skeleton.draw_count == 0);
    assert_eq!(skeleton.unplaced.len(), 4);
    Ok(())
}

#[test]
fn a_removed_artifact_deleted_from_the_library_keeps_a_generic_name() -> TestResult {
    let mut f = drawn()?;
    let tdd = f.id("tdd");
    let trimmed = spec_without(&f, "tdd")?;
    f.library.save_profile(f.profile, &trimmed)?;
    f.library.remove(&tdd)?;
    let stale = f.library.harnmap(f.profile)?.stale.ok_or("stale")?;
    assert_eq!(stale.removed.len(), 1);
    assert_eq!(stale.removed[0].id, tdd);
    assert_eq!(stale.removed[0].name, "a removed artifact");
    assert_eq!(stale.removed[0].kind, None);
    Ok(())
}

#[test]
fn staleness_reports_added_removed_changed_and_category() -> TestResult {
    let f = drawn()?;
    let tdd = f.id("tdd");
    let leaky = f.id("leaky");

    // Removed: the placement leaves the view, the record keeps it.
    let trimmed = spec_without(&f, "tdd")?;
    f.library.save_profile(f.profile, &trimmed)?;
    let view = f.library.harnmap(f.profile)?;
    let stale = view.stale.ok_or("stale")?;
    assert_eq!(stale.removed.len(), 1);
    assert_eq!(stale.removed[0].id, tdd);
    assert_eq!(stale.removed[0].name, "tdd");
    assert_eq!(
        stale.removed[0].kind,
        Some(crate::artifact::ArtifactKind::Skill)
    );
    assert!(view.doc.placements.iter().all(|p| p.artifact != tdd));
    // Edits still work while a placement is orphaned.
    f.library.harnmap_edit(
        f.profile,
        MapEdit::Hide {
            artifact: leaky.clone(),
        },
    )?;
    assert!(f
        .library
        .harnmap_draw_at(f.profile, 1)?
        .placements
        .iter()
        .any(|p| p.artifact == tdd));

    // Added: a fresh draw without leaky, then leaky joins.
    let without = spec_without(&f, "leaky")?;
    f.library.save_profile(f.profile, &without)?;
    let mut g = f;
    g.claude(&[json!({
        "stages": [{"name": "Only"}],
        "loops": [],
        "placements": [{"artifact": "a1", "zone": "floor", "note": "", "why": "", "role": "", "when": "", "hands_off": ""}],
    })])?;
    draw(&g)?;
    let f = g;
    let mut with = without.clone();
    with.items.push(leaky.clone());
    f.library.save_profile(f.profile, &with)?;
    let stale = f.library.harnmap(f.profile)?.stale.ok_or("stale")?;
    assert_eq!(stale.added, vec![leaky.clone()]);
    assert!(stale.removed.is_empty() && stale.changed.is_empty());

    // Changed: the digest of a placed artifact differs.
    f.library.store.conn.execute(
        "UPDATE artifacts SET artifact = json_set(artifact, '$.digest', 'other') WHERE id = ?1",
        [&f.id("review")],
    )?;
    let stale = f.library.harnmap(f.profile)?.stale.ok_or("stale")?;
    assert_eq!(stale.changed, vec![f.id("review")]);

    // Category changed.
    f.library
        .set_category(f.profile, CategoryId::Research, None)?;
    let change = f
        .library
        .harnmap(f.profile)?
        .stale
        .and_then(|s| s.category_changed);
    assert_eq!(
        change,
        Some(CategoryChange {
            from: CategoryId::Software,
            to: CategoryId::Research
        })
    );
    Ok(())
}

#[test]
fn restore_copies_a_draw_as_the_next_one() -> TestResult {
    let mut f = drawn()?;
    f.claude(&[json!({
        "stages": [{"name": "Only"}],
        "loops": [],
        "placements": [],
    })])?;
    draw(&f)?;
    let first = f.library.harnmap_draw_at(f.profile, 1)?;
    assert_ne!(first, f.library.harnmap_draw_at(f.profile, 2)?);
    let no = f.library.harnmap_restore(f.profile, 1)?;
    assert_eq!(no, 3);
    assert_eq!(f.library.harnmap_draw_at(f.profile, 3)?, first);
    let view = f.library.harnmap(f.profile)?;
    assert_eq!(
        (view.no, view.draw_count, view.description.as_str()),
        (3, 3, "Spec first")
    );
    assert_eq!(view.model, "opus");
    // The older draws are untouched, and the restore is the current one now.
    assert_eq!(f.library.harnmap_draw_at(f.profile, 1)?, first);
    let missing = f.library.harnmap_draw_at(f.profile, 9);
    assert!(matches!(missing, Err(Error::DrawNotFound { no: 9 })));
    assert!(matches!(
        f.library.harnmap_restore(f.profile, 9),
        Err(Error::DrawNotFound { .. })
    ));
    Ok(())
}

#[test]
fn history_is_newest_first_with_computed_change_lines() -> TestResult {
    let mut f = drawn()?;
    let history = f.library.harnmap_history(f.profile)?;
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].change_line,
        "First drawing on the Software development template"
    );
    assert_eq!(history[0].restored_from, None);

    // Draw 2: tdd on Review, review dropped, leaky placed, stages renamed/added/removed.
    let stages = json!([
        {"id": "idea", "name": "Ideas"}, {"id": "spec", "name": "Spec"}, {"id": "plan", "name": "Plan"},
        {"id": "build", "name": "Build"}, {"id": "test", "name": "Test"}, {"id": "review", "name": "Review"},
        {"name": "Extra"}
    ]);
    f.claude(&[json!({
        "stages": stages,
        "loops": [],
        "placements": [
            {"artifact": "a1", "zone": "stage:review", "note": "", "why": "", "role": "", "when": "", "hands_off": ""},
            {"artifact": "a3", "zone": "floor", "note": "", "why": "", "role": "", "when": "", "hands_off": ""}
        ],
    })])?;
    draw(&f)?;
    let no = f.library.harnmap_restore(f.profile, 1)?;
    assert_eq!(no, 3);
    let history = f.library.harnmap_history(f.profile)?;
    assert_eq!(
        history.iter().map(|h| h.no).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(history[0].change_line, "Restored from draw 1");
    assert_eq!(history[0].restored_from, Some(1));
    let line = &history[1].change_line;
    assert!(line.contains("tdd moved to Review"), "{line}");
    assert!(line.contains("1 placed"), "{line}");
    assert!(line.contains("2 left unplaced"), "{line}");
    assert!(line.contains("stage Extra added"), "{line}");
    assert!(line.contains("stage Ship removed"), "{line}");
    assert!(line.contains("Idea renamed to Ideas"), "{line}");
    Ok(())
}

#[test]
fn many_moves_and_a_new_shape_are_summarised() -> TestResult {
    let mut f = drawn()?;
    let placements = |zone: &str| {
        json!([
            {"artifact": "a1", "zone": zone, "note": "", "why": "", "role": "", "when": "", "hands_off": ""},
            {"artifact": "a2", "zone": zone, "note": "", "why": "", "role": "", "when": "", "hands_off": ""},
            {"artifact": "a4", "zone": zone, "note": "", "why": "", "role": "", "when": "", "hands_off": ""}
        ])
    };
    let stages: Vec<_> = ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"]
        .iter()
        .map(|n| json!({"name": n}))
        .collect();
    f.claude(&[json!({"stages": stages, "loops": [], "placements": placements("toolbelt")})])?;
    draw(&f)?;
    let history = f.library.harnmap_history(f.profile)?;
    assert!(
        history[0].change_line.contains("3 artifacts moved"),
        "{}",
        history[0].change_line
    );
    f.claude(&[json!({"stages": stages, "loops": [], "placements": placements("toolbelt")})])?;
    draw(&f)?;
    let history = f.library.harnmap_history(f.profile)?;
    assert_eq!(history[0].change_line, "No changes to the map");
    Ok(())
}
