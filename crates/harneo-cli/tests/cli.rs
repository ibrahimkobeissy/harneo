use assert_cmd::Command;
use predicates::prelude::*;

fn harneo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_harneo"))
}

#[test]
fn version_flag_prints_name_and_core_version() {
    harneo()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("harneo {}\n", harneo_core::version()));
}

#[test]
fn paths_prints_the_data_dir_alone() {
    harneo()
        .arg("paths")
        .env("XDG_DATA_HOME", "/tmp/harneo-cli-test")
        .assert()
        .success()
        .stdout("/tmp/harneo-cli-test/io.harneo\n");
}

#[test]
fn unknown_command_fails() {
    harneo()
        .arg("nope")
        .assert()
        .failure()
        .stderr(predicate::str::contains("nope"));
}

fn skill_folder() -> Result<tempfile::TempDir, std::io::Error> {
    let dir = tempfile::tempdir()?;
    for (name, text) in [
        ("alpha", "---\nname: alpha\ndescription: First\n---\nBody"),
        ("beta", "---\nname: beta\ndescription: Second\n---\nBody"),
    ] {
        let skill = dir.path().join("skills").join(name);
        std::fs::create_dir_all(&skill)?;
        std::fs::write(skill.join("SKILL.md"), text)?;
    }
    Ok(dir)
}

#[test]
fn scan_lists_artifacts_and_a_summary() -> Result<(), Box<dyn std::error::Error>> {
    let dir = skill_folder()?;
    harneo()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("skill alpha"))
        .stdout(predicate::str::contains("path: skills/beta"))
        .stdout(predicate::str::contains("files: 1"))
        .stdout(predicate::str::contains("2 artifacts (2 skill)"));
    Ok(())
}

#[test]
fn scan_json_has_an_artifacts_array() -> Result<(), Box<dyn std::error::Error>> {
    let dir = skill_folder()?;
    let output = harneo()
        .arg("scan")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["artifacts"].as_array().map(Vec::len), Some(2));
    Ok(())
}

#[test]
fn scan_of_a_missing_path_exits_1() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    harneo()
        .arg("scan")
        .arg(dir.path().join("absent"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a folder"));
    Ok(())
}

#[test]
fn scan_of_a_file_exits_1() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("plain.txt");
    std::fs::write(&file, "x")?;
    harneo()
        .arg("scan")
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a folder"));
    Ok(())
}

#[test]
fn scan_prints_diagnostics_and_still_exits_0() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let skill = dir.path().join("skills/bare");
    std::fs::create_dir_all(&skill)?;
    std::fs::write(skill.join("SKILL.md"), "no frontmatter")?;
    harneo()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning skill.missing-description",
        ))
        .stdout(predicate::str::contains("0 errors, 1 warnings"));
    Ok(())
}

#[test]
fn scan_json_of_the_project_fixture_lists_its_ten_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../harneo-core/tests/fixtures/artifact/project");
    let output = harneo().arg("scan").arg(&fixture).arg("--json").output()?;
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let artifacts = report["artifacts"].as_array().ok_or("no artifacts")?;
    assert_eq!(artifacts.len(), 10);
    Ok(())
}

fn managed_folder() -> Result<tempfile::TempDir, std::io::Error> {
    let dir = tempfile::tempdir()?;
    for rel in ["skills/synced/a_b/x", "skills/synced/c_d/x"] {
        let skill = dir.path().join(rel);
        std::fs::create_dir_all(&skill)?;
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: x\ndescription: d\n---\n",
        )?;
    }
    Ok(dir)
}

#[test]
fn scan_skips_managed_folders_unless_asked() -> Result<(), Box<dyn std::error::Error>> {
    let dir = managed_folder()?;
    harneo()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("scan.managed-skipped"))
        .stdout(predicate::str::contains("0 artifacts"));
    harneo()
        .arg("scan")
        .arg(dir.path())
        .arg("--include-managed")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 artifacts (2 skill)"))
        .stdout(predicate::str::contains("scan.managed-skipped").not());
    Ok(())
}
