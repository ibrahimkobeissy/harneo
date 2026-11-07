//! The executable bit of a script survives import, update, install and a profile file.

use crate::harnmap::CategoryId;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::fixture::{put, TestResult};
use crate::artifact::ArtifactKind;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::{ExportOptions, ProfileSpec};
use crate::updates::{CheckScope, UpdateChoice};

const SCRIPT: &str = "skills/s/scripts/run.sh";

fn mode(path: &Path) -> std::io::Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

fn chmod(path: &Path, bits: u32) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(bits))
}

fn stored(data: &Path, id: &str, rel: &str) -> std::path::PathBuf {
    data.join("library/artifacts").join(id).join(rel)
}

fn source(root: &Path, body: &str) -> TestResult {
    put(
        root,
        "skills/s/SKILL.md",
        &format!("---\nname: s\ndescription: Does a thing.\n---\n{body}\n"),
    )?;
    put(root, SCRIPT, "echo hi\n")?;
    chmod(&root.join(SCRIPT), 0o755)?;
    put(root, "skills/s/scripts/plain.txt", "text\n")?;
    Ok(())
}

fn import(
    src: &Path,
    data: &Path,
) -> std::result::Result<(Library, String), Box<dyn std::error::Error>> {
    let mut library = Library::open(data)?;
    let preview = library.preview(&Source::Local {
        path: src.to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let id = library
        .find(ArtifactKind::Skill, "s")?
        .ok_or("skill missing")?;
    Ok((library, id))
}

fn spec(id: &str) -> ProfileSpec {
    ProfileSpec {
        items: vec![id.to_owned()],
        settings: None,
        mcp: Vec::new(),
        manual_only: BTreeMap::new(),
    }
}

#[test]
fn import_update_scaffold_and_profile_file_keep_the_executable_bit() -> TestResult {
    let src = tempfile::tempdir()?;
    source(src.path(), "one")?;
    let data = tempfile::tempdir()?;
    let (mut library, id) = import(src.path(), data.path())?;

    // Import keeps the bit on the script only.
    assert_eq!(
        mode(&stored(data.path(), &id, "scripts/run.sh"))? & 0o111,
        0o111
    );
    assert_eq!(
        mode(&stored(data.path(), &id, "scripts/plain.txt"))? & 0o111,
        0
    );

    // An update keeps it too.
    put(
        src.path(),
        "skills/s/SKILL.md",
        "---\nname: s\ndescription: Does a thing.\n---\ntwo\n",
    )?;
    library.check_updates(&CheckScope::All)?;
    library.update(&id, UpdateChoice::TakeUpstream)?;
    assert!(fs::read_to_string(stored(data.path(), &id, "SKILL.md"))?.contains("two"));
    assert_eq!(
        mode(&stored(data.path(), &id, "scripts/run.sh"))? & 0o111,
        0o111
    );

    // Scaffolding writes the script executable and the text file not.
    let profile = library.create_profile("p", "", &spec(&id), CategoryId::Software, None)?;
    let project = tempfile::tempdir()?;
    let plan = library.plan_scaffold(project.path(), profile, None)?;
    let run = plan
        .writes
        .iter()
        .find(|w| w.path.ends_with("scripts/run.sh"))
        .ok_or("script not in plan")?;
    assert!(run.executable);
    assert!(!plan
        .writes
        .iter()
        .any(|w| w.path.ends_with("plain.txt") && w.executable));
    library.apply(&plan)?;
    let written = project.path().join(&run.path);
    assert_eq!(mode(&written)?, 0o755);
    let text = plan
        .writes
        .iter()
        .find(|w| w.path.ends_with("plain.txt"))
        .ok_or("text not in plan")?;
    assert_eq!(mode(&project.path().join(&text.path))? & 0o111, 0);

    // Export, then import into a fresh data dir.
    let exported = library.export_profile(profile, None, ExportOptions::default())?;
    let json: serde_json::Value = serde_json::from_slice(&exported)?;
    let files = json["artifacts"][0]["files"].as_array().ok_or("no files")?;
    let flagged: Vec<&str> = files
        .iter()
        .filter(|f| f.get("executable") == Some(&serde_json::Value::Bool(true)))
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert_eq!(flagged, ["scripts/run.sh"]);
    let fresh = tempfile::tempdir()?;
    let mut other = Library::open(fresh.path())?;
    other.import_profile_file_commit(&exported, None)?;
    let other_id = other
        .find(ArtifactKind::Skill, "s")?
        .ok_or("skill missing")?;
    assert_eq!(
        mode(&stored(fresh.path(), &other_id, "scripts/run.sh"))? & 0o111,
        0o111
    );
    assert_eq!(
        mode(&stored(fresh.path(), &other_id, "scripts/plain.txt"))? & 0o111,
        0
    );
    Ok(())
}
