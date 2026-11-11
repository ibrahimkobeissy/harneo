//! Importing the parts of a plugin: what is stored, the exclusivity rules and the install.

use crate::harnmap::CategoryId;
use std::fs;
use std::path::Path;

use super::*;
use crate::artifact::ArtifactKind;
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: The {name} skill\n---\nBody\n")
}

/// A plugin at the repo root: three skills and two MCP servers in one `.mcp.json`.
pub(crate) fn plugin_repo(root: &Path) -> std::io::Result<()> {
    write(
        root,
        ".claude-plugin/plugin.json",
        r#"{"name":"toolbox","description":"Tools"}"#,
    )?;
    for name in ["alpha", "beta", "gamma"] {
        write(root, &format!("skills/{name}/SKILL.md"), &skill(name))?;
    }
    write(
        root,
        ".mcp.json",
        r#"{"mcpServers":{"db":{"command":"db-server","env":{"API_KEY":"harneo-test-literal-value-123"}},"cache":{"command":"cache-server"}}}"#,
    )
}

pub(crate) fn unpacking(folders: &[&str]) -> ScanOptions {
    ScanOptions {
        unpack: folders.iter().map(|f| (*f).to_owned()).collect(),
        ..ScanOptions::default()
    }
}

struct Setup {
    _data: tempfile::TempDir,
    _repo: tempfile::TempDir,
    library: Library,
    source: Source,
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let repo = tempfile::tempdir()?;
    plugin_repo(repo.path())?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: repo.path().to_path_buf(),
    };
    Ok(Setup {
        _data: data,
        _repo: repo,
        library,
        source,
    })
}

fn id(preview: &Preview, kind: ArtifactKind, name: &str) -> std::result::Result<String, String> {
    preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == kind && c.artifact.name == name)
        .map(|c| c.id.clone())
        .ok_or_else(|| format!("{name} not in the preview"))
}

impl Setup {
    fn parts(&self) -> Result<Preview> {
        self.library.preview_with(&self.source, &unpacking(&[""]))
    }

    /// The preview with the plugin taken whole (the default unpacks it).
    fn whole(&self) -> Result<Preview> {
        let whole = ScanOptions {
            whole: vec!["./".into()],
            ..ScanOptions::default()
        };
        self.library.preview_with(&self.source, &whole)
    }

    fn rows(&self) -> i64 {
        self.library
            .store
            .conn
            .query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0))
            .unwrap_or(-1)
    }

    fn stored_unpack(&self, preview: &Preview) -> Result<Vec<String>> {
        records::stored_unpack(
            &self.library.store.conn,
            &ids::source_id(&preview.source.identity()?),
        )
    }
}

#[test]
fn two_of_three_skills_are_stored_with_their_plugin_and_no_plugin() -> TestResult {
    let mut s = setup()?;
    let preview = s.parts()?;
    let alpha = id(&preview, ArtifactKind::Skill, "alpha")?;
    let beta = id(&preview, ArtifactKind::Skill, "beta")?;
    let report = s
        .library
        .import(&preview, &[alpha.clone(), beta.clone()], &TagChoices::new())?;
    assert_eq!(report.imported, [alpha.clone(), beta]);
    assert_eq!(s.rows(), 2);
    let kinds: Vec<String> = s
        .library
        .store
        .conn
        .prepare("SELECT kind FROM artifacts")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    assert_eq!(kinds, ["skill", "skill"]);
    let detail = s.library.detail(&alpha)?;
    assert_eq!(detail.artifact.metadata["fromPlugin"]["name"], "toolbox");
    assert_eq!(detail.artifact.metadata["layoutBy"], "plugin.json");
    assert_eq!(detail.files, ["SKILL.md"]);
    assert_eq!(s.stored_unpack(&preview)?, [""]);
    Ok(())
}

#[test]
fn an_unpacked_mcp_part_is_stored_as_its_own_server_json() -> TestResult {
    let mut s = setup()?;
    let preview = s.parts()?;
    let db = id(&preview, ArtifactKind::Mcp, "db")?;
    s.library
        .import(&preview, std::slice::from_ref(&db), &TagChoices::new())?;
    let detail = s.library.detail(&db)?;
    assert_eq!(detail.files, ["server.json"]);
    let text = fs::read_to_string(
        s.library
            .store
            .artifacts_dir()
            .join(&db)
            .join("server.json"),
    )?;
    let stored: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(
        stored,
        serde_json::json!({"db": {"command": "db-server", "env": {"API_KEY": "${API_KEY}"}}})
    );
    assert!(
        !text.contains("cache"),
        "the other server stays out: {text}"
    );
    Ok(())
}

#[test]
fn a_plugin_and_its_parts_cannot_be_picked_together() -> TestResult {
    let mut s = setup()?;
    let whole = s.whole()?;
    let mut mixed = s.parts()?;
    let plugin = whole
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin)
        .ok_or("plugin")?
        .clone();
    mixed.candidates.push(plugin.clone());
    let alpha = id(&mixed, ArtifactKind::Skill, "alpha")?;
    let refused = s
        .library
        .import(&mixed, &[plugin.id, alpha], &TagChoices::new())
        .expect_err("both picked");
    assert!(
        matches!(refused, Error::PluginExclusive { .. }),
        "{refused}"
    );
    assert!(refused.to_string().contains("toolbox"), "{refused}");
    assert_eq!(s.rows(), 0);
    Ok(())
}

#[test]
fn parts_are_refused_while_the_plugin_is_in_whole_and_the_reverse() -> TestResult {
    let mut s = setup()?;
    let whole = s.whole()?;
    let plugin = id(&whole, ArtifactKind::Plugin, "toolbox")?;
    s.library
        .import(&whole, std::slice::from_ref(&plugin), &TagChoices::new())?;
    let parts = s.parts()?;
    let alpha = id(&parts, ArtifactKind::Skill, "alpha")?;
    let refused = s
        .library
        .import(&parts, std::slice::from_ref(&alpha), &TagChoices::new())
        .expect_err("plugin is whole");
    assert!(
        matches!(refused, Error::PluginExclusive { .. }),
        "{refused}"
    );
    let message = refused.to_string();
    assert!(
        message.contains("toolbox") && message.contains("remove"),
        "{message}"
    );
    assert_eq!(s.rows(), 1);
    assert!(s.stored_unpack(&parts)?.is_empty());

    s.library.remove(&plugin)?;
    s.library
        .import(&parts, std::slice::from_ref(&alpha), &TagChoices::new())?;
    let whole = s.whole()?;
    let plugin = id(&whole, ArtifactKind::Plugin, "toolbox")?;
    let refused = s
        .library
        .import(&whole, std::slice::from_ref(&plugin), &TagChoices::new())
        .expect_err("parts are in");
    assert!(
        matches!(refused, Error::PluginExclusive { .. }),
        "{refused}"
    );
    assert!(refused.to_string().contains("remove"), "{refused}");
    assert_eq!(s.rows(), 1);

    // With the parts gone, the plugin goes in whole and the unpack choice is dropped.
    s.library.remove(&alpha)?;
    s.library.import(&whole, &[plugin], &TagChoices::new())?;
    assert!(s.stored_unpack(&whole)?.is_empty());
    Ok(())
}

/// Adds a skill that uses `${CLAUDE_PLUGIN_ROOT}` to the setup's plugin.
fn needy_skill(s: &Setup) -> std::io::Result<()> {
    let Source::Local { path } = &s.source else {
        return Ok(());
    };
    write(
        path,
        "skills/needy/SKILL.md",
        "---\nname: needy\ndescription: Needs the plugin\n---\nRun ${CLAUDE_PLUGIN_ROOT}/shared/tool.sh\n",
    )
}

#[test]
fn a_part_that_needs_its_plugin_keeps_the_whole_plugin_whole() -> TestResult {
    let mut s = setup()?;
    needy_skill(&s)?;
    let preview = s.parts()?;
    assert!(names(&preview, ArtifactKind::Skill).is_empty());
    let plugin = id(&preview, ArtifactKind::Plugin, "toolbox")?;
    let reason = preview
        .candidates
        .iter()
        .find(|c| c.id == plugin)
        .and_then(|c| c.must_stay_whole.clone());
    assert!(matches!(
        reason,
        Some(crate::plugin::WholeReason::PluginVariable { .. })
    ));
    s.library.import(&preview, &[plugin], &TagChoices::new())?;
    assert_eq!(s.rows(), 1);
    Ok(())
}

#[test]
fn an_unpacked_skill_scaffolds_into_claude_skills_without_enabling_the_plugin() -> TestResult {
    let mut s = setup()?;
    let preview = s.parts()?;
    let alpha = id(&preview, ArtifactKind::Skill, "alpha")?;
    s.library
        .import(&preview, std::slice::from_ref(&alpha), &TagChoices::new())?;
    let spec = ProfileSpec {
        items: vec![alpha],
        settings: None,
        mcp: Vec::new(),
        manual_only: std::collections::BTreeMap::new(),
    };
    let profile = s
        .library
        .create_profile("tools", "", &spec, CategoryId::Software, None)?;
    let project = tempfile::tempdir()?;
    let plan = s.library.plan_scaffold(project.path(), profile, None)?;
    s.library.apply(&plan)?;
    let installed = fs::read_to_string(project.path().join(".claude/skills/alpha/SKILL.md"))?;
    assert!(installed.contains("The alpha skill"));
    let settings =
        fs::read_to_string(project.path().join(".claude/settings.json")).unwrap_or_default();
    assert!(!settings.contains("enabledPlugins"), "{settings}");
    Ok(())
}

#[test]
fn the_same_plugin_from_another_source_is_caught_too() -> TestResult {
    let mut s = setup()?;
    let whole = s.whole()?;
    let plugin = id(&whole, ArtifactKind::Plugin, "toolbox")?;
    s.library
        .import(&whole, std::slice::from_ref(&plugin), &TagChoices::new())?;

    // A second copy of the same repo (a clone next to the first, say), unpacked.
    let other = tempfile::tempdir()?;
    plugin_repo(other.path())?;
    let elsewhere = Source::Local {
        path: other.path().to_path_buf(),
    };
    let parts = s.library.preview_with(&elsewhere, &unpacking(&[""]))?;
    let alpha = id(&parts, ArtifactKind::Skill, "alpha")?;
    let refused = s
        .library
        .import(&parts, std::slice::from_ref(&alpha), &TagChoices::new())
        .expect_err("the plugin is in the library whole, from another source");
    assert!(
        matches!(refused, Error::PluginExclusive { .. }),
        "{refused}"
    );
    assert_eq!(s.rows(), 1);

    // A different plugin from another source is not affected.
    let unrelated = tempfile::tempdir()?;
    write(
        unrelated.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"other","description":"Other"}"#,
    )?;
    write(unrelated.path(), "skills/delta/SKILL.md", &skill("delta"))?;
    let theirs = Source::Local {
        path: unrelated.path().to_path_buf(),
    };
    let parts = s.library.preview_with(&theirs, &unpacking(&[""]))?;
    let delta = id(&parts, ArtifactKind::Skill, "delta")?;
    s.library.import(&parts, &[delta], &TagChoices::new())?;
    assert_eq!(s.rows(), 2);
    Ok(())
}

#[test]
fn the_library_reports_what_it_holds_of_a_previews_plugins() -> TestResult {
    let mut s = setup()?;
    let whole = s.whole()?;
    let fresh = s.library.plugin_holds(&whole)?;
    assert_eq!(
        fresh,
        [PluginHold {
            name: "toolbox".into(),
            folder: whole.candidates[0].artifact.path.clone(),
            whole: false,
            parts: false,
        }]
    );
    let plugin = id(&whole, ArtifactKind::Plugin, "toolbox")?;
    s.library
        .import(&whole, std::slice::from_ref(&plugin), &TagChoices::new())?;

    // The parts' preview names the same plugin once, and sees it in the library whole.
    let parts = s.parts()?;
    let held = s.library.plugin_holds(&parts)?;
    assert_eq!(held.len(), 1);
    assert!(held[0].whole && !held[0].parts, "{held:?}");

    s.library.remove(&plugin)?;
    let alpha = id(&parts, ArtifactKind::Skill, "alpha")?;
    s.library.import(&parts, &[alpha], &TagChoices::new())?;
    let held = s.library.plugin_holds(&s.whole()?)?;
    assert!(!held[0].whole && held[0].parts, "{held:?}");
    Ok(())
}

#[test]
fn a_listed_part_names_its_plugin() -> TestResult {
    let mut s = setup()?;
    let parts = s.parts()?;
    let alpha = id(&parts, ArtifactKind::Skill, "alpha")?;
    s.library.import(&parts, &[alpha], &TagChoices::new())?;
    let page = s.library.list(&Query::default())?;
    assert_eq!(page.items[0].from_plugin.as_deref(), Some("toolbox"));
    Ok(())
}

#[test]
fn the_preview_carries_why_a_plugin_must_stay_whole() -> TestResult {
    let s = setup()?;
    needy_skill(&s)?;
    let whole = s.whole()?;
    let plugin = whole
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin)
        .ok_or("no plugin")?;
    assert_eq!(
        plugin.must_stay_whole,
        Some(crate::plugin::WholeReason::PluginVariable {
            variable: "${CLAUDE_PLUGIN_ROOT}".to_owned(),
            parts: vec![crate::plugin::PluginPart::Skills]
        })
    );
    // Asking to unpack it changes nothing: it is still the one whole plugin.
    let parts = s.parts()?;
    assert_eq!(plugins_of(&parts), 1);
    assert!(names(&parts, ArtifactKind::Skill).is_empty());
    Ok(())
}

/// A skills-only plugin pack, like addyosmani/agent-skills: nothing in it needs the plugin.
pub(crate) fn skills_pack(root: &Path) -> std::io::Result<()> {
    write(
        root,
        ".claude-plugin/plugin.json",
        r#"{"name":"pack","description":"Skills"}"#,
    )?;
    for name in ["one", "two"] {
        write(root, &format!("skills/{name}/SKILL.md"), &skill(name))?;
    }
    Ok(())
}

fn plugins_of(preview: &Preview) -> usize {
    preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind == ArtifactKind::Plugin)
        .count()
}

fn local(dir: &tempfile::TempDir) -> Source {
    Source::Local {
        path: dir.path().to_path_buf(),
    }
}

fn names(preview: &Preview, kind: ArtifactKind) -> Vec<String> {
    preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind == kind)
        .map(|c| c.artifact.name.clone())
        .collect()
}

#[test]
fn a_skills_pack_with_no_choice_previews_as_its_parts_and_stores_the_choice() -> TestResult {
    let repo = tempfile::tempdir()?;
    skills_pack(repo.path())?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&local(&repo))?;
    assert_eq!(plugins_of(&preview), 0);
    assert_eq!(names(&preview, ArtifactKind::Skill), ["one", "two"]);
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let stored = records::stored_unpack(
        &library.store.conn,
        &ids::source_id(&preview.source.identity()?),
    )?;
    assert_eq!(stored, [""]);
    Ok(())
}

#[test]
fn an_explicit_whole_choice_beats_the_default() -> TestResult {
    let repo = tempfile::tempdir()?;
    skills_pack(repo.path())?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let whole = ScanOptions {
        whole: vec!["./".into()],
        ..ScanOptions::default()
    };
    let preview = library.preview_with(&local(&repo), &whole)?;
    assert_eq!(plugins_of(&preview), 1);
    assert!(names(&preview, ArtifactKind::Skill).is_empty());
    let off = ScanOptions {
        unpack_by_default: false,
        ..ScanOptions::default()
    };
    assert_eq!(
        plugins_of(&library.preview_with(&local(&repo), &off)?),
        1,
        "the default can be switched off"
    );
    Ok(())
}

#[test]
fn a_plugin_with_hooks_is_proposed_whole_with_its_reason() -> TestResult {
    let repo = tempfile::tempdir()?;
    skills_pack(repo.path())?;
    write(
        repo.path(),
        "hooks/hooks.json",
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    )?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let preview = library.preview(&local(&repo))?;
    let plugin = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin)
        .ok_or("the plugin stays whole")?;
    let reason = plugin.must_stay_whole.as_ref().ok_or("a reason")?;
    assert_eq!(
        reason.text(),
        "Has hooks: they only work inside the plugin."
    );
    // A mixed plugin cannot be unpacked either (ADR-006): asking leaves it whole.
    let parts = library.preview_with(&local(&repo), &unpacking(&[""]))?;
    assert_eq!(plugins_of(&parts), 1);
    assert!(names(&parts, ArtifactKind::Skill).is_empty());
    Ok(())
}

#[test]
fn a_plugin_already_imported_whole_from_the_source_stays_whole() -> TestResult {
    let repo = tempfile::tempdir()?;
    skills_pack(repo.path())?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let off = ScanOptions {
        unpack_by_default: false,
        ..ScanOptions::default()
    };
    let before = library.preview_with(&local(&repo), &off)?;
    let ids: Vec<String> = before.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&before, &ids, &TagChoices::new())?;
    let again = library.preview(&local(&repo))?;
    assert_eq!(plugins_of(&again), 1);
    Ok(())
}

#[test]
fn a_new_default_import_of_a_plugin_held_whole_elsewhere_is_refused_not_converted() -> TestResult {
    let first = tempfile::tempdir()?;
    skills_pack(first.path())?;
    let second = tempfile::tempdir()?;
    skills_pack(second.path())?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let off = ScanOptions {
        unpack_by_default: false,
        ..ScanOptions::default()
    };
    let held = library.preview_with(&local(&first), &off)?;
    let ids: Vec<String> = held.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&held, &ids, &TagChoices::new())?;
    let preview = library.preview(&local(&second))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    let refused = library.import(&preview, &ids, &TagChoices::new());
    assert!(matches!(refused, Err(Error::PluginExclusive { .. })));
    Ok(())
}
