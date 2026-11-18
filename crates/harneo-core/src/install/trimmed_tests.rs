use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::trimmed::{narrow, KeptPart};
use crate::artifact::ArtifactKind;

fn part(kind: ArtifactKind, path: &str) -> KeptPart {
    KeptPart {
        kind,
        path: path.to_owned(),
    }
}

fn run(manifest: &Value, kept: &[KeptPart], files: &[&str]) -> Value {
    let files: BTreeSet<String> = files.iter().map(|f| (*f).to_owned()).collect();
    let out = narrow(&manifest.to_string(), kept, &files).expect("narrows");
    serde_json::from_slice(&out).expect("json")
}

#[test]
fn component_lists_keep_only_the_kept_parts_and_a_key_naming_none_goes() {
    let manifest = json!({
        "name": "p", "version": "2", "userConfig": {"t": {}},
        "skills": ["./skills/a", "./skills/b"],
        "commands": ["./commands/x.md"],
        "agents": "./agents/z.md",
    });
    let kept = [
        part(ArtifactKind::Skill, "skills/b"),
        part(ArtifactKind::Command, "commands/x.md"),
    ];
    let out = run(&manifest, &kept, &[]);
    assert_eq!(out["skills"], json!(["./skills/b"]));
    assert_eq!(out["commands"], json!(["./commands/x.md"]));
    assert!(
        out.get("agents").is_none(),
        "the agents key pointed only at dropped parts"
    );
    // Everything else is the plugin's own.
    assert_eq!(out["userConfig"], json!({"t": {}}));
    assert_eq!(out["version"], "2");
}

#[test]
fn a_folder_entry_stays_when_it_holds_a_kept_part_and_object_entries_are_narrowed() {
    let manifest = json!({
        "name": "p",
        "skills": "./skills/",
        "commands": {"go": {"source": "./cmds/go.md"}, "stop": {"source": "./cmds/stop.md"}},
    });
    let kept = [
        part(ArtifactKind::Skill, "skills/a"),
        part(ArtifactKind::Command, "cmds/go.md"),
    ];
    let out = run(&manifest, &kept, &[]);
    assert_eq!(out["skills"], "./skills/");
    assert_eq!(out["commands"], json!({"go": {"source": "./cmds/go.md"}}));
}

#[test]
fn a_part_outside_the_default_folder_is_named_when_the_manifest_does_not() {
    let manifest = json!({"name": "p"});
    let kept = [
        part(ArtifactKind::Skill, "skills/a"),
        part(ArtifactKind::Skill, "extra/one"),
    ];
    let out = run(&manifest, &kept, &[]);
    assert_eq!(out["skills"], json!(["./skills/a", "./extra/one"]));
    // Parts in their default place need no key.
    let plain = run(&manifest, &[part(ArtifactKind::Skill, "skills/a")], &[]);
    assert!(plain.get("skills").is_none());
}

#[test]
fn hooks_stay_only_when_their_file_is_in_the_folder() {
    let manifest = json!({"name": "p", "hooks": "./hooks/hooks.json"});
    assert!(run(&manifest, &[], &[]).get("hooks").is_none());
    let kept = run(&manifest, &[], &["hooks/hooks.json"]);
    assert_eq!(kept["hooks"], "./hooks/hooks.json");
    let inline = json!({"name": "p", "hooks": {"Stop": []}});
    assert_eq!(run(&inline, &[], &[])["hooks"], json!({"Stop": []}));
}

#[test]
fn a_manifest_that_needs_no_change_is_written_byte_for_byte(
) -> Result<(), Box<dyn std::error::Error>> {
    let text = "{ \"name\":\"p\",   \"skills\": [\"./skills/a\"] }\n";
    let kept = [part(ArtifactKind::Skill, "skills/a")];
    let out = narrow(text, &kept, &BTreeSet::new())?;
    assert_eq!(out, text.as_bytes());
    assert!(narrow("[1]", &kept, &BTreeSet::new()).is_err());
    Ok(())
}
