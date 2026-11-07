use std::collections::HashSet;
use std::str::FromStr;

use super::*;

#[test]
fn all_eight_templates_load_in_picker_order() {
    let all = templates();
    let ids: Vec<_> = all.iter().map(|t| t.id).collect();
    assert_eq!(ids, CategoryId::ALL);
    assert_eq!(all.len(), 8);
}

#[test]
fn stage_ids_are_unique_and_loops_point_at_real_stages() {
    for t in templates() {
        let ids: HashSet<_> = t.stages.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), t.stages.len(), "{} repeats a stage id", t.label);
        assert!(!t.stages.is_empty(), "{} has no stage", t.label);
        for l in &t.loops {
            assert!(ids.contains(l.from), "{}: loop from {}", t.label, l.from);
            assert!(ids.contains(l.to), "{}: loop to {}", t.label, l.to);
            assert_ne!(l.from, l.to);
        }
        for p in &t.phases {
            for s in &p.stages {
                assert!(ids.contains(s), "{}: phase stage {s}", t.label);
            }
        }
        for id in &t.parallel {
            assert!(ids.contains(id), "{}: parallel stage {id}", t.label);
        }
        assert!(t.suggestions.iter().all(|s| !s.is_empty()));
    }
}

#[test]
fn shapes_and_stage_counts_match_the_spec() {
    let all = templates();
    let by = |id| all.iter().find(|t| t.id == id).expect("template exists");
    assert_eq!(by(CategoryId::Software).shape, Some(Shape::Line));
    assert_eq!(by(CategoryId::Video).shape, Some(Shape::Parallel));
    assert_eq!(by(CategoryId::Social).shape, Some(Shape::Cycle));
    assert_eq!(by(CategoryId::Design).shape, Some(Shape::Diamond));
    assert_eq!(by(CategoryId::Custom).shape, None);
    assert_eq!(by(CategoryId::Custom).stages.len(), 1);
    assert_eq!(by(CategoryId::Software).stages.len(), 7);
    assert_eq!(by(CategoryId::Video).phases.len(), 3);
    assert_eq!(
        by(CategoryId::Software).suggestions,
        [
            "Spec first, always.",
            "TDD lives inside Build.",
            "Review sends work back to Build."
        ]
    );
}

#[test]
fn category_ids_round_trip_through_text() {
    for id in CategoryId::ALL {
        assert_eq!(CategoryId::from_str(id.as_str()).ok(), Some(id));
    }
    assert!(CategoryId::from_str("nope").is_err());
}

#[test]
fn only_video_has_parallel_tracks() {
    for t in templates() {
        let expected: &[&str] = if t.id == CategoryId::Video {
            &["edit", "sound-captions"]
        } else {
            &[]
        };
        assert_eq!(t.parallel, expected, "{}", t.label);
    }
}
