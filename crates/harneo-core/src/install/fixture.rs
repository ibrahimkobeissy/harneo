//! Shared test setup: a library with a profile, and empty project folders.

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
    let profile = library.create_profile("web", "", &spec)?;
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
