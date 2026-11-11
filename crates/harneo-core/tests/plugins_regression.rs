//! Plugins after ADR-004 (unpack by default, whole only when it must be), end to end: one test
//! per path, through the public API the app and the CLI use, against fixture repos built in
//! temp folders (git ones reached by `file://` URL) and a fixture script standing in for
//! `claude`. Nothing here touches the network or runs the real CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use harneo_core::artifact::{ArtifactKind, ScanOptions};
use harneo_core::bundles::ClaudeConfig;
use harneo_core::drift::{DriftChange, DriftStatus};
use harneo_core::harnmap::{Candidate, CategoryId, PlacedBy};
use harneo_core::install::{
    install_plugins, plugin_targets, PlanKind, PluginInstallStatus, WriteAction,
};
use harneo_core::library::{Library, Preview, Query, Source, TagChoices, TagColor, UnpackOutcome};
use harneo_core::plugin::{PluginPart, WholeReason};
use harneo_core::profiles::{ExportOptions, ProfileSpec};
use harneo_core::updates::{CheckScope, SourceResult, UpdateChoice, UpdateOutcome, UpstreamState};
use harneo_core::usage::{Status, UsageIndex};
use harneo_core::Error;
use serde_json::{json, Value};

type R<T = ()> = Result<T, Box<dyn std::error::Error>>;

// ---------------------------------------------------------------- helpers

fn put(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn skill_md(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: The {name} skill\n---\n{body}\n")
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// A git repository in a temp folder, reached by `file://` URL.
struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new(files: &[(&str, String)]) -> R<Self> {
        let dir = tempfile::tempdir()?;
        git(dir.path(), &["init", "-q", "-b", "main"])?;
        for (rel, text) in files {
            put(dir.path(), rel, text)?;
        }
        let repo = Self { dir };
        repo.commit("one")?;
        Ok(repo)
    }

    fn commit(&self, message: &str) -> R {
        git(self.dir.path(), &["add", "-A"])?;
        git(self.dir.path(), &["commit", "-q", "-m", message])?;
        Ok(())
    }

    fn change(&self, rel: &str, text: &str) -> R {
        put(self.dir.path(), rel, text)?;
        self.commit("change")
    }

    fn source(&self) -> R<Source> {
        Ok(Source::parse_git(
            &format!("file://{}", self.dir.path().display()),
            None,
            None,
        )?)
    }
}

fn local(dir: &Path) -> Source {
    Source::Local {
        path: dir.to_path_buf(),
    }
}

/// A library in a fresh data folder.
struct Lab {
    data: tempfile::TempDir,
    library: Library,
}

impl Lab {
    fn new() -> R<Self> {
        let data = tempfile::tempdir()?;
        let library = Library::open(data.path())?;
        Ok(Self { data, library })
    }

    /// Closes and opens the library again, as the app does on the next start.
    fn reopen(&mut self) -> R {
        self.library = Library::open(self.data.path())?;
        Ok(())
    }

    fn import_all(&mut self, preview: &Preview) -> R<Vec<String>> {
        let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        self.library.import(preview, &ids, &TagChoices::new())?;
        Ok(ids)
    }

    fn id(&self, kind: ArtifactKind, name: &str) -> R<String> {
        self.library
            .find(kind, name)?
            .ok_or_else(|| format!("{name} is not in the library").into())
    }

    /// Everything in the library, as `(kind, name)`.
    fn held(&self) -> R<Vec<(ArtifactKind, String)>> {
        let mut out: Vec<(ArtifactKind, String)> = self
            .library
            .audits()?
            .into_iter()
            .map(|a| (a.kind, a.name))
            .collect();
        out.sort();
        Ok(out)
    }

    fn profile(&self, name: &str, ids: &[&str]) -> R<i64> {
        let spec = ProfileSpec {
            items: ids.iter().map(|i| (*i).to_owned()).collect(),
            ..ProfileSpec::default()
        };
        Ok(self
            .library
            .create_profile(name, "", &spec, CategoryId::Software, None)?)
    }
}

fn whole_options() -> ScanOptions {
    ScanOptions {
        unpack_by_default: false,
        ..ScanOptions::default()
    }
}

fn unpack_options(folder: &str) -> ScanOptions {
    ScanOptions {
        unpack: vec![folder.to_owned()],
        ..ScanOptions::default()
    }
}

fn names_of(preview: &Preview, kind: ArtifactKind) -> Vec<String> {
    let mut names: Vec<String> = preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind == kind)
        .map(|c| c.artifact.name.clone())
        .collect();
    names.sort();
    names
}

fn plugin_candidate<'a>(
    preview: &'a Preview,
    name: &str,
) -> R<&'a harneo_core::library::Candidate> {
    preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Plugin && c.artifact.name == name)
        .ok_or_else(|| format!("plugin {name} is not in the preview").into())
}

fn read(root: &Path, rel: &str) -> R<String> {
    Ok(fs::read_to_string(root.join(rel))?)
}

fn settings_of(root: &Path) -> R<Value> {
    Ok(serde_json::from_str(&read(root, ".claude/settings.json")?)?)
}

/// Every file under `root`, relative and sorted.
fn files_under(root: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// A script standing in for `claude`: a log of its calls, and the two plugin listings empty.
fn fake_claude(dir: &Path) -> R<ClaudeConfig> {
    let d = dir.display();
    let path = dir.join("claude");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\ncase \"$*\" in\n  \"plugin marketplace list --json\") echo '[]' ;;\n  \"plugin list --json\") echo '[]' ;;\n  *) echo \"$*\" >> '{d}/calls.txt'; echo '{{\"outcome\":\"ok\",\"message\":\"done\"}}' ;;\nesac\n"
        ),
    )?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(ClaudeConfig {
        program: path.into(),
        timeout: std::time::Duration::from_secs(20),
        ..ClaudeConfig::default()
    })
}

// ---------------------------------------------------------------- fixtures

/// A skills-only pack shaped like `addyosmani/agent-skills`: a plugin and its marketplace at the
/// repo root, a folder per skill (one with a reference file), nothing that needs the plugin.
fn agent_skills_files() -> Vec<(&'static str, String)> {
    vec![
        (
            ".claude-plugin/plugin.json",
            r#"{"name":"agent-skills","description":"Engineering skills"}"#.to_owned(),
        ),
        (
            ".claude-plugin/marketplace.json",
            r#"{"name":"addy-agent-skills","owner":{"name":"addy"},"plugins":[{"name":"agent-skills","source":"./"}]}"#
                .to_owned(),
        ),
        (
            "skills/spec-driven-development/SKILL.md",
            skill_md("spec-driven-development", "Write the spec first."),
        ),
        (
            "skills/test-driven-development/SKILL.md",
            skill_md("test-driven-development", "Red, green, refactor."),
        ),
        (
            "skills/test-driven-development/references/patterns.md",
            "# Patterns\nArrange, act, assert.\n".to_owned(),
        ),
        (
            "skills/code-review/SKILL.md",
            skill_md("code-review", "Review in five axes."),
        ),
        ("README.md", "# agent-skills\n".to_owned()),
    ]
}

const SKILLS: [&str; 3] = [
    "code-review",
    "spec-driven-development",
    "test-driven-development",
];

// ---------------------------------------------------------------- 1. skills-only pack

#[test]
fn a_skills_only_pack_imports_as_ordinary_artifacts_with_audit_tags_and_bundles() -> R {
    let repo = Repo::new(&agent_skills_files())?;
    let mut lab = Lab::new()?;
    let source = repo.source()?;

    // No choice made: the preview offers the parts, not the plugin.
    let preview = lab.library.preview(&source)?;
    assert_eq!(names_of(&preview, ArtifactKind::Skill), SKILLS);
    assert!(names_of(&preview, ArtifactKind::Plugin).is_empty());
    for c in &preview.candidates {
        assert_eq!(c.artifact.metadata["fromPlugin"]["name"], "agent-skills");
        assert!(c.must_stay_whole.is_none() && c.must_stay_whole_text.is_none());
        assert!(!c.already_in_library);
    }
    let id = |name: &str| -> R<String> {
        preview
            .candidates
            .iter()
            .find(|c| c.artifact.name == name)
            .map(|c| c.id.clone())
            .ok_or_else(|| format!("{name} missing").into())
    };
    let (spec, tdd, review) = (
        id("spec-driven-development")?,
        id("test-driven-development")?,
        id("code-review")?,
    );

    // The library tells the window it holds nothing of the plugin yet.
    let holds = lab.library.plugin_holds(&preview)?;
    assert_eq!(holds.len(), 1);
    assert_eq!(holds[0].name, "agent-skills");
    assert!(!holds[0].whole && !holds[0].parts);

    // Tags, and a bundle (an accepted link), travel with the import like for any artifact.
    let mut tags = TagChoices::new();
    tags.insert(tdd.clone(), vec!["method".to_owned()]);
    let report = lab.library.import_with_links(
        &preview,
        &[spec.clone(), tdd.clone(), review.clone()],
        &tags,
        &[(
            spec.clone(),
            tdd.clone(),
            "tests follow the spec".to_owned(),
        )],
    )?;
    assert_eq!(report.imported.len(), 3);
    assert_eq!(
        lab.held()?,
        SKILLS.map(|n| (ArtifactKind::Skill, n.to_owned()))
    );

    // Audited as ordinary artifacts, with the reference file stored beside the skill.
    for audit in lab.library.audits()? {
        assert!(audit.audit.is_some(), "{} is audited", audit.name);
    }
    let detail = lab.library.detail(&tdd)?;
    assert_eq!(
        detail.artifact.metadata["fromPlugin"]["name"],
        "agent-skills"
    );
    assert!(detail.audit.is_some());
    assert_eq!(detail.files, ["SKILL.md", "references/patterns.md"]);
    assert_eq!(lab.library.artifact_tags(&tdd)?[0].name, "method");
    assert_eq!(lab.library.bundle_of(&spec)?, std::slice::from_ref(&tdd));
    assert_eq!(lab.library.links_of(&tdd)?.needed_by.len(), 1);

    // The list names the plugin each part came from; the library filter finds them by tag.
    let page = lab.library.list(&Query::default())?;
    assert_eq!(page.items.len(), 3);
    assert!(page
        .items
        .iter()
        .all(|i| i.from_plugin.as_deref() == Some("agent-skills")));

    // Previewing again shows them as in the library, and the plugin is now held as parts.
    let again = lab.library.preview(&source)?;
    assert!(again.candidates.iter().all(|c| c.already_in_library));
    assert!(lab.library.plugin_holds(&again)?[0].parts);
    Ok(())
}

// ---------------------------------------------------------------- 2. hooks / mods / LSP

/// A marketplace of three plugins that only work whole: one with hooks (and a skill), one mod
/// (a hooks module), one LSP server. The folder has a GitHub `origin`, as a clone would.
fn mods_marketplace(root: &Path) -> std::io::Result<()> {
    put(
        root,
        ".claude-plugin/marketplace.json",
        r#"{"name":"claude-mods","owner":{"name":"acme"},"plugins":[
            {"name":"radar","source":"./plugins/radar"},
            {"name":"bell","source":"./plugins/bell"},
            {"name":"lint-lsp","source":"./plugins/lint-lsp"}]}"#,
    )?;
    put(
        root,
        ".git/config",
        "[remote \"origin\"]\n\turl = https://github.com/acme/claude-mods.git\n",
    )?;
    for name in ["radar", "bell", "lint-lsp"] {
        put(
            root,
            &format!("plugins/{name}/.claude-plugin/plugin.json"),
            &format!(r#"{{"name":"{name}"}}"#),
        )?;
    }
    put(
        root,
        "plugins/radar/hooks/hooks.json",
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo radar"}]}]}}"#,
    )?;
    put(
        root,
        "plugins/radar/skills/radar/SKILL.md",
        &skill_md("radar", "Watch the sessions."),
    )?;
    put(
        root,
        "plugins/bell/hooks/hooks.json",
        r#"{"modules":["./register.tsx"]}"#,
    )?;
    put(
        root,
        "plugins/bell/hooks/register.tsx",
        "export default {};\n",
    )?;
    put(
        root,
        "plugins/lint-lsp/.lsp.json",
        r#"{"ts":{"command":"tsserver","extensionToLanguage":{".ts":"typescript"}}}"#,
    )
}

#[test]
fn hooks_mods_and_lsp_plugins_are_proposed_whole_and_scaffold_enables_them() -> R {
    let src = tempfile::tempdir()?;
    mods_marketplace(src.path())?;
    let mut lab = Lab::new()?;

    // Default import: all three stay whole, each with its reason in the core's words.
    let preview = lab.library.preview(&local(src.path()))?;
    assert_eq!(
        names_of(&preview, ArtifactKind::Plugin),
        ["bell", "lint-lsp", "radar"]
    );
    assert!(names_of(&preview, ArtifactKind::Skill).is_empty());
    let reason = |name: &str| -> R<(Option<WholeReason>, Option<String>)> {
        let c = plugin_candidate(&preview, name)?;
        Ok((c.must_stay_whole.clone(), c.must_stay_whole_text.clone()))
    };
    assert_eq!(
        reason("radar")?,
        (
            Some(WholeReason::WholeOnlyParts {
                parts: vec![PluginPart::Hooks]
            }),
            Some("Has hooks: they only work inside the plugin.".to_owned())
        )
    );
    assert_eq!(
        reason("bell")?,
        (
            Some(WholeReason::WholeOnlyParts {
                parts: vec![PluginPart::Mods]
            }),
            Some("Has mods: they only work inside the plugin.".to_owned())
        )
    );
    assert_eq!(
        reason("lint-lsp")?,
        (
            Some(WholeReason::WholeOnlyParts {
                parts: vec![PluginPart::LspServers]
            }),
            Some("Has LSP servers: they only work inside the plugin.".to_owned())
        )
    );
    lab.import_all(&preview)?;
    assert_eq!(
        lab.held()?,
        ["bell", "lint-lsp", "radar"].map(|n| (ArtifactKind::Plugin, n.to_owned()))
    );

    // The library's own unpack refuses each, with the same reason, and changes nothing.
    for name in ["radar", "bell", "lint-lsp"] {
        let id = lab.id(ArtifactKind::Plugin, name)?;
        let outcome = lab.library.unpack_in_library(&id)?;
        assert!(outcome.refused.is_some() && outcome.parts.is_empty());
        assert_eq!(outcome.reason, reason(name)?.1);
        assert!(lab.library.detail(&id).is_ok());
    }

    // Scaffold enables them in settings.json, adds their marketplace, copies no part.
    let ids: Vec<String> = ["radar", "bell", "lint-lsp"]
        .iter()
        .map(|n| lab.id(ArtifactKind::Plugin, n))
        .collect::<R<_>>()?;
    let profile = lab.profile("mods", &ids.iter().map(String::as_str).collect::<Vec<_>>())?;
    let project = tempfile::tempdir()?;
    let plan = lab.library.plan_scaffold(project.path(), profile, None)?;
    assert_eq!(plan.kind, PlanKind::Scaffold);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    let paths: Vec<&str> = plan.writes.iter().map(|w| w.path.as_str()).collect();
    assert!(paths.contains(&".claude/settings.json"));
    assert!(
        !paths
            .iter()
            .any(|p| p.contains("skills") || p.contains("hooks")),
        "{paths:?}"
    );
    lab.library.apply(&plan)?;
    let settings = settings_of(project.path())?;
    for key in [
        "radar@claude-mods",
        "bell@claude-mods",
        "lint-lsp@claude-mods",
    ] {
        assert_eq!(settings["enabledPlugins"][key], true, "{key}");
    }
    assert_eq!(
        settings["extraKnownMarketplaces"]["claude-mods"]["source"],
        json!({"source": "github", "repo": "acme/claude-mods"})
    );
    assert!(!project.path().join(".claude/skills").exists());

    // The install step plans the marketplace add and one install per plugin, through `claude`.
    let targets = plugin_targets(project.path())?;
    assert_eq!(targets.len(), 3);
    assert!(targets
        .iter()
        .all(|t| t.source.as_deref() == Some("acme/claude-mods")));
    let bin = tempfile::tempdir()?;
    let config = fake_claude(bin.path())?;
    let results = install_plugins(&config, project.path(), |_| {})?;
    assert_eq!(results.len(), 3);
    assert!(results
        .iter()
        .all(|r| r.status == PluginInstallStatus::Installed));
    let calls = read(bin.path(), "calls.txt")?;
    assert!(
        calls.contains("plugin marketplace add acme/claude-mods --json"),
        "{calls}"
    );
    for key in [
        "radar@claude-mods",
        "bell@claude-mods",
        "lint-lsp@claude-mods",
    ] {
        assert!(
            calls.contains(&format!("plugin install {key} --scope project --json")),
            "{calls}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------- 3. mixed plugin

fn mixed_plugin(root: &Path) -> std::io::Result<()> {
    put(root, ".claude-plugin/plugin.json", r#"{"name":"mixed"}"#)?;
    for name in ["alpha", "beta"] {
        put(
            root,
            &format!("skills/{name}/SKILL.md"),
            &skill_md(name, "Body"),
        )?;
    }
    put(
        root,
        "hooks/hooks.json",
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    )?;
    put(root, "monitors/monitors.json", "[]")
}

#[test]
fn a_mixed_plugin_is_whole_by_default_and_cannot_be_unpacked() -> R {
    let src = tempfile::tempdir()?;
    mixed_plugin(src.path())?;
    let lab = Lab::new()?;

    let preview = lab.library.preview(&local(src.path()))?;
    let plugin = plugin_candidate(&preview, "mixed")?;
    assert_eq!(
        plugin.must_stay_whole,
        Some(WholeReason::WholeOnlyParts {
            parts: vec![PluginPart::Hooks, PluginPart::Monitors]
        })
    );
    assert_eq!(
        plugin.must_stay_whole_text.as_deref(),
        Some("Has hooks and monitors: they only work inside the plugin.")
    );
    assert!(names_of(&preview, ArtifactKind::Skill).is_empty());

    // Asking to unpack it changes nothing (ADR-006): the mixed plugin stays whole, with its reason.
    let asked = lab
        .library
        .preview_with(&local(src.path()), &unpack_options(""))?;
    assert_eq!(names_of(&asked, ArtifactKind::Plugin), ["mixed"]);
    assert!(names_of(&asked, ArtifactKind::Skill).is_empty());
    assert_eq!(
        plugin_candidate(&asked, "mixed")?.must_stay_whole,
        plugin.must_stay_whole
    );

    // The same plugin held whole in another library cannot be unpacked in place either.
    let mut other = Lab::new()?;
    other.import_all(&preview)?;
    let id = other.id(ArtifactKind::Plugin, "mixed")?;
    let outcome = other.library.unpack_in_library(&id)?;
    assert_eq!(outcome.refused, plugin.must_stay_whole);
    assert!(other.library.detail(&id).is_ok());
    Ok(())
}

// ---------------------------------------------------------------- 4. root tie, remote only

#[test]
fn a_part_tied_to_the_plugin_root_and_a_remote_only_entry_stay_whole() -> R {
    let src = tempfile::tempdir()?;
    put(
        src.path(),
        ".claude-plugin/marketplace.json",
        r#"{"name":"market","owner":{"name":"acme"},"plugins":[
            {"name":"tied","source":"./plugins/tied"},
            {"name":"far","source":{"source":"github","repo":"other/far"}}]}"#,
    )?;
    put(
        src.path(),
        "plugins/tied/.claude-plugin/plugin.json",
        r#"{"name":"tied"}"#,
    )?;
    put(
        src.path(),
        "plugins/tied/skills/runner/SKILL.md",
        "---\nname: runner\ndescription: Runs\n---\nRun ${CLAUDE_PLUGIN_ROOT}/shared/tool.sh\n",
    )?;
    put(src.path(), "plugins/tied/shared/tool.sh", "echo hi\n")?;
    let mut lab = Lab::new()?;

    let preview = lab.library.preview(&local(src.path()))?;
    assert_eq!(names_of(&preview, ArtifactKind::Plugin), ["far", "tied"]);
    assert!(names_of(&preview, ArtifactKind::Skill).is_empty());
    let tied = plugin_candidate(&preview, "tied")?;
    assert_eq!(
        tied.must_stay_whole,
        Some(WholeReason::PluginVariable {
            variable: "${CLAUDE_PLUGIN_ROOT}".to_owned(),
            parts: vec![PluginPart::Skills]
        })
    );
    let far = plugin_candidate(&preview, "far")?;
    assert_eq!(far.must_stay_whole, Some(WholeReason::RemoteOnly));
    assert_eq!(
        far.must_stay_whole_text.as_deref(),
        Some(
            "Its files live in another repo that is never fetched, so it can only be taken whole."
        )
    );

    // Even asking to unpack them leaves them whole, each with a warning.
    let asked = lab
        .library
        .preview_with(&local(src.path()), &unpack_options("plugins/tied"))?;
    assert!(names_of(&asked, ArtifactKind::Plugin).contains(&"far".to_owned()));
    assert!(names_of(&asked, ArtifactKind::Skill).is_empty());
    assert!(plugin_candidate(&asked, "tied")?.must_stay_whole.is_some());
    let far_path = far.artifact.path.clone();
    let remote = ScanOptions {
        unpack: vec![far_path],
        ..ScanOptions::default()
    };
    let asked = lab.library.preview_with(&local(src.path()), &remote)?;
    let far_asked = plugin_candidate(&asked, "far")?;
    assert!(far_asked
        .artifact
        .diagnostics
        .iter()
        .any(|d| d.code == "plugin.unpack-remote-only"));

    // Held whole, the library refuses to unpack both, with their reasons.
    lab.import_all(&preview)?;
    for (name, reason) in [
        ("tied", tied.must_stay_whole.clone()),
        ("far", far.must_stay_whole.clone()),
    ] {
        let id = lab.id(ArtifactKind::Plugin, name)?;
        let outcome = lab.library.unpack_in_library(&id)?;
        assert_eq!(outcome.refused, reason, "{name}");
        assert!(outcome.parts.is_empty());
        assert!(lab.library.detail(&id).is_ok());
    }
    Ok(())
}

// ---------------------------------------------------------------- 5. updates

fn outcome_of(report: &SourceResult) -> (u32, u32, u32) {
    match report {
        SourceResult::Changed {
            outdated,
            removed,
            new_upstream,
            ..
        } => (*outdated, *removed, *new_upstream),
        _ => (0, 0, 0),
    }
}

fn stored(lab: &Lab, id: &str, rel: &str) -> R<String> {
    Ok(fs::read_to_string(
        lab.data.path().join("library/artifacts").join(id).join(rel),
    )?)
}

#[test]
fn updates_replay_the_unpacked_choice_update_the_parts_and_protect_local_edits() -> R {
    let repo = Repo::new(&agent_skills_files())?;
    let mut lab = Lab::new()?;
    let preview = lab.library.preview(&repo.source()?)?;
    lab.import_all(&preview)?;
    let spec = lab.id(ArtifactKind::Skill, "spec-driven-development")?;
    let tdd = lab.id(ArtifactKind::Skill, "test-driven-development")?;
    let review = lab.id(ArtifactKind::Skill, "code-review")?;

    // A reopened library checks the source as parts: nothing changed, nothing to report.
    lab.reopen()?;
    let report = lab.library.check_updates(&CheckScope::All)?;
    assert_eq!(report.sources.len(), 1);
    assert_eq!(report.sources[0].result, SourceResult::Current);

    // Upstream changes two parts, adds a third skill. The parts are what is outdated.
    repo.change(
        "skills/spec-driven-development/SKILL.md",
        &skill_md("spec-driven-development", "Write the spec first, then ask."),
    )?;
    repo.change(
        "skills/code-review/SKILL.md",
        &skill_md("code-review", "Review in six axes."),
    )?;
    repo.change(
        "skills/shipping/SKILL.md",
        &skill_md("shipping", "Ship it."),
    )?;
    // The library copy of `code-review` was edited locally.
    let edited = stored(&lab, &review, "SKILL.md")? + "\nmy own note\n";
    fs::write(
        lab.data
            .path()
            .join("library/artifacts")
            .join(&review)
            .join("SKILL.md"),
        &edited,
    )?;
    let report = lab.library.check_updates(&CheckScope::All)?;
    assert_eq!(outcome_of(&report.sources[0].result), (2, 0, 1));
    let status = lab.library.update_status(&spec)?.ok_or("spec outdated")?;
    assert_eq!(status.state, UpstreamState::Outdated);
    assert!(!status.local_edits);
    let status = lab
        .library
        .update_status(&review)?
        .ok_or("review outdated")?;
    assert!(status.local_edits);
    assert!(lab.library.update_status(&tdd)?.is_none());
    assert_eq!(
        lab.held()?.len(),
        3,
        "the new upstream skill is not imported on its own"
    );
    assert!(lab
        .library
        .list(&Query::default())?
        .items
        .iter()
        .all(|i| i.kind != ArtifactKind::Plugin));

    // Update all takes the clean part and leaves the edited one alone.
    let reports = lab.library.update_all()?;
    let by_id = |id: &str| reports.iter().find(|r| r.id == id).map(|r| r.outcome);
    assert_eq!(by_id(&spec), Some(UpdateOutcome::Updated));
    assert_eq!(by_id(&review), Some(UpdateOutcome::SkippedLocalEdits));
    assert!(stored(&lab, &spec, "SKILL.md")?.contains("then ask"));
    assert_eq!(stored(&lab, &review, "SKILL.md")?, edited);

    // Keeping mine clears the flag; taking upstream backs the edit up first.
    let kept = lab.library.update(&review, UpdateChoice::KeepMine)?;
    assert_eq!(kept.outcome, UpdateOutcome::Kept);
    lab.library.check_updates(&CheckScope::All)?;
    let taken = lab.library.update(&review, UpdateChoice::TakeUpstream)?;
    assert_eq!(taken.outcome, UpdateOutcome::Updated);
    assert!(taken.backup.is_some(), "the edited copy was backed up");
    assert!(stored(&lab, &review, "SKILL.md")?.contains("six axes"));
    assert!(!stored(&lab, &review, "SKILL.md")?.contains("my own note"));
    // Still parts, still no plugin.
    assert_eq!(
        lab.held()?,
        SKILLS.map(|n| (ArtifactKind::Skill, n.to_owned()))
    );
    Ok(())
}

#[test]
fn a_source_imported_whole_before_the_default_changed_stays_whole_through_updates() -> R {
    let repo = Repo::new(&agent_skills_files())?;
    let mut lab = Lab::new()?;
    // What a build before ADR-004 did: the plugin, taken whole.
    let before = lab
        .library
        .preview_with(&repo.source()?, &whole_options())?;
    assert_eq!(names_of(&before, ArtifactKind::Plugin), ["agent-skills"]);
    lab.import_all(&before)?;
    let plugin = lab.id(ArtifactKind::Plugin, "agent-skills")?;

    repo.change(
        "skills/code-review/SKILL.md",
        &skill_md("code-review", "Review in six axes."),
    )?;
    lab.reopen()?;
    let report = lab.library.check_updates(&CheckScope::All)?;
    assert_eq!(outcome_of(&report.sources[0].result).0, 1);
    let status = lab
        .library
        .update_status(&plugin)?
        .ok_or("plugin outdated")?;
    assert_eq!(status.state, UpstreamState::Outdated);
    let done = lab.library.update(&plugin, UpdateChoice::TakeUpstream)?;
    assert_eq!(done.outcome, UpdateOutcome::Updated);
    assert!(stored(&lab, &plugin, "skills/code-review/SKILL.md")?.contains("six axes"));
    // Whole before, whole after: one Plugin, no parts appeared.
    assert_eq!(
        lab.held()?,
        [(ArtifactKind::Plugin, "agent-skills".to_owned())]
    );
    // A plain preview of the same source keeps offering it whole, never converting it.
    let again = lab.library.preview(&repo.source()?)?;
    assert_eq!(names_of(&again, ArtifactKind::Plugin), ["agent-skills"]);
    assert!(lab.library.plugin_holds(&again)?[0].whole);
    Ok(())
}

// ---------------------------------------------------------------- 6. one-click unpack

/// A marketplace with one skills-only plugin, `kit`, with a GitHub origin like a clone.
fn kit_marketplace(root: &Path) -> std::io::Result<()> {
    put(
        root,
        ".claude-plugin/marketplace.json",
        r#"{"name":"kit-market","owner":{"name":"acme"},"plugins":[{"name":"kit","source":"./plugins/kit"}]}"#,
    )?;
    put(
        root,
        ".git/config",
        "[remote \"origin\"]\n\turl = https://github.com/acme/kit-market.git\n",
    )?;
    put(
        root,
        "plugins/kit/.claude-plugin/plugin.json",
        r#"{"name":"kit"}"#,
    )?;
    for name in ["lint", "fmt"] {
        put(
            root,
            &format!("plugins/kit/skills/{name}/SKILL.md"),
            &skill_md(name, "Body"),
        )?;
    }
    put(
        root,
        "plugins/kit/agents/fixer.md",
        "---\nname: fixer\ndescription: Fixes\n---\nFix it.\n",
    )
}

#[test]
fn one_click_unpack_of_a_plugin_held_by_two_profiles_keeps_projects_and_backups_working() -> R {
    let src = tempfile::tempdir()?;
    kit_marketplace(src.path())?;
    let mut lab = Lab::new()?;
    let before = lab
        .library
        .preview_with(&local(src.path()), &whole_options())?;
    assert_eq!(names_of(&before, ArtifactKind::Plugin), ["kit"]);
    lab.import_all(&before)?;
    let plugin = lab.id(ArtifactKind::Plugin, "kit")?;
    let tag = lab.library.create_tag("mine", TagColor::Red)?;
    lab.library.tag_artifact(&plugin, tag.id)?;
    let held = lab.library.create_manual_collection("held")?;
    lab.library.add_to_collection(held.id, &plugin)?;
    let heavy = lab.profile("Heavy", &[&plugin])?;
    let light = lab.profile("Light", &[&plugin])?;

    // A project scaffolded from the old version: the plugin is enabled in its settings.
    let project = tempfile::tempdir()?;
    let applied = lab
        .library
        .apply(&lab.library.plan_scaffold(project.path(), heavy, None)?)?;
    let settings = settings_of(project.path())?;
    assert_eq!(settings["enabledPlugins"]["kit@kit-market"], true);
    assert!(!project.path().join(".claude/skills").exists());

    // The plan names what would change; the click does it.
    let plan = lab.library.unpack_plan(&plugin)?;
    let mut planned: Vec<&str> = plan.parts.iter().map(|p| p.name.as_str()).collect();
    planned.sort_unstable();
    assert_eq!(planned, ["fixer", "fmt", "lint"]);
    let changes: Vec<(&str, u32, u32)> = plan
        .profiles
        .iter()
        .map(|p| (p.name.as_str(), p.from, p.to))
        .collect();
    assert_eq!(changes, [("Heavy", 1, 2), ("Light", 1, 2)]);
    assert!(
        lab.library.detail(&plugin).is_ok(),
        "a plan changes nothing"
    );
    let done: UnpackOutcome = lab.library.unpack_in_library(&plugin)?;
    assert_eq!(done.parts, plan.parts);
    assert!(done.refused.is_none());

    // New versions hold the parts; the old versions still hold the plugin.
    let part_ids: Vec<String> = done.parts.iter().map(|p| p.id.clone()).collect();
    for profile in [heavy, light] {
        let now = lab.library.profile(profile, None)?;
        assert_eq!(now.version, 2);
        assert_eq!(now.spec.items, part_ids);
        assert_eq!(
            lab.library.profile(profile, Some(1))?.spec.items,
            std::slice::from_ref(&plugin)
        );
    }
    // Tags and manual collections went to every part; the plugin is gone.
    for id in &part_ids {
        let tags = lab.library.artifact_tags(id)?;
        assert_eq!((tags.len(), tags[0].id), (1, tag.id));
        assert_eq!(lab.library.artifact_collections(id)?, vec![held.id]);
        assert!(lab.library.detail(id)?.audit.is_some());
    }
    assert!(matches!(
        lab.library.detail(&plugin),
        Err(Error::NotInLibrary { .. })
    ));
    assert_eq!(lab.held()?.len(), 3);

    // The project still opens: the profile moved on, and says so.
    let drift = lab.library.check_project(applied.project_id)?;
    assert_eq!(drift.profile_newer, Some(2));
    assert!(lab
        .library
        .check_all()?
        .iter()
        .any(|p| p.project_id == applied.project_id));

    // Switching it to the new version writes the parts, stops enabling the plugin, and backs up
    // what it replaces.
    let switch = lab.library.plan_switch(applied.project_id, heavy, None)?;
    assert_eq!(switch.kind, PlanKind::Switch);
    assert_eq!(switch.profile_version, 2);
    let action = |path: &str| {
        switch
            .writes
            .iter()
            .find(|w| w.path == path)
            .map(|w| w.action)
    };
    assert_eq!(
        action(".claude/skills/lint/SKILL.md"),
        Some(WriteAction::New)
    );
    assert_eq!(
        action(".claude/skills/fmt/SKILL.md"),
        Some(WriteAction::New)
    );
    assert_eq!(action(".claude/agents/fixer.md"), Some(WriteAction::New));
    assert_eq!(action(".claude/settings.json"), Some(WriteAction::Delete));
    let switched = lab.library.apply(&switch)?;
    assert!(switched.backup_id.is_some());
    assert!(project
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .is_file());
    assert!(!project.path().join(".claude/settings.json").exists());
    let after = lab.library.check_project(applied.project_id)?;
    assert_eq!(after.status, DriftStatus::InSync, "{:?}", after.files);
    assert_eq!(after.profile_newer, None);
    assert_eq!(lab.library.project(applied.project_id)?.profile_version, 2);

    // Drift on a part is shown and re-applies cleanly.
    fs::write(
        project.path().join(".claude/skills/lint/SKILL.md"),
        "mine\n",
    )?;
    let drifted = lab.library.check_project(applied.project_id)?;
    assert_eq!(drifted.status, DriftStatus::Drift);
    assert!(matches!(
        drifted.files[0].change,
        DriftChange::Edited { .. }
    ));
    let reapply = lab.library.plan_reapply(applied.project_id, None)?;
    lab.library.apply(&reapply)?;
    assert_eq!(
        lab.library.check_project(applied.project_id)?.status,
        DriftStatus::InSync
    );
    assert!(read(project.path(), ".claude/skills/lint/SKILL.md")?.contains("name: lint"));

    // The backup of the switch restores the settings that enabled the plugin.
    let backup = switched.backup_id.ok_or("backup")?;
    lab.library.restore_backup(backup)?;
    assert_eq!(
        settings_of(project.path())?["enabledPlugins"]["kit@kit-market"],
        true
    );
    Ok(())
}

// ---------------------------------------------------------------- 7. install

#[test]
fn scaffold_and_refit_write_unpacked_parts_under_claude_and_never_enable_the_plugin() -> R {
    let src = tempfile::tempdir()?;
    for (rel, text) in agent_skills_files() {
        put(src.path(), rel, &text)?;
    }
    put(
        src.path(),
        ".git/config",
        "[remote \"origin\"]\n\turl = https://github.com/addyosmani/agent-skills.git\n",
    )?;
    let mut lab = Lab::new()?;
    let preview = lab.library.preview(&local(src.path()))?;
    lab.import_all(&preview)?;
    let ids: Vec<String> = SKILLS
        .iter()
        .map(|n| lab.id(ArtifactKind::Skill, n))
        .collect::<R<_>>()?;
    let profile = lab.profile(
        "skills",
        &ids.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;

    // Scaffold: the parts as ordinary files, the plugin neither copied nor enabled.
    let fresh = tempfile::tempdir()?;
    let plan = lab.library.plan_scaffold(fresh.path(), profile, None)?;
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    lab.library.apply(&plan)?;
    let files = files_under(fresh.path());
    assert!(
        files.contains(&".claude/skills/code-review/SKILL.md".to_owned()),
        "{files:?}"
    );
    assert!(
        files.contains(&".claude/skills/test-driven-development/references/patterns.md".to_owned())
    );
    assert!(files.contains(&"HARNESS.md".to_owned()));
    assert!(
        !files
            .iter()
            .any(|f| f.contains("settings.json") || f.contains(".claude-plugin")),
        "{files:?}"
    );
    assert!(!read(fresh.path(), "HARNESS.md")?.contains("## Plugins"));
    assert!(plugin_targets(fresh.path())?.is_empty());
    let bin = tempfile::tempdir()?;
    let config = fake_claude(bin.path())?;
    assert!(install_plugins(&config, fresh.path(), |_| {})?.is_empty());
    assert!(
        !bin.path().join("calls.txt").exists(),
        "claude was never asked to install"
    );
    assert_eq!(
        lab.library
            .check_project(lab.library.projects()?[0].id)?
            .status,
        DriftStatus::InSync
    );

    // Refit over a project that has its own harness: its settings stay untouched (no plugin
    // enabled), its own copy of a skill is replaced after a backup, and restore brings it back.
    let own = tempfile::tempdir()?;
    put(
        own.path(),
        ".claude/settings.json",
        "{\"permissions\":{\"allow\":[\"Read\"]}}\n",
    )?;
    put(
        own.path(),
        ".claude/skills/code-review/SKILL.md",
        &skill_md("code-review", "Mine."),
    )?;
    let settings_before = read(own.path(), ".claude/settings.json")?;
    let comparison = lab.library.compare(own.path(), profile, None)?;
    assert!(comparison
        .rows
        .iter()
        .filter(|r| r.item.contains("settings"))
        .all(|r| r.kind == harneo_core::install::RowKind::ProjectOnly));
    let plan = lab.library.plan_refit(&comparison, &Default::default())?;
    assert_eq!(plan.kind, PlanKind::Refit);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    let report = lab.library.apply(&plan)?;
    assert_eq!(read(own.path(), ".claude/settings.json")?, settings_before);
    assert!(!read(own.path(), ".claude/settings.json")?.contains("enabledPlugins"));
    assert!(own
        .path()
        .join(".claude/skills/spec-driven-development/SKILL.md")
        .is_file());
    assert!(own
        .path()
        .join(".claude/skills/test-driven-development/references/patterns.md")
        .is_file());
    assert!(
        read(own.path(), ".claude/skills/code-review/SKILL.md")?.contains("Review in five axes.")
    );
    assert!(plugin_targets(own.path())?.is_empty());
    let backup = report.backup_id.ok_or("the replaced skill was backed up")?;
    lab.library.restore_backup(backup)?;
    assert!(read(own.path(), ".claude/skills/code-review/SKILL.md")?.contains("Mine."));
    Ok(())
}

#[test]
fn a_whole_plugin_scaffolds_and_refits_as_before_enabled_and_never_copied() -> R {
    let src = tempfile::tempdir()?;
    kit_marketplace(src.path())?;
    let mut lab = Lab::new()?;
    let preview = lab
        .library
        .preview_with(&local(src.path()), &whole_options())?;
    lab.import_all(&preview)?;
    let plugin = lab.id(ArtifactKind::Plugin, "kit")?;
    let profile = lab.profile("kit", &[&plugin])?;

    let fresh = tempfile::tempdir()?;
    let plan = lab.library.plan_scaffold(fresh.path(), profile, None)?;
    lab.library.apply(&plan)?;
    let files = files_under(fresh.path());
    assert_eq!(files, [".claude/settings.json", "HARNESS.md"]);
    assert_eq!(
        settings_of(fresh.path())?["enabledPlugins"]["kit@kit-market"],
        true
    );
    assert!(read(fresh.path(), "HARNESS.md")?.contains("Plugins are not copied"));

    // Refit over a project with its own settings: the permissions stay, the plugin is enabled.
    let own = tempfile::tempdir()?;
    put(
        own.path(),
        ".claude/settings.json",
        "{\"permissions\":{\"allow\":[\"Read\"]}}\n",
    )?;
    let plan = lab
        .library
        .refit_plan(own.path(), profile, None, &Default::default())?;
    lab.library.apply(&plan)?;
    let settings = settings_of(own.path())?;
    assert_eq!(settings["enabledPlugins"]["kit@kit-market"], true);
    assert!(!files_under(own.path())
        .iter()
        .any(|f| f.contains("skills") || f.contains("agents")));
    Ok(())
}

// ---------------------------------------------------------------- 8. usage

fn today() -> R<String> {
    let out = Command::new("date").args(["-u", "+%Y-%m-%d"]).output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

fn skill_use(session: &str, day: &str, skill: &str) -> String {
    json!({
        "type": "assistant", "sessionId": session, "timestamp": format!("{day}T10:00:00.000Z"),
        "cwd": "/work/app",
        "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t", "name": "Skill", "input": {"skill": skill}}]}
    })
    .to_string()
}

fn agent_use(session: &str, day: &str, agent: &str) -> String {
    json!({
        "type": "assistant", "sessionId": session, "timestamp": format!("{day}T10:00:00.000Z"),
        "cwd": "/work/app",
        "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t", "name": "Task", "input": {"subagent_type": agent}}]}
    })
    .to_string()
}

fn write_log(root: &Path, name: &str, lines: &[String]) -> R {
    put(
        root,
        &format!("-work-app/{name}.jsonl"),
        &(lines.join("\n") + "\n"),
    )?;
    Ok(())
}

fn sessions(lab: &Lab, kind: ArtifactKind, name: &str) -> R<(u32, Status)> {
    let page = lab.library.list(&Query::default())?;
    let item = page
        .items
        .iter()
        .find(|i| i.kind == kind && i.name == name)
        .ok_or_else(|| format!("{name} is not listed"))?;
    Ok((item.usage.as_ref().map_or(0, |u| u.sessions), item.status))
}

#[test]
fn sessions_are_counted_for_the_plugin_before_and_for_the_part_after_it_is_unpacked() -> R {
    let src = tempfile::tempdir()?;
    kit_marketplace(src.path())?;
    let mut lab = Lab::new()?;
    let before = lab
        .library
        .preview_with(&local(src.path()), &whole_options())?;
    lab.import_all(&before)?;
    let plugin = lab.id(ArtifactKind::Plugin, "kit")?;
    let day = today()?;
    let logs = tempfile::tempdir()?;
    let index = UsageIndex::open(lab.data.path())?;

    // Before: the plugin's skills are called `kit:lint`; the plugin is what gets used.
    write_log(
        logs.path(),
        "one",
        &[
            skill_use("s1", &day, "kit:lint"),
            skill_use("s2", &day, "kit:fmt"),
            skill_use("s2", &day, "kit:lint"),
        ],
    )?;
    index.index(logs.path(), |_| {})?;
    assert_eq!(
        sessions(&lab, ArtifactKind::Plugin, "kit")?,
        (2, Status::Current)
    );
    assert_eq!(index.usage_of(ArtifactKind::Plugin, "kit")?.sessions_8w, 2);
    assert_eq!(
        index.usage_of(ArtifactKind::Skill, "kit:lint")?.sessions_8w,
        2
    );

    // The click: the plugin is replaced by its parts, which start unused.
    lab.library.unpack_in_library(&plugin)?;
    assert_eq!(
        sessions(&lab, ArtifactKind::Skill, "lint")?,
        (0, Status::NotUsedYet)
    );
    assert_eq!(
        sessions(&lab, ArtifactKind::Agent, "fixer")?,
        (0, Status::NotUsedYet)
    );

    // After: the parts live in `.claude/`, called by their own names. Each use counts for the
    // part used, and the namespaced calls do not leak onto a part of the same name.
    write_log(
        logs.path(),
        "two",
        &[
            skill_use("s3", &day, "lint"),
            skill_use("s4", &day, "lint"),
            skill_use("s4", &day, "fmt"),
            agent_use("s5", &day, "fixer"),
        ],
    )?;
    index.index(logs.path(), |_| {})?;
    assert_eq!(
        sessions(&lab, ArtifactKind::Skill, "lint")?,
        (2, Status::Current)
    );
    assert_eq!(
        sessions(&lab, ArtifactKind::Skill, "fmt")?,
        (1, Status::Current)
    );
    assert_eq!(
        sessions(&lab, ArtifactKind::Agent, "fixer")?,
        (1, Status::Current)
    );
    // The old plugin-namespaced sessions stay in the index, for whoever still runs the plugin.
    assert_eq!(
        index.usage_of(ArtifactKind::Skill, "kit:lint")?.sessions_8w,
        2
    );
    assert_eq!(index.usage_of(ArtifactKind::Plugin, "kit")?.sessions_8w, 2);
    Ok(())
}

// ---------------------------------------------------------------- 9. Harnmap, Harnfit, export

/// A `claude` that answers `answers` in turn (the last repeats), as the structured output.
fn scripted_claude(dir: &Path, answers: &[Value]) -> R<ClaudeConfig> {
    let d = dir.display();
    for (n, a) in answers.iter().enumerate() {
        let body = json!({"is_error": false, "structured_output": a}).to_string();
        fs::write(dir.join(format!("ans-{n}")), &body)?;
        if n + 1 == answers.len() {
            fs::write(dir.join("ans-last"), &body)?;
        }
    }
    let path = dir.join("claude");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nn=$(ls '{d}' | grep -c '^in-')\ncat > \"{d}/in-$n.txt\"\nf=\"{d}/ans-$n\"\n[ -f \"$f\" ] || f=\"{d}/ans-last\"\ncat \"$f\"\n"
        ),
    )?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(ClaudeConfig {
        program: path.into(),
        timeout: std::time::Duration::from_secs(20),
        ..ClaudeConfig::default()
    })
}

fn stage_names() -> Vec<Value> {
    ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"]
        .iter()
        .map(|n| json!({"name": n}))
        .collect()
}

fn placed(short: &str, zone: &str) -> Value {
    json!({"artifact": short, "zone": zone, "note": "n", "why": "w", "role": "r", "when": "t", "hands_off": "h"})
}

fn map_answer(shorts: &[&str]) -> Value {
    json!({
        "stages": stage_names(),
        "loops": [],
        "placements": shorts.iter().enumerate().map(|(i, s)| placed(s, if i == 0 { "stage:Build" } else { "floor" })).collect::<Vec<_>>(),
    })
}

fn fits_answer() -> Value {
    json!({"verdict": "fits", "zone": "toolbelt", "why": "Fine.", "conflicts_with": [], "quotes": []})
}

#[test]
fn unpacked_parts_are_placed_one_by_one_a_whole_plugin_is_one_node_and_export_round_trips() -> R {
    // One pack unpacked (three skills), one plugin held whole, one loose candidate skill.
    let pack = tempfile::tempdir()?;
    for (rel, text) in agent_skills_files() {
        put(pack.path(), rel, &text)?;
    }
    let kit = tempfile::tempdir()?;
    kit_marketplace(kit.path())?;
    let extra = tempfile::tempdir()?;
    put(
        extra.path(),
        "skills/extra/SKILL.md",
        &skill_md("extra", "Extra."),
    )?;
    let mut lab = Lab::new()?;
    let preview = lab.library.preview(&local(pack.path()))?;
    lab.import_all(&preview)?;
    let whole = lab
        .library
        .preview_with(&local(kit.path()), &whole_options())?;
    lab.import_all(&whole)?;
    let loose = lab.library.preview(&local(extra.path()))?;
    lab.import_all(&loose)?;
    let part_ids: Vec<String> = SKILLS
        .iter()
        .map(|n| lab.id(ArtifactKind::Skill, n))
        .collect::<R<_>>()?;
    let plugin = lab.id(ArtifactKind::Plugin, "kit")?;
    let extra_id = lab.id(ArtifactKind::Skill, "extra")?;
    let parts = lab.profile(
        "Parts",
        &part_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let one = lab.profile("One", &[&plugin])?;

    // Before any drawing: every part waits on its own, each with its own name and kind.
    let view = lab.library.harnmap(parts)?;
    assert!(!view.drawn);
    let mut waiting = view.unplaced.clone();
    waiting.sort();
    let mut wanted = part_ids.clone();
    wanted.sort();
    assert_eq!(waiting, wanted);
    for id in &part_ids {
        assert_eq!(view.artifacts[id].kind, ArtifactKind::Skill);
    }

    // Drawn: each part is a placement of its own.
    let bin = tempfile::tempdir()?;
    lab.library.use_claude(scripted_claude(
        bin.path(),
        &[map_answer(&["a1", "a2", "a3"])],
    )?);
    lab.library.harnmap_draw(parts, "skills first")?;
    let view = lab.library.harnmap(parts)?;
    assert!(view.drawn && view.unplaced.is_empty() && view.stale.is_none());
    let mut placed_ids: Vec<String> = view
        .doc
        .placements
        .iter()
        .map(|p| p.artifact.clone())
        .collect();
    placed_ids.sort();
    assert_eq!(placed_ids, wanted);
    assert!(view.doc.placements.iter().all(|p| p.by == PlacedBy::Claude));

    // A whole plugin is one node: one item, one placement.
    clear_calls(bin.path())?;
    lab.library
        .use_claude(scripted_claude(bin.path(), &[map_answer(&["a1"])])?);
    lab.library.harnmap_draw(one, "")?;
    let view = lab.library.harnmap(one)?;
    assert_eq!(view.artifacts.len(), 1);
    assert_eq!(view.artifacts[&plugin].kind, ArtifactKind::Plugin);
    assert_eq!(view.doc.placements.len(), 1);
    assert_eq!(view.doc.placements[0].artifact, plugin);

    // Harnfit fits a part (from the library or a preview); a plugin is refused either way.
    clear_calls(bin.path())?;
    lab.library
        .use_claude(scripted_claude(bin.path(), &[fits_answer()])?);
    let fit = lab
        .library
        .fit(parts, &Candidate::Library(extra_id.clone()))?;
    assert_eq!(fit.verdict.candidate.kind, ArtifactKind::Skill);
    let refused = lab.library.fit(parts, &Candidate::Library(plugin.clone()));
    assert!(
        matches!(refused, Err(Error::HarnfitPluginUnsupported)),
        "{refused:?}"
    );
    let spare = tempfile::tempdir()?;
    kit_marketplace(spare.path())?;
    let remote_whole = lab
        .library
        .preview_with(&local(spare.path()), &whole_options())?;
    let candidate = remote_whole.candidates[0].id.clone();
    let refused = lab.library.fit(
        parts,
        &Candidate::Preview {
            preview: Box::new(remote_whole),
            candidate,
        },
    );
    assert!(
        matches!(refused, Err(Error::HarnfitPluginUnsupported)),
        "{refused:?}"
    );
    assert_eq!(lab.library.fits(parts)?.len(), 1);

    // Export (v2) with the map and the fit, import into a fresh library.
    let bytes = lab
        .library
        .export_profile(parts, None, ExportOptions::default())?;
    let json: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(json["formatVersion"], 2);
    assert_eq!(json["artifacts"].as_array().map(Vec::len), Some(3));
    assert!(json["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|a| a["artifact"]["kind"] == "skill"
            && a["artifact"]["metadata"]["fromPlugin"]["name"] == "agent-skills"));
    let mut fresh = Lab::new()?;
    let preview = fresh.library.import_profile_file(&bytes)?;
    assert_eq!(preview.artifacts.len(), 3);
    assert!(preview
        .artifacts
        .iter()
        .all(|a| !a.in_library && a.kind == ArtifactKind::Skill));
    let report = fresh.library.import_profile_file_commit(&bytes, None)?;
    assert_eq!((report.imported.len(), report.map_draws), (3, 1));
    let again = fresh.library.profile(report.profile_id, None)?;
    assert_eq!(again.spec.items.len(), 3);
    assert_eq!(again.category, CategoryId::Software);
    for id in &again.spec.items {
        let detail = fresh.library.detail(id)?;
        assert_eq!(detail.artifact.kind, ArtifactKind::Skill);
        assert_eq!(
            detail.artifact.metadata["fromPlugin"]["name"],
            "agent-skills"
        );
    }
    let view = fresh.library.harnmap(report.profile_id)?;
    assert!(view.drawn && view.stale.is_none() && view.unplaced.is_empty());
    assert_eq!(view.doc.placements.len(), 3);
    // Importing it again finds the artifacts already there.
    let second = fresh.library.import_profile_file(&bytes)?;
    assert!(second.artifacts.iter().all(|a| a.in_library));
    Ok(())
}

fn clear_calls(dir: &Path) -> R {
    for entry in fs::read_dir(dir)? {
        fs::remove_file(entry?.path())?;
    }
    Ok(())
}

// ---------------------------------------------------------------- 10. old libraries

/// Turns the library database under `data` back into a version 10 one, the schema before plugins
/// could be unpacked or maps drawn.
fn downgrade_to_v10(data: &Path) -> R {
    let conn = rusqlite::Connection::open(data.join("library/library.db"))?;
    conn.execute_batch(
        "DROP TABLE harnmap_draws;
         DROP TABLE harnfit_fits;
         ALTER TABLE profiles DROP COLUMN category;
         ALTER TABLE profiles DROP COLUMN category_note;
         ALTER TABLE sources DROP COLUMN unpack;
         PRAGMA user_version = 10;",
    )?;
    Ok(())
}

#[test]
fn a_library_with_plugins_imported_whole_long_ago_opens_works_and_can_be_unpacked() -> R {
    let pack = tempfile::tempdir()?;
    for (rel, text) in agent_skills_files() {
        put(pack.path(), rel, &text)?;
    }
    let kit = tempfile::tempdir()?;
    kit_marketplace(kit.path())?;
    let mods = tempfile::tempdir()?;
    mods_marketplace(mods.path())?;

    // What an old build left behind: three sources taken whole, a profile, a project.
    let lab = Lab::new()?;
    let current_version = lab.library.schema_version()?;
    let Lab { data, mut library } = lab;
    for dir in [pack.path(), kit.path(), mods.path()] {
        let preview = library.preview_with(&local(dir), &whole_options())?;
        let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        library.import(&preview, &ids, &TagChoices::new())?;
    }
    let find = |library: &Library, name: &str| -> R<String> {
        library
            .find(ArtifactKind::Plugin, name)?
            .ok_or_else(|| format!("{name} missing").into())
    };
    let (agent_skills, kit_id, radar) = (
        find(&library, "agent-skills")?,
        find(&library, "kit")?,
        find(&library, "radar")?,
    );
    let tag = library.create_tag("old", TagColor::Cyan)?;
    library.tag_artifact(&kit_id, tag.id)?;
    let profile = library.create_profile(
        "Old",
        "",
        &ProfileSpec {
            items: vec![agent_skills.clone(), kit_id.clone(), radar.clone()],
            ..ProfileSpec::default()
        },
        CategoryId::Software,
        None,
    )?;
    let project = tempfile::tempdir()?;
    let applied = library.apply(&library.plan_scaffold(project.path(), profile, None)?)?;
    drop(library);
    downgrade_to_v10(data.path())?;

    // It opens, migrates forward, and everything it held is still there.
    let mut lab = Lab {
        library: Library::open(data.path())?,
        data,
    };
    assert_eq!(lab.library.schema_version()?, current_version);
    let held = lab.held()?;
    assert!(
        held.iter().all(|(kind, _)| *kind == ArtifactKind::Plugin),
        "{held:?}"
    );
    assert_eq!(held.len(), 5, "{held:?}"); // agent-skills, kit, radar, bell, lint-lsp
    let page = lab.library.list(&Query::default())?;
    assert_eq!(page.items.len(), 5);
    assert_eq!(lab.library.artifact_tags(&kit_id)?[0].name, "old");
    assert!(lab.library.audits()?.iter().all(|a| a.audit.is_some()));
    let view = lab.library.profile(profile, None)?;
    assert_eq!((view.version, view.category), (1, CategoryId::Software));
    assert_eq!(
        view.spec.items,
        [agent_skills.clone(), kit_id.clone(), radar.clone()]
    );
    assert!(!lab.library.harnmap(profile)?.drawn);
    assert_eq!(
        lab.library.check_project(applied.project_id)?.status,
        DriftStatus::InSync
    );

    // The plugins are still whole and still enable the way they did.
    let again = lab.library.preview(&local(kit.path()))?;
    assert_eq!(names_of(&again, ArtifactKind::Plugin), ["kit"]);
    assert!(lab.library.plugin_holds(&again)?[0].whole);
    let fresh = tempfile::tempdir()?;
    lab.library
        .apply(&lab.library.plan_scaffold(fresh.path(), profile, None)?)?;
    let settings = settings_of(fresh.path())?;
    assert_eq!(settings["enabledPlugins"]["kit@kit-market"], true);
    assert_eq!(settings["enabledPlugins"]["radar@claude-mods"], true);
    let report = lab.library.check_updates(&CheckScope::All)?;
    assert!(report
        .sources
        .iter()
        .all(|s| s.result == SourceResult::Current));
    let bytes = lab
        .library
        .export_profile(profile, None, ExportOptions::default())?;
    let mut other = Lab::new()?;
    assert_eq!(
        other
            .library
            .import_profile_file_commit(&bytes, None)?
            .imported
            .len(),
        3
    );
    assert_eq!(other.held()?.len(), 3);
    other.reopen()?;

    // The skills-only ones can be unpacked in place; the one with hooks is refused.
    let refused = lab.library.unpack_in_library(&radar)?;
    assert!(refused.refused.is_some() && lab.library.detail(&radar).is_ok());
    for plugin in [&agent_skills, &kit_id] {
        let done = lab.library.unpack_in_library(plugin)?;
        assert!(done.refused.is_none(), "{done:?}");
        assert_eq!(done.profiles.len(), 1);
        assert_eq!(done.profiles[0].name, "Old");
    }
    let held = lab.held()?;
    assert_eq!(
        held.iter()
            .filter(|(kind, _)| *kind == ArtifactKind::Plugin)
            .count(),
        3,
        "radar, bell and lint-lsp stay: {held:?}"
    );
    assert_eq!(
        held.len(),
        3 + 3 + 3,
        "three skills, two skills and an agent: {held:?}"
    );
    let now = lab.library.profile(profile, None)?;
    assert_eq!(now.version, 3);
    assert_eq!(now.spec.items.len(), 1 + 6);
    assert_eq!(lab.library.profile(profile, Some(1))?.spec.items.len(), 3);

    // Updates now replay the unpacking the click stored.
    put(
        kit.path(),
        "plugins/kit/skills/lint/SKILL.md",
        &skill_md("lint", "Changed upstream."),
    )?;
    let report = lab.library.check_updates(&CheckScope::All)?;
    assert_eq!(
        report
            .sources
            .iter()
            .map(|s| outcome_of(&s.result).0)
            .sum::<u32>(),
        1
    );
    let lint = lab.id(ArtifactKind::Skill, "lint")?;
    assert_eq!(
        lab.library
            .update(&lint, UpdateChoice::TakeUpstream)?
            .outcome,
        UpdateOutcome::Updated
    );
    assert!(stored(&lab, &lint, "SKILL.md")?.contains("Changed upstream."));
    // The project made before still opens and moves to the new version.
    let switch = lab.library.plan_switch(applied.project_id, profile, None)?;
    lab.library.apply(&switch)?;
    let after = lab.library.check_project(applied.project_id)?;
    assert_eq!(after.status, DriftStatus::InSync, "{:?}", after.files);
    assert!(project
        .path()
        .join(".claude/skills/lint/SKILL.md")
        .is_file());
    assert_eq!(
        settings_of(project.path())?["enabledPlugins"]["radar@claude-mods"],
        true
    );
    assert!(settings_of(project.path())?["enabledPlugins"]
        .get("kit@kit-market")
        .is_none());
    Ok(())
}
