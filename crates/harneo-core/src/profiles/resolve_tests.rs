use super::tests::{fixture, spec, TestResult};
use super::*;
use crate::harnmap::CategoryId;

#[test]
fn bundles_are_pulled_in_with_the_reason() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    let resolved = f.library.resolve(&spec(&[&a, &b]))?;
    let shown: Vec<(&str, &Why)> = resolved
        .artifacts
        .iter()
        .map(|i| (i.name.as_str(), &i.why))
        .collect();
    assert_eq!(
        shown,
        [
            ("a", &Why::Picked),
            (
                "helper",
                &Why::PulledBy {
                    artifact: a.clone()
                }
            ),
            ("b", &Why::Picked),
        ]
    );
    assert!(resolved.missing.is_empty());
    // Picking the member too makes it Picked, once.
    let both = f.library.resolve(&spec(&[&a, &helper]))?;
    assert_eq!(both.artifacts.len(), 2);
    assert_eq!(both.artifacts[1].why, Why::Picked);
    Ok(())
}

#[test]
fn bundles_are_pulled_transitively_and_cycles_are_safe() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    f.library
        .add_link(&helper, &b, crate::bundles::LinkOrigin::Manual, "")?;
    f.library
        .add_link(&b, &a, crate::bundles::LinkOrigin::Manual, "")?;
    let resolved = f.library.resolve(&spec(&[&a]))?;
    let names: Vec<&str> = resolved.artifacts.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["a", "helper", "b"]);
    assert!(resolved.artifacts[1..].iter().all(|i| i.why
        == Why::PulledBy {
            artifact: a.clone()
        }));
    Ok(())
}

#[test]
fn settings_and_mcp_are_listed_and_missing_ids_reported() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let settings = f.id(ArtifactKind::Settings, "settings")?;
    let api = f.id(ArtifactKind::Mcp, "api")?;
    let gone = "ffffffffffffffff".to_owned();
    let mut s = spec(&[&a, &gone]);
    s.settings = Some(settings);
    s.mcp = vec![McpChoice {
        artifact: api,
        env: BTreeMap::new(),
    }];
    let resolved = f.library.resolve(&s)?;
    let whys: Vec<&Why> = resolved.artifacts.iter().map(|i| &i.why).collect();
    assert_eq!(
        whys,
        [
            &Why::Picked,
            &Why::PulledBy { artifact: a },
            &Why::Settings,
            &Why::Mcp
        ]
    );
    assert_eq!(resolved.missing, [gone]);
    Ok(())
}

#[test]
fn worst_trust_is_the_lowest_audited_score() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    let b = f.id(ArtifactKind::Skill, "b")?;
    let helper = f.id(ArtifactKind::Agent, "helper")?;
    for (id, score) in [(&a, 90), (&helper, 40), (&b, 70)] {
        f.library.store.conn.execute(
            "UPDATE artifacts SET trust_score = ?2 WHERE id = ?1",
            rusqlite::params![id, score],
        )?;
    }
    assert_eq!(f.library.resolve(&spec(&[&a, &b]))?.worst_trust, Some(40));
    assert_eq!(f.library.resolve(&spec(&[&b]))?.worst_trust, Some(70));
    f.library
        .store
        .conn
        .execute("UPDATE artifacts SET trust_score = NULL", [])?;
    assert_eq!(f.library.resolve(&spec(&[&b]))?.worst_trust, None);
    Ok(())
}

#[test]
fn summaries_count_kinds_and_files_with_bundles() -> TestResult {
    let f = fixture()?;
    let a = f.id(ArtifactKind::Skill, "a")?;
    f.library
        .create_profile("P", "", &spec(&[&a]), CategoryId::Software, None)?;
    let summary = &f.library.profiles()?[0];
    let counts: Vec<(ArtifactKind, u32)> =
        summary.counts.iter().map(|c| (c.kind, c.count)).collect();
    assert_eq!(counts, [(ArtifactKind::Skill, 1), (ArtifactKind::Agent, 1)]);
    assert!(summary.file_count >= 2);
    Ok(())
}
