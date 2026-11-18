//! ADR-008: a kept part that names a dropped part of its plugin is detected at import and in
//! the profile builder; nothing is decided for the user.

use serde_json::json;

use super::plugin_record_tests::Lab;
use super::*;
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn spec_of(ids: &[&str]) -> ProfileSpec {
    ProfileSpec {
        items: ids.iter().map(|i| (*i).to_owned()).collect(),
        ..ProfileSpec::default()
    }
}

fn pairs(preview: &Preview, from: &str) -> Vec<String> {
    let id = Lab::id_of(preview, from);
    preview
        .part_refs
        .iter()
        .filter(|r| r.from == id)
        .filter_map(|r| preview.candidates.iter().find(|c| c.id == r.to))
        .map(|c| c.artifact.name.clone())
        .collect()
}

#[test]
fn the_preview_says_which_parts_name_which() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    // `/build` names a skill with and without the slash.
    let mut build = pairs(&preview, "build");
    build.sort();
    assert_eq!(
        build,
        ["incremental-implementation", "test-driven-development"]
    );
    assert_eq!(pairs(&preview, "test"), ["test-driven-development"]);
    // Skills link a file, not a part.
    assert!(pairs(&preview, "planning").is_empty());
    assert!(pairs(&preview, "code-review").is_empty());
    Ok(())
}

#[test]
fn a_name_that_is_only_the_end_of_another_plugin_is_not_a_reference() {
    let found = crate::plugin::namespaced_uses(
        "see my-agent-skills:build, agent-skills:plan. and /agent-skills:test-driven-development-",
        "agent-skills",
    );
    assert_eq!(
        found.into_iter().collect::<Vec<_>>(),
        ["plan", "test-driven-development"]
    );
}

#[test]
fn the_builder_lists_a_kept_part_that_names_a_dropped_one_and_nothing_otherwise() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let kept: Vec<String> = ["build", "incremental-implementation", "planning"]
        .iter()
        .map(|n| Lab::id_of(&preview, n))
        .collect();
    lab.library.import(&preview, &kept, &TagChoices::new())?;
    let build = kept[0].clone();
    let detail = lab.library.detail(&build)?;
    assert_eq!(
        detail.artifact.metadata["fromPlugin"]["uses"],
        json!(["incremental-implementation", "test-driven-development"])
    );

    // `/build` is kept, `incremental-implementation` is in the library but not the profile.
    let needs = lab.library.part_needs(&spec_of(&[&build]))?;
    let names: Vec<(&str, &str, bool)> = needs
        .iter()
        .map(|n| {
            (
                n.part_name.as_str(),
                n.needs.as_str(),
                n.available.is_some(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("build", "incremental-implementation", true),
            // Dropped at import: not in the library, so only the name can be shown.
            ("build", "test-driven-development", false),
        ]
    );
    assert_eq!(needs[0].available.as_deref(), Some(kept[1].as_str()));
    assert_eq!(needs[0].plugin, "agent-skills");

    // Adding the needed part clears its line; the other remains.
    let both = lab.library.part_needs(&spec_of(&[&build, &kept[1]]))?;
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].needs, "test-driven-development");
    // No dangling names, no notice.
    assert!(lab.library.part_needs(&spec_of(&[&kept[2]]))?.is_empty());
    Ok(())
}

#[test]
fn an_updated_part_keeps_the_names_it_used() -> TestResult {
    let mut lab = Lab::new()?;
    let preview = lab.preview()?;
    let build = Lab::id_of(&preview, "build");
    lab.library
        .import(&preview, std::slice::from_ref(&build), &TagChoices::new())?;
    std::fs::write(
        lab.repo.join("commands/build.md"),
        "---\ndescription: Build v2\n---\nUse agent-skills:incremental-implementation.\n",
    )?;
    lab.library
        .check_updates(&crate::updates::CheckScope::All)?;
    lab.library
        .update(&build, crate::updates::UpdateChoice::TakeUpstream)?;
    let detail = lab.library.detail(&build)?;
    assert_eq!(
        detail.artifact.metadata["fromPlugin"]["uses"],
        json!(["incremental-implementation", "test-driven-development"]),
        "the names recorded at import stay until the part is imported again"
    );
    Ok(())
}
