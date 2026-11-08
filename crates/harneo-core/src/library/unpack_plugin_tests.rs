//! Unpacking a plugin that is in the library whole (ADR-004).

use std::fs;
use std::path::Path;

use super::*;
use crate::artifact::ArtifactKind;
use crate::harnmap::{Basis, BasisArtifact, CategoryId, MapDoc, PlacedBy, Placement, Zone};
use crate::library::tags::TagColor;
use crate::plugin::WholeReason;
use crate::profiles::ProfileSpec;
use crate::updates::{CheckScope, SourceResult};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn skill(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: The {name} skill\n---\n{body}\n")
}

/// A plugin at the repo root with two skills; `extra` adds more files.
fn pack(root: &Path, extra: &[(&str, &str)]) -> std::io::Result<()> {
    write(root, ".claude-plugin/plugin.json", r#"{"name":"pack"}"#)?;
    for name in ["alpha", "beta"] {
        write(
            root,
            &format!("skills/{name}/SKILL.md"),
            &skill(name, "Body"),
        )?;
    }
    for (rel, text) in extra {
        write(root, rel, text)?;
    }
    Ok(())
}

struct Setup {
    _data: tempfile::TempDir,
    repo: tempfile::TempDir,
    library: Library,
    plugin: String,
}

/// The plugin imported whole into a fresh library.
fn setup(extra: &[(&str, &str)]) -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let repo = tempfile::tempdir()?;
    pack(repo.path(), extra)?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: repo.path().to_path_buf(),
    };
    let options = ScanOptions {
        whole: vec![String::new()],
        ..ScanOptions::default()
    };
    let preview = library.preview_with(&source, &options)?;
    let plugin = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin)
        .map(|c| c.id.clone())
        .ok_or("no plugin in the preview")?;
    library.import(&preview, std::slice::from_ref(&plugin), &TagChoices::new())?;
    Ok(Setup {
        _data: data,
        repo,
        library,
        plugin,
    })
}

impl Setup {
    fn rows(&self) -> i64 {
        self.library
            .store
            .conn
            .query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0))
            .unwrap_or(-1)
    }

    fn profile(&self, name: &str) -> std::result::Result<i64, Box<dyn std::error::Error>> {
        let spec = ProfileSpec {
            items: vec![self.plugin.clone()],
            ..ProfileSpec::default()
        };
        Ok(self
            .library
            .create_profile(name, "", &spec, CategoryId::Software, None)?)
    }

    /// A draw of the profile's map that places the plugin on the toolbelt.
    fn draw_with_plugin(&self, profile: i64) -> TestResult {
        let mut doc = MapDoc::from_template(CategoryId::Software);
        doc.placements.push(Placement {
            artifact: self.plugin.clone(),
            zone: Zone::Toolbelt,
            order: 0,
            note: String::new(),
            why: String::new(),
            role: String::new(),
            when: String::new(),
            hands_off: String::new(),
            next: Vec::new(),
            next_why: std::collections::BTreeMap::new(),
            parent: None,
            also_run_by: Vec::new(),
            by: PlacedBy::Claude,
            pinned: false,
        });
        doc.hidden.push(self.plugin.clone());
        let basis = Basis {
            profile_version: 1,
            artifacts: vec![BasisArtifact {
                id: self.plugin.clone(),
                digest: "d".into(),
            }],
        };
        self.library.store.conn.execute(
            "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms,
                 description, category, map, basis, restored_from)
             VALUES (?1, 1, 0, 'm', 0, '', ?2, ?3, ?4, NULL)",
            rusqlite::params![
                profile,
                CategoryId::Software.as_str(),
                serde_json::to_string(&doc)?,
                serde_json::to_string(&basis)?
            ],
        )?;
        Ok(())
    }
}

fn names(outcome: &UnpackOutcome) -> Vec<&str> {
    outcome.parts.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn a_skills_only_plugin_becomes_its_parts_and_profiles_get_them() -> TestResult {
    let mut s = setup(&[])?;
    let heavy = s.profile("Heavy")?;
    let light = s.profile("Light")?;
    s.library.save_profile(
        light,
        &ProfileSpec {
            items: vec![s.plugin.clone()],
            manual_only: [(s.plugin.clone(), true)].into(),
            ..ProfileSpec::default()
        },
    )?;
    s.draw_with_plugin(heavy)?;
    let copy = s.library.store.artifacts_dir().join(&s.plugin);
    assert!(copy.is_dir());

    let outcome = s.library.unpack_in_library(&s.plugin)?;
    assert!(outcome.refused.is_none());
    assert_eq!(names(&outcome), ["alpha", "beta"]);
    assert!(outcome.parts.iter().all(|p| p.kind == ArtifactKind::Skill));
    assert_eq!(outcome.placements_dropped, 1);
    let changes: Vec<(&str, u32, u32)> = outcome
        .profiles
        .iter()
        .map(|p| (p.name.as_str(), p.from, p.to))
        .collect();
    assert_eq!(changes, [("Heavy", 1, 2), ("Light", 2, 3)]);

    // The plugin is gone, its parts are ordinary, audited artifacts of the same source.
    assert!(matches!(
        s.library.detail(&s.plugin),
        Err(Error::NotInLibrary { .. })
    ));
    assert!(!copy.exists());
    assert_eq!(s.rows(), 2);
    let alpha = s.library.detail(&outcome.parts[0].id)?;
    assert_eq!(alpha.artifact.metadata["fromPlugin"]["name"], "pack");
    assert_eq!(alpha.artifact.metadata["layoutBy"], "plugin.json");
    assert_eq!(alpha.files, ["SKILL.md"]);
    assert!(alpha.audit.is_some());
    assert!(alpha.artifact.digest.is_some());
    assert_eq!(
        records::stored_unpack(&s.library.store.conn, &alpha.provenance.source.id)?,
        [""]
    );

    // Profiles hold the parts as a new version; the old version keeps the plugin.
    let ids: Vec<String> = outcome.parts.iter().map(|p| p.id.clone()).collect();
    let new = s.library.profile(heavy, None)?;
    assert_eq!((new.version, new.spec.items), (2, ids.clone()));
    assert_eq!(
        s.library.profile(heavy, Some(1))?.spec.items,
        [s.plugin.clone()]
    );
    let light = s.library.profile(light, None)?;
    assert_eq!(light.version, 3);
    assert!(light.spec.manual_only.is_empty());

    // The map forgot the plugin; the parts arrive as added, the plugin as removed.
    let view = s.library.harnmap(heavy)?;
    assert!(view.doc.placements.is_empty() && view.doc.hidden.is_empty());
    let stale = view.stale.ok_or("stale")?;
    assert_eq!(stale.added, ids);
    assert_eq!(stale.removed.len(), 1);
    assert_eq!(stale.removed[0].id, s.plugin);
    Ok(())
}

#[test]
fn updates_replay_parts_not_the_plugin() -> TestResult {
    let mut s = setup(&[])?;
    let outcome = s.library.unpack_in_library(&s.plugin)?;
    write(
        s.repo.path(),
        "skills/alpha/SKILL.md",
        &skill("alpha", "Changed"),
    )?;
    let report = s.library.check_updates(&CheckScope::All)?;
    assert!(
        matches!(
            report.sources[0].result,
            SourceResult::Changed {
                outdated: 1,
                removed: 0,
                ..
            }
        ),
        "{:?}",
        report.sources[0].result
    );
    assert_eq!(outcome.parts.len(), 2);
    Ok(())
}

#[test]
fn a_plugin_with_hooks_is_refused_with_its_reason_and_nothing_changes() -> TestResult {
    let mut s = setup(&[("hooks/hooks.json", r#"{"hooks":{}}"#)])?;
    let profile = s.profile("Heavy")?;
    let plan = s.library.unpack_plan(&s.plugin)?;
    let outcome = s.library.unpack_in_library(&s.plugin)?;
    assert_eq!(plan, outcome);
    assert!(matches!(
        outcome.refused,
        Some(WholeReason::WholeOnlyParts { .. })
    ));
    assert_eq!(
        outcome.reason.as_deref(),
        Some("Has hooks: they only work inside the plugin.")
    );
    assert!(outcome.parts.is_empty() && outcome.profiles.is_empty());
    assert_eq!(s.rows(), 1);
    assert_eq!(s.library.profile(profile, None)?.version, 1);
    assert!(s.library.detail(&s.plugin).is_ok());
    Ok(())
}

#[test]
fn a_plugin_with_nothing_to_unpack_is_refused() -> TestResult {
    let repo = tempfile::tempdir()?;
    write(
        repo.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"bare"}"#,
    )?;
    write(repo.path(), "README.md", "hi")?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let source = Source::Local {
        path: repo.path().to_path_buf(),
    };
    let preview = library.preview(&source)?;
    let plugin = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin)
        .map(|c| c.id.clone())
        .ok_or("plugin")?;
    library.import(&preview, std::slice::from_ref(&plugin), &TagChoices::new())?;
    let outcome = library.unpack_in_library(&plugin)?;
    assert_eq!(outcome.refused, Some(WholeReason::NoParts));
    assert!(library.detail(&plugin).is_ok());
    Ok(())
}

#[test]
fn the_plan_lists_what_would_change_and_changes_nothing() -> TestResult {
    let s = setup(&[])?;
    let heavy = s.profile("Heavy")?;
    s.draw_with_plugin(heavy)?;
    let plan = s.library.unpack_plan(&s.plugin)?;
    assert_eq!(names(&plan), ["alpha", "beta"]);
    assert_eq!(plan.profiles.len(), 1);
    assert_eq!(plan.placements_dropped, 1);
    assert_eq!(s.rows(), 1);
    assert_eq!(s.library.profile(heavy, None)?.version, 1);
    assert_eq!(s.library.harnmap(heavy)?.doc.placements.len(), 1);
    Ok(())
}

#[test]
fn mcp_servers_join_profiles_as_servers() -> TestResult {
    let mut s = setup(&[(
        ".mcp.json",
        r#"{"mcpServers":{"db":{"command":"db-server"}}}"#,
    )])?;
    let profile = s.profile("Heavy")?;
    let outcome = s.library.unpack_in_library(&s.plugin)?;
    let db = outcome
        .parts
        .iter()
        .find(|p| p.kind == ArtifactKind::Mcp)
        .ok_or("db")?;
    let spec = s.library.profile(profile, None)?.spec;
    assert_eq!(spec.items.len(), 2);
    assert_eq!(spec.mcp.len(), 1);
    assert_eq!(spec.mcp[0].artifact, db.id);
    Ok(())
}

#[test]
fn a_failure_leaves_the_library_as_it_was() -> TestResult {
    let mut s = setup(&[])?;
    let profile = s.profile("Heavy")?;
    // A stored version that names an artifact the library lacks cannot be saved again.
    s.library.store.conn.execute(
        "UPDATE profile_versions SET spec = json_set(spec, '$.manualOnly.gone', json('true'))
         WHERE profile_id = ?1",
        [profile],
    )?;
    let failed = s.library.unpack_in_library(&s.plugin);
    assert!(
        matches!(failed, Err(Error::NotInLibrary { .. })),
        "{failed:?}"
    );
    assert_eq!(s.rows(), 1);
    assert!(s.library.detail(&s.plugin).is_ok());
    assert!(s.library.store.artifacts_dir().join(&s.plugin).is_dir());
    let dirs = fs::read_dir(s.library.store.artifacts_dir())?.count();
    assert_eq!(dirs, 1);
    assert_eq!(s.library.profile(profile, None)?.version, 1);
    Ok(())
}

#[test]
fn parts_keep_the_plugins_tags_and_collections() -> TestResult {
    let mut s = setup(&[])?;
    let tag = s.library.create_tag("mine", TagColor::Red)?;
    s.library.tag_artifact(&s.plugin, tag.id)?;
    let held = s.library.create_manual_collection("held")?;
    s.library.add_to_collection(held.id, &s.plugin)?;
    let done = s.library.unpack_in_library(&s.plugin)?;
    assert!(done.parts.len() > 1);
    for part in &done.parts {
        let tags = s.library.artifact_tags(&part.id)?;
        assert_eq!(tags.len(), 1);
        assert_eq!((tags[0].id, tags[0].color), (tag.id, TagColor::Red));
        assert_eq!(s.library.artifact_collections(&part.id)?, vec![held.id]);
    }
    assert!(s.library.artifact_collections(&s.plugin)?.is_empty());
    let order: Vec<String> = s
        .library
        .store
        .conn
        .prepare(
            "SELECT artifact_id FROM collection_items WHERE collection_id = ?1 ORDER BY position",
        )?
        .query_map([held.id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let wanted: Vec<String> = done.parts.iter().map(|p| p.id.clone()).collect();
    assert_eq!(order, wanted);
    Ok(())
}

#[test]
fn only_a_whole_plugin_can_be_unpacked() -> TestResult {
    let mut s = setup(&[])?;
    let outcome = s.library.unpack_in_library(&s.plugin)?;
    let part = outcome.parts[0].id.clone();
    assert!(matches!(
        s.library.unpack_in_library(&part),
        Err(Error::NotAPlugin { .. })
    ));
    assert!(matches!(
        s.library.unpack_plan("0000000000000000"),
        Err(Error::NotInLibrary { .. })
    ));
    Ok(())
}
