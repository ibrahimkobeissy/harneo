//! ADR-008: the parts of a plugin a profile holds are written as a trimmed plugin under
//! `.claude/skills/<plugin>/`. No `enabledPlugins`, no marketplace, no `claude plugin install`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::drift::DriftStatus;
use crate::harnmap::CategoryId;
use crate::library::plugin_record_tests::{Lab, SKILLS};
use crate::library::{Preview, TagChoices};
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn files_under(root: &Path, rel: &str) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if let Ok(inside) = path.strip_prefix(base) {
                out.push(inside.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(rel), &root.join(rel), &mut out);
    out.sort();
    out
}

/// The fixture plugin imported with every part kept, and a profile of `names`.
struct Imported {
    lab: Lab,
    preview: Preview,
}

impl Imported {
    fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut lab = Lab::new()?;
        let preview = lab.preview()?;
        let all: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        lab.library.import(&preview, &all, &TagChoices::new())?;
        Ok(Self { lab, preview })
    }

    fn profile(&self, name: &str, parts: &[&str]) -> std::result::Result<i64, crate::Error> {
        let spec = ProfileSpec {
            items: parts.iter().map(|p| Lab::id_of(&self.preview, p)).collect(),
            ..ProfileSpec::default()
        };
        self.lab
            .library
            .create_profile(name, "", &spec, CategoryId::Software, None)
    }
}

fn scaffold(
    imported: &Imported,
    profile: i64,
    target: &Path,
) -> std::result::Result<crate::install::ApplyReport, crate::Error> {
    let plan = imported.lab.library.plan_scaffold(target, profile, None)?;
    imported.lab.library.apply(&plan)
}

#[test]
fn three_skills_and_a_command_scaffold_the_trimmed_plugin_and_nothing_else_of_it() -> TestResult {
    let imported = Imported::new()?;
    let profile = imported.profile(
        "p",
        &[
            "incremental-implementation",
            "test-driven-development",
            "planning",
            "build",
        ],
    )?;
    let project = tempfile::tempdir()?;
    let plan = imported
        .lab
        .library
        .plan_scaffold(project.path(), profile, None)?;
    assert_eq!(plan.trimmed_plugins(), ["agent-skills"]);
    imported.lab.library.apply(&plan)?;
    let dir = ".claude/skills/agent-skills";
    assert_eq!(
        files_under(project.path(), dir),
        [
            ".claude-plugin/plugin.json",
            "commands/build.md",
            "references/checklist.md",
            "references/security.md",
            "skills/incremental-implementation/SKILL.md",
            "skills/planning/SKILL.md",
            "skills/test-driven-development/SKILL.md",
        ]
    );
    // The manifest keeps the plugin's own keys and names only the kept parts.
    let manifest: Value = serde_json::from_slice(&fs::read(
        project.path().join(dir).join(".claude-plugin/plugin.json"),
    )?)?;
    assert_eq!(manifest["name"], "agent-skills");
    assert_eq!(manifest["version"], "1.0.0");
    assert_eq!(manifest["experimental"]["x"], 1);
    assert_eq!(
        manifest["skills"],
        serde_json::json!([
            "./skills/incremental-implementation",
            "./skills/test-driven-development",
            "./skills/planning"
        ])
    );
    assert_eq!(
        manifest["commands"],
        serde_json::json!(["./commands/build.md"])
    );
    // Parts are byte for byte what the plugin has.
    let source = fs::read(imported.lab.repo.join("skills/planning/SKILL.md"))?;
    assert_eq!(
        fs::read(project.path().join(dir).join("skills/planning/SKILL.md"))?,
        source
    );
    // No settings, no enabled plugin, no marketplace.
    assert!(!project.path().join(".claude/settings.json").exists());
    // HARNESS.md lists the folder under its plugin, with each part.
    let harness = fs::read_to_string(project.path().join("HARNESS.md"))?;
    assert!(harness.contains("### agent-skills (`.claude/skills/agent-skills/`)"));
    for part in [
        "Command `build`",
        "Skill `planning`",
        "Skill `test-driven-development`",
    ] {
        assert!(harness.contains(&format!("- {part}")), "{part}\n{harness}");
    }
    assert!(!harness.contains("code-review"));
    Ok(())
}

#[test]
fn a_fresh_scaffold_has_no_drift_and_no_unknown_files() -> TestResult {
    let imported = Imported::new()?;
    let profile = imported.profile("p", &["planning", "build"])?;
    let project = tempfile::tempdir()?;
    let applied = scaffold(&imported, profile, project.path())?;
    let report = imported.lab.library.check_project(applied.project_id)?;
    assert_eq!(report.status, DriftStatus::InSync, "{:?}", report.files);
    assert!(report.files.is_empty());
    // An edit to a part's file or a supporting file is drift on that file only.
    let edited = project
        .path()
        .join(".claude/skills/agent-skills/references/checklist.md");
    fs::write(&edited, "- changed\n")?;
    let report = imported.lab.library.check_project(applied.project_id)?;
    assert_eq!(report.files.len(), 1, "{:?}", report.files);
    assert_eq!(
        report.files[0].path,
        ".claude/skills/agent-skills/references/checklist.md"
    );
    Ok(())
}

#[test]
fn two_profiles_with_different_parts_write_different_folders_and_no_settings() -> TestResult {
    let imported = Imported::new()?;
    let one = imported.profile("one", &["planning"])?;
    let two = imported.profile("two", &["code-review", "test"])?;
    let (a, b) = (tempfile::tempdir()?, tempfile::tempdir()?);
    scaffold(&imported, one, a.path())?;
    scaffold(&imported, two, b.path())?;
    let (fa, fb) = (
        files_under(a.path(), ".claude/skills/agent-skills"),
        files_under(b.path(), ".claude/skills/agent-skills"),
    );
    assert_ne!(fa, fb);
    assert!(fa.contains(&"skills/planning/SKILL.md".to_owned()));
    assert!(!fa.contains(&"skills/code-review/SKILL.md".to_owned()));
    assert!(fb.contains(&"skills/code-review/SKILL.md".to_owned()));
    assert!(fb.contains(&"commands/test.md".to_owned()));
    let manifest = |root: &Path| -> std::result::Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&fs::read(root.join(
            ".claude/skills/agent-skills/.claude-plugin/plugin.json",
        ))?)?)
    };
    assert_eq!(
        manifest(a.path())?["skills"],
        serde_json::json!(["./skills/planning"])
    );
    assert!(manifest(a.path())?.get("commands").is_none());
    assert_eq!(
        manifest(b.path())?["skills"],
        serde_json::json!(["./skills/code-review"])
    );
    for root in [a.path(), b.path()] {
        assert!(!root.join(".claude/settings.json").exists());
        assert!(crate::install::plugin_targets(root)?.is_empty());
    }
    Ok(())
}

#[test]
fn dropping_the_last_part_removes_the_folder_and_changing_parts_updates_it() -> TestResult {
    let imported = Imported::new()?;
    let two = imported.profile("two", &["planning", "build"])?;
    let one = imported.profile("one", &["planning"])?;
    let project = tempfile::tempdir()?;
    let applied = scaffold(&imported, two, project.path())?;
    let dir = project.path().join(".claude/skills/agent-skills");
    assert!(dir.join("commands/build.md").is_file());

    // Dropping a part updates the folder.
    let plan = imported
        .lab
        .library
        .plan_switch(applied.project_id, one, None)?;
    imported.lab.library.apply(&plan)?;
    assert!(!dir.join("commands/build.md").exists());
    assert!(dir.join("skills/planning/SKILL.md").is_file());
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join(".claude-plugin/plugin.json"))?)?;
    assert!(manifest.get("commands").is_none());

    // Dropping the plugin's last part deletes the folder: code-review is now a loose copy.
    make_loose(&imported, "code-review")?;
    let none = imported.profile("none", &["code-review"])?;
    let plan = imported
        .lab
        .library
        .plan_switch(applied.project_id, none, None)?;
    imported.lab.library.apply(&plan)?;
    assert!(
        !dir.exists(),
        "{:?}",
        files_under(project.path(), ".claude")
    );
    assert!(project
        .path()
        .join(".claude/skills/code-review/SKILL.md")
        .is_file());
    Ok(())
}

/// A part with no plugin record, as a library from before ADR-008 holds it: it installs loose.
fn make_loose(imported: &Imported, name: &str) -> TestResult {
    let changed = imported.lab.library.store.conn.execute(
        "UPDATE artifacts SET artifact = json_remove(artifact, '$.metadata.fromPlugin.needs')
         WHERE id = ?1",
        rusqlite::params![Lab::id_of(&imported.preview, name)],
    )?;
    assert_eq!(changed, 1);
    Ok(())
}

#[test]
fn refit_deletes_the_trimmed_folder_when_the_profile_has_no_part_of_it() -> TestResult {
    let imported = Imported::new()?;
    let with = imported.profile("with", &["planning"])?;
    let project = tempfile::tempdir()?;
    scaffold(&imported, with, project.path())?;
    // A profile that holds only a loose skill (a part with no plugin record).
    make_loose(&imported, "code-review")?;
    let without = imported.profile("without", &["code-review"])?;
    let comparison = imported
        .lab
        .library
        .compare(project.path(), without, None)?;
    let row = comparison
        .rows
        .iter()
        .find(|r| r.item == ".claude/skills/agent-skills")
        .ok_or("the trimmed plugin is one item")?;
    assert_eq!(row.kind, crate::install::RowKind::ProjectOnly);
    let choices = BTreeMap::from([(
        ".claude/skills/agent-skills".to_owned(),
        crate::install::RefitAction::Delete,
    )]);
    let plan = imported.lab.library.plan_refit(&comparison, &choices)?;
    imported.lab.library.apply(&plan)?;
    assert!(!project.path().join(".claude/skills/agent-skills").exists());
    Ok(())
}

#[test]
fn skills_stay_manual_only_inside_the_trimmed_plugin() -> TestResult {
    let imported = Imported::new()?;
    let id = Lab::id_of(&imported.preview, "planning");
    let spec = ProfileSpec {
        items: vec![id.clone()],
        manual_only: BTreeMap::from([(id, true)]),
        ..ProfileSpec::default()
    };
    let profile =
        imported
            .lab
            .library
            .create_profile("m", "", &spec, CategoryId::Software, None)?;
    let project = tempfile::tempdir()?;
    scaffold(&imported, profile, project.path())?;
    let text = fs::read_to_string(
        project
            .path()
            .join(".claude/skills/agent-skills/skills/planning/SKILL.md"),
    )?;
    assert!(text.contains("disable-model-invocation: true"), "{text}");
    let _ = SKILLS;
    Ok(())
}
