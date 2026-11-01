//! Promoting a `settings.json` whose profile merges plugins into it (#196).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use super::fixture::{put, read, TestResult};
use super::DriftStatus;
use crate::artifact::ArtifactKind;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

const SETTINGS: &str = ".claude/settings.json";

const MARKET: &str = "{\"name\":\"mods\",\"plugins\":[\
{\"name\":\"radar\",\"source\":\"./plugins/radar\"}]}\n";

struct Setup {
    data: tempfile::TempDir,
    _sources: Vec<tempfile::TempDir>,
    library: Library,
    project: tempfile::TempDir,
    id: i64,
    settings: String,
}

fn import(
    library: &mut Library,
    files: &[(&str, &str)],
) -> std::result::Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    for (rel, text) in files {
        put(src.path(), rel, text)?;
    }
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(src)
}

/// A profile with a Settings template (`template`) and plugin `radar@mods`, scaffolded into a
/// new project.
fn setup(template: &Value) -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let text = format!("{}\n", serde_json::to_string_pretty(template)?);
    let a = import(&mut library, &[(SETTINGS, &text)])?;
    let b = import(
        &mut library,
        &[
            (".claude-plugin/marketplace.json", MARKET),
            (
                "plugins/radar/.claude-plugin/plugin.json",
                "{\"name\":\"radar\"}\n",
            ),
            (
                ".git/config",
                "[remote \"origin\"]\n\turl = https://github.com/me/mods.git\n",
            ),
        ],
    )?;
    let find = |kind, name| -> std::result::Result<String, String> {
        library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} missing"))
    };
    let settings = find(ArtifactKind::Settings, "settings")?;
    let spec = ProfileSpec {
        items: vec![find(ArtifactKind::Plugin, "radar")?],
        settings: Some(settings.clone()),
        mcp: Vec::new(),
        manual_only: BTreeMap::new(),
    };
    let profile = library.create_profile("p", "", &spec)?;
    let project = tempfile::tempdir()?;
    let plan = library.plan_scaffold(project.path(), profile, None)?;
    let id = library.apply(&plan)?.project_id;
    Ok(Setup {
        data,
        _sources: vec![a, b],
        library,
        project,
        id,
        settings,
    })
}

fn installed(s: &Setup) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&read(s.project.path(), SETTINGS)?)?)
}

fn write(s: &Setup, value: &Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    put(s.project.path(), SETTINGS, &format!("{text}\n"))
}

fn library_copy(s: &Setup) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    let path = Path::new(s.data.path())
        .join("library/artifacts")
        .join(&s.settings)
        .join("settings.json");
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// What the drift check says about promoting the settings file: `None` when it is not listed.
fn blocked(s: &Setup) -> std::result::Result<Option<Option<String>>, Box<dyn std::error::Error>> {
    Ok(s.library
        .check_project(s.id)?
        .files
        .into_iter()
        .find(|f| f.path == SETTINGS)
        .map(|f| f.promote_blocked))
}

fn template() -> Value {
    json!({"sandbox": {"excludedCommands": ["gh *"]}})
}

#[test]
fn promote_takes_the_merged_plugins_out_and_keeps_the_edit() -> TestResult {
    let s = setup(&template())?;
    let mut project = installed(&s)?;
    assert_eq!(project["enabledPlugins"]["radar@mods"], true);
    project["sandbox"]["excludedCommands"] = json!(["gh *", "board.sh *"]);
    write(&s, &project)?;

    assert_eq!(blocked(&s)?, Some(None), "promote is offered");
    s.library.promote(s.id, SETTINGS)?;
    let copy = library_copy(&s)?;
    assert_eq!(
        copy,
        json!({"sandbox": {"excludedCommands": ["gh *", "board.sh *"]}})
    );

    let check = s.library.check_project(s.id)?;
    assert_eq!(check.status, DriftStatus::InSync);
    assert!(check.library_newer.is_empty(), "{:?}", check.library_newer);
    // Rendering the promoted template gives back the project's file.
    let plan = s.library.plan_reapply(s.id, None)?;
    s.library.apply(&plan)?;
    assert_eq!(installed(&s)?, project);
    Ok(())
}

#[test]
fn the_templates_own_plugin_entries_and_hand_added_ones_stay() -> TestResult {
    let own = json!({
        "enabledPlugins": {"mine@else": true, "radar@mods": false},
        "model": "opus"
    });
    let s = setup(&own)?;
    let mut project = installed(&s)?;
    assert_eq!(
        project["enabledPlugins"]["radar@mods"], true,
        "the profile turns it on"
    );
    project["model"] = json!("sonnet");
    project["enabledPlugins"]["hand@made"] = json!(true);
    write(&s, &project)?;

    s.library.promote(s.id, SETTINGS)?;
    assert_eq!(
        library_copy(&s)?,
        json!({
            "enabledPlugins": {"hand@made": true, "mine@else": true, "radar@mods": false},
            "model": "sonnet"
        })
    );
    Ok(())
}

#[test]
fn a_changed_or_removed_profile_plugin_is_refused_and_nothing_moves() -> TestResult {
    let s = setup(&template())?;
    let before = library_copy(&s)?;
    let original = installed(&s)?;
    let mut project = original.clone();

    project["enabledPlugins"]["radar@mods"] = json!(false);
    write(&s, &project)?;
    let off = s
        .library
        .promote(s.id, SETTINGS)
        .err()
        .map(|e| e.to_string());
    assert_eq!(
        blocked(&s)?,
        Some(off.clone()),
        "the check gives the same reason up front"
    );
    assert!(off.is_some_and(|m| m.contains("radar@mods") && m.contains("profile")));

    if let Some(map) = project.as_object_mut() {
        map.remove("enabledPlugins");
    }
    write(&s, &project)?;
    let gone = s
        .library
        .promote(s.id, SETTINGS)
        .err()
        .map(|e| e.to_string());
    assert!(gone.is_some_and(|m| m.contains("radar@mods")));

    let mut market = original;
    market["extraKnownMarketplaces"]["mods"] =
        json!({"source": {"source": "github", "repo": "x/y"}});
    write(&s, &market)?;
    let moved = s
        .library
        .promote(s.id, SETTINGS)
        .err()
        .map(|e| e.to_string());
    assert!(moved.is_some_and(|m| m.contains("extraKnownMarketplaces")));

    put(s.project.path(), SETTINGS, "not json")?;
    assert!(s.library.promote(s.id, SETTINGS).is_err());

    assert_eq!(library_copy(&s)?, before);
    Ok(())
}
