//! Shared test setup: a library with a profile, and projects scaffolded through the real plan
//! and apply.

use std::collections::BTreeMap;
use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

pub type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

pub const AGENT: &str = ".claude/agents/rev.md";
pub const SKILL: &str = ".claude/skills/s/SKILL.md";

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

pub fn read(root: &Path, rel: &str) -> std::io::Result<String> {
    std::fs::read_to_string(root.join(rel))
}

/// A library holding a skill, an agent and a CLAUDE.md, and profile "web" with all three.
pub fn fx() -> std::result::Result<Fx, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    put(
        src.path(),
        ".claude/skills/s/SKILL.md",
        "---\nname: s\ndescription: Does a thing.\n---\nBody.\n",
    )?;
    put(
        src.path(),
        ".claude/agents/rev.md",
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nTwo\nThree\n",
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

/// A new empty project folder scaffolded with the fixture profile and registered; returns its
/// folder (keep it alive) and id.
pub fn project(
    f: &Fx,
) -> std::result::Result<(tempfile::TempDir, i64), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let plan = f.library.plan_scaffold(dir.path(), f.profile, None)?;
    let id = f.library.apply(&plan)?.project_id;
    Ok((dir, id))
}
