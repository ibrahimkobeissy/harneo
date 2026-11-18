//! Shared test setup: a library with a profile, and empty project folders.

use crate::harnmap::CategoryId;
use std::collections::BTreeMap;
use std::path::Path;

use super::{Plan, Write, WriteAction};
use crate::artifact::ArtifactKind;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

pub type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

pub struct Fx {
    pub data: tempfile::TempDir,
    _src: tempfile::TempDir,
    pub library: Library,
    pub profile: i64,
}

pub fn put(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

/// A library holding a skill, an agent and a CLAUDE.md, and profile "web" with all three.
pub fn fx() -> std::result::Result<Fx, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    put(
        src.path(),
        ".claude/skills/s/SKILL.md",
        "---\nname: s\ndescription: Does a thing.\n---\nBody.\n",
    )?;
    put(src.path(), ".claude/skills/s/scripts/run.sh", "echo hi\n")?;
    put(
        src.path(),
        ".claude/agents/rev.md",
        "---\nname: rev\ndescription: Reviews code\n---\nB\n",
    )?;
    put(src.path(), "CLAUDE.md", "# Project\n")?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let find = |kind, name| -> std::result::Result<String, String> {
        library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} missing"))
    };
    let spec = ProfileSpec {
        items: vec![
            find(ArtifactKind::Skill, "s")?,
            find(ArtifactKind::Agent, "rev")?,
            find(ArtifactKind::ClaudeMd, "CLAUDE.md")?,
        ],
        settings: None,
        mcp: Vec::new(),
        manual_only: BTreeMap::new(),
    };
    let profile = library.create_profile("web", "", &spec, CategoryId::Software, None)?;
    Ok(Fx {
        data,
        _src: src,
        library,
        profile,
    })
}

pub fn write(path: &str, action: WriteAction, text: Option<&str>) -> Write {
    Write {
        path: path.to_owned(),
        action,
        bytes: text.map(|t| t.as_bytes().to_vec()),
        diff: None,
        artifact: None,
        executable: false,
        mode: None,
    }
}

/// A hand-made plan for `target` using the fixture's profile.
pub fn plan(fx: &Fx, target: &Path, writes: Vec<Write>) -> std::io::Result<Plan> {
    let backups = writes
        .iter()
        .filter(|w| matches!(w.action, WriteAction::Replace | WriteAction::Delete))
        .count();
    Ok(Plan {
        target: target.canonicalize()?.display().to_string(),
        profile_id: fx.profile,
        profile_name: "web".to_owned(),
        profile_version: 1,
        reason: "test".to_owned(),
        writes,
        backup_count: u32::try_from(backups).unwrap_or(0),
        ..Default::default()
    })
}

/// What `HARNESS.md` held before Harneo stopped writing it (user decision 2026-11-18).
pub const OLD_HARNESS: &str = "# Harness\n\nInstalled by Harneo from profile **web v1** on 2026-01-01.\n\n| Artifact | Type | Source | Why it's here |\n|---|---|---|---|\n| s | Skill | Local | Does a thing |\n";

/// Turns a project scaffolded now into one scaffolded before that decision: `HARNESS.md` is
/// written (with `harness`) and recorded in the manifest, and `CLAUDE.md` gets the
/// `@HARNESS.md` line, also recorded.
pub fn make_legacy(
    lib: &Library,
    root: &Path,
    project_id: i64,
    harness: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let claude = std::fs::read_to_string(root.join("CLAUDE.md"))?;
    let claude = format!("{claude}\n@HARNESS.md\n");
    put(root, "CLAUDE.md", &claude)?;
    put(root, "HARNESS.md", harness)?;
    let record = |path: &str, text: &str, artifact: Option<String>| {
        lib.store.conn.execute(
            "INSERT OR REPLACE INTO installed_files (project_id, path, digest, artifact_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                project_id,
                path,
                super::plan::digest(text.as_bytes()),
                artifact
            ],
        )
    };
    let claude_artifact = lib
        .installed_files(project_id)?
        .into_iter()
        .find(|f| f.path == "CLAUDE.md")
        .and_then(|f| f.artifact_id);
    record("CLAUDE.md", &claude, claude_artifact)?;
    record("HARNESS.md", harness, None)?;
    Ok(())
}
