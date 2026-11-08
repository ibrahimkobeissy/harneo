//! The files of a plugin taken whole (ADR-005, hybrid): a plugin in its own folder keeps the whole
//! folder, since scripts reach their data files through paths built at run time; a plugin at the
//! scan root keeps its component scope, its default component folders whole and the top-level
//! entries its code names, so the repo's plumbing stays out. Nested plugins and the scanner's
//! skipped folders always stay out; the component scope still decides what unpacking offers.

use std::fs;
use std::path::Path;
use std::process::Command;

use harneo_core::artifact::{
    AgentAdapter, Artifact, ArtifactKind, ClaudeAdapter, ScanOptions, ScanReport,
};
use harneo_core::library::{Library, Source, TagChoices};
use harneo_core::updates::{CheckScope, UpdateChoice, UpdateOutcome, UpstreamState};

type R<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn put(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn scan(root: &Path, options: &ScanOptions) -> R<ScanReport> {
    Ok(ClaudeAdapter.scan(root, options)?)
}

fn whole() -> ScanOptions {
    ScanOptions {
        unpack_by_default: false,
        ..ScanOptions::default()
    }
}

fn plugin<'a>(report: &'a ScanReport, name: &str) -> R<&'a Artifact> {
    report
        .artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::Plugin && a.name == name)
        .ok_or_else(|| format!("plugin {name} not found").into())
}

fn has(a: &Artifact, file: &str) -> bool {
    a.files.iter().any(|f| f == file)
}

const SESSION_HOOK: &str = "import json, pathlib\nhere = pathlib.Path(__file__).parent\nrules = (here / 'rules.md').read_text()\ntpl = (here.parent / 'templates' / 'settings.json').read_text()\n";

/// A plugin shaped like `security-guardrails`: a hook script and a setup skill that read their
/// data files through paths they build themselves.
fn guardrails(root: &Path, base: &str) -> std::io::Result<()> {
    let p = |rel: &str| format!("{base}{rel}");
    put(
        root,
        &p(".claude-plugin/plugin.json"),
        r#"{"name":"security-guardrails","description":"Guardrails"}"#,
    )?;
    put(root, &p(".claude-plugin/icon.png"), "png")?;
    put(
        root,
        &p("hooks/hooks.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"python3 ${CLAUDE_PLUGIN_ROOT}/hooks/session_start.py"}]}]}}"#,
    )?;
    put(root, &p("hooks/session_start.py"), SESSION_HOOK)?;
    put(root, &p("hooks/rules.md"), "# Rules\n")?;
    put(
        root,
        &p("templates/settings.json"),
        "{\"permissions\":{}}\n",
    )?;
    put(root, &p("templates/gitleaks.toml"), "title = \"x\"\n")?;
    put(
        root,
        &p("skills/setup/SKILL.md"),
        "---\nname: setup\ndescription: Set up\n---\nRun the script.\n",
    )?;
    put(
        root,
        &p("skills/setup/scripts/setup-project.py"),
        "print(1)\n",
    )
}

const GUARDRAIL_FILES: [&str; 9] = [
    ".claude-plugin/icon.png",
    ".claude-plugin/plugin.json",
    "hooks/hooks.json",
    "hooks/rules.md",
    "hooks/session_start.py",
    "skills/setup/SKILL.md",
    "skills/setup/scripts/setup-project.py",
    "templates/gitleaks.toml",
    "templates/settings.json",
];

#[test]
fn a_plugin_at_the_root_keeps_its_hooks_folder_and_the_folders_its_code_names() -> R {
    let dir = tempfile::tempdir()?;
    guardrails(dir.path(), "")?;
    let report = scan(dir.path(), &whole())?;
    let a = plugin(&report, "security-guardrails")?;
    assert_eq!(a.files, GUARDRAIL_FILES);
    Ok(())
}

#[test]
fn a_marketplace_at_the_root_does_not_swallow_its_plugins_or_skipped_folders() -> R {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    put(
        root,
        ".claude-plugin/plugin.json",
        r#"{"name":"outer","description":"d"}"#,
    )?;
    put(
        root,
        ".claude-plugin/marketplace.json",
        r#"{"name":"m","plugins":[{"name":"outer","source":"./"},{"name":"security-guardrails","source":"./plugins/security-guardrails"},{"name":"bare","source":"./plugins/bare"}]}"#,
    )?;
    put(root, "docs/guide.md", "# guide\n")?;
    put(root, "node_modules/dep/index.js", "x\n")?;
    put(root, "target/out.txt", "x\n")?;
    guardrails(root, "plugins/security-guardrails/")?;
    put(root, "plugins/bare/tool.sh", "echo\n")?;

    let report = scan(root, &whole())?;
    let outer = plugin(&report, "outer")?;
    // At the root: no repo plumbing (`docs/`), no nested plugin, no skipped folder.
    assert_eq!(
        outer.files,
        [
            ".claude-plugin/marketplace.json",
            ".claude-plugin/plugin.json",
        ]
    );
    let inner = plugin(&report, "security-guardrails")?;
    assert_eq!(inner.files.len(), GUARDRAIL_FILES.len());
    assert!(has(inner, "plugins/security-guardrails/hooks/rules.md"));
    assert!(plugin(&report, "bare")?
        .files
        .iter()
        .all(|f| f.starts_with("plugins/bare/")));
    Ok(())
}

#[test]
fn unpacking_still_offers_only_components_as_parts() -> R {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    put(root, ".claude-plugin/plugin.json", r#"{"name":"kit"}"#)?;
    put(
        root,
        "skills/setup/SKILL.md",
        "---\nname: setup\ndescription: Set up\n---\nRun it.\n",
    )?;
    put(root, "templates/settings.json", "{}\n")?;
    put(root, "docs/guide.md", "# guide\n")?;
    let options = ScanOptions {
        unpack: vec![String::new()],
        ..ScanOptions::default()
    };
    let report = scan(root, &options)?;
    let kinds: Vec<(ArtifactKind, &str)> = report
        .artifacts
        .iter()
        .map(|a| (a.kind, a.name.as_str()))
        .collect();
    assert_eq!(kinds, [(ArtifactKind::Skill, "setup")]);
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> R {
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
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned().into())
    }
}

#[test]
fn an_update_check_brings_the_files_an_older_scope_left_out() -> R {
    // The library copy as an older build stored it: the plugin without its data files. It sits
    // in its own folder, like `security-guardrails` in its marketplace repo.
    let repo = tempfile::tempdir()?;
    let root = repo.path();
    git(root, &["init", "-q", "-b", "main"])?;
    put(
        root,
        "plugins/security-guardrails/.claude-plugin/plugin.json",
        r#"{"name":"security-guardrails"}"#,
    )?;
    put(
        root,
        "plugins/security-guardrails/hooks/session_start.py",
        SESSION_HOOK,
    )?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", "one"])?;
    let source = Source::parse_git(&format!("file://{}", root.display()), None, None)?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let options = whole();
    let preview = library.preview_with(&source, &options)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let id = library
        .find(ArtifactKind::Plugin, "security-guardrails")?
        .ok_or("plugin not imported")?;
    let stored = data.path().join("library/artifacts").join(&id);
    assert!(!stored.join("hooks/rules.md").exists());

    // The files the folder always had, which the old scope would not have kept.
    put(root, "plugins/security-guardrails/hooks/rules.md", "# Rules\n")?;
    put(
        root,
        "plugins/security-guardrails/templates/settings.json",
        "{}\n",
    )?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", "two"])?;
    drop(library);
    let mut library = Library::open(data.path())?;
    library.check_updates(&CheckScope::All)?;
    let status = library.update_status(&id)?.ok_or("plugin outdated")?;
    assert_eq!(status.state, UpstreamState::Outdated);
    let done = library.update(&id, UpdateChoice::TakeUpstream)?;
    assert_eq!(done.outcome, UpdateOutcome::Updated);
    assert!(stored.join("hooks/rules.md").exists());
    assert!(stored.join("templates/settings.json").exists());
    Ok(())
}

#[test]
fn a_plugin_in_its_own_folder_keeps_files_nothing_names() -> R {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    guardrails(root, "plugins/security-guardrails/")?;
    put(root, "plugins/security-guardrails/data/words.txt", "a\n")?;
    let report = scan(root, &whole())?;
    let a = plugin(&report, "security-guardrails")?;
    assert!(has(a, "plugins/security-guardrails/data/words.txt"));
    assert_eq!(a.files.len(), GUARDRAIL_FILES.len() + 1);
    Ok(())
}

#[test]
fn a_plugin_at_the_root_leaves_out_folders_only_prose_or_nothing_names() -> R {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    guardrails(root, "")?;
    // `docs` named in a skill's prose, `data` named nowhere: neither is code naming a path.
    put(
        root,
        "skills/setup/SKILL.md",
        "---\nname: setup\ndescription: Set up\n---\nSee \"docs\" and docs/setup.md.\n",
    )?;
    put(root, "docs/setup.md", "# setup\n")?;
    put(root, "data/words.txt", "a\n")?;
    let report = scan(root, &whole())?;
    let a = plugin(&report, "security-guardrails")?;
    assert!(
        has(a, "templates/settings.json"),
        "named by the hook script"
    );
    assert!(!a.files.iter().any(|f| f.starts_with("docs/")));
    assert!(!a.files.iter().any(|f| f.starts_with("data/")));
    Ok(())
}
