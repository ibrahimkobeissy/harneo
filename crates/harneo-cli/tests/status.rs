use std::collections::BTreeMap;
use std::path::Path;

use assert_cmd::Command;
use harneo_core::artifact::ArtifactKind;
use harneo_core::library::{Library, Source, TagChoices};
use harneo_core::profiles::ProfileSpec;
use predicates::prelude::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const AGENT: &str = ".claude/agents/rev.md";

struct Fx {
    data: tempfile::TempDir,
    work: tempfile::TempDir,
    library: Library,
    profile: i64,
}

fn put(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

fn harneo(fx: &Fx) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_harneo"));
    cmd.env("HARNEO_DATA_DIR", fx.data.path());
    cmd
}

/// A library with a skill and an agent, and a profile "web" holding both.
fn fx() -> Result<Fx, Box<dyn std::error::Error>> {
    let src = tempfile::tempdir()?;
    put(
        src.path(),
        ".claude/skills/s/SKILL.md",
        "---\nname: s\ndescription: Does a thing.\n---\nBody.\n",
    )?;
    put(
        src.path(),
        AGENT,
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nTwo\nThree\n",
    )?;
    let data = tempfile::tempdir()?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: src.path().to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    let find = |kind, name| -> Result<String, String> {
        library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} missing"))
    };
    let spec = ProfileSpec {
        items: vec![
            find(ArtifactKind::Skill, "s")?,
            find(ArtifactKind::Agent, "rev")?,
        ],
        settings: None,
        mcp: Vec::new(),
        manual_only: BTreeMap::new(),
    };
    let profile = library.create_profile("web", "", &spec)?;
    Ok(Fx {
        data,
        work: tempfile::tempdir()?,
        library,
        profile,
    })
}

/// Scaffolds a project called `name` and returns its folder.
fn scaffold(fx: &Fx, name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = fx.work.path().join(name);
    std::fs::create_dir_all(&dir)?;
    let plan = fx.library.plan_scaffold(&dir, fx.profile, None)?;
    fx.library.apply(&plan)?;
    Ok(std::fs::canonicalize(dir)?)
}

#[test]
fn no_projects_says_so() -> TestResult {
    let fx = fx()?;
    harneo(&fx)
        .arg("status")
        .assert()
        .success()
        .stdout("No projects yet.\n");
    Ok(())
}

#[test]
fn list_shows_in_sync_drift_and_missing_projects() -> TestResult {
    let fx = fx()?;
    scaffold(&fx, "clean")?;
    let edited = scaffold(&fx, "edited")?;
    let gone = scaffold(&fx, "gone")?;
    put(
        &edited,
        AGENT,
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nChanged\n",
    )?;
    put(
        &edited,
        ".claude/skills/extra/SKILL.md",
        "---\nname: extra\ndescription: Mine\n---\nX\n",
    )?;
    std::fs::remove_dir_all(&gone)?;
    let output = harneo(&fx).arg("status").output()?;
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout)?;
    let line = |name: &str| text.lines().find(|l| l.starts_with(name)).unwrap_or("");
    assert!(line("clean").ends_with("web v1  In sync"), "{text}");
    assert!(line("edited").contains("web v1  Drift · "), "{text}");
    assert!(line("edited").contains(" files"), "{text}");
    assert!(line("gone").ends_with("web v1  Missing"), "{text}");
    assert!(line("clean").contains(&*fx.work.path().join("clean").to_string_lossy()));
    Ok(())
}

#[test]
fn path_lists_edited_deleted_and_added_files() -> TestResult {
    let fx = fx()?;
    let project = scaffold(&fx, "app")?;
    put(
        &project,
        AGENT,
        "---\nname: rev\ndescription: Reviews code\n---\nOne\nChanged\n",
    )?;
    std::fs::remove_file(project.join(".claude/skills/s/SKILL.md"))?;
    put(
        &project,
        ".claude/skills/extra/SKILL.md",
        "---\nname: extra\ndescription: Mine\n---\nX\n",
    )?;
    harneo(&fx)
        .arg("status")
        .arg(&project)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "edited  +1 −2  .claude/agents/rev.md\n",
        ))
        .stdout(predicate::str::contains(
            "deleted  .claude/skills/s/SKILL.md\n",
        ))
        .stdout(predicate::str::contains("added  .claude/skills/extra\n"));
    Ok(())
}

#[test]
fn path_of_a_missing_project_says_missing() -> TestResult {
    let fx = fx()?;
    let project = scaffold(&fx, "app")?;
    std::fs::remove_dir_all(&project)?;
    harneo(&fx)
        .arg("status")
        .arg(&project)
        .assert()
        .success()
        .stdout(predicate::str::contains("Missing"));
    Ok(())
}

#[test]
fn path_of_an_unregistered_folder_exits_1() -> TestResult {
    let fx = fx()?;
    let other = fx.work.path().join("other");
    std::fs::create_dir_all(&other)?;
    harneo(&fx)
        .arg("status")
        .arg(&other)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a registered project"));
    Ok(())
}

#[test]
fn exit_code_flag_returns_3_only_when_drifted() -> TestResult {
    let fx = fx()?;
    let project = scaffold(&fx, "app")?;
    harneo(&fx).args(["status", "--exit-code"]).assert().code(0);
    harneo(&fx)
        .arg("status")
        .arg(&project)
        .arg("--exit-code")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("In sync"));
    put(&project, AGENT, "edited\n")?;
    harneo(&fx).arg("status").assert().code(0);
    harneo(&fx).args(["status", "--exit-code"]).assert().code(3);
    harneo(&fx)
        .arg("status")
        .arg(&project)
        .arg("--exit-code")
        .assert()
        .code(3);
    Ok(())
}

#[test]
fn a_missing_project_alone_does_not_exit_3() -> TestResult {
    let fx = fx()?;
    let project = scaffold(&fx, "app")?;
    std::fs::remove_dir_all(project)?;
    harneo(&fx).args(["status", "--exit-code"]).assert().code(0);
    Ok(())
}

#[test]
fn json_prints_the_core_reports() -> TestResult {
    let fx = fx()?;
    let project = scaffold(&fx, "app")?;
    put(&project, AGENT, "edited\n")?;
    let output = harneo(&fx).args(["status", "--json"]).output()?;
    assert!(output.status.success());
    let list: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(list[0]["project"]["name"], "app");
    assert_eq!(list[0]["summary"]["status"], "drift");
    assert_eq!(list[0]["summary"]["drifted"], 1);
    let output = harneo(&fx)
        .arg("status")
        .arg(&project)
        .arg("--json")
        .output()?;
    assert!(output.status.success());
    let detail: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(detail["report"]["status"], "drift");
    assert_eq!(detail["report"]["files"][0]["path"], AGENT);
    assert_eq!(detail["report"]["files"][0]["change"]["type"], "edited");
    Ok(())
}
