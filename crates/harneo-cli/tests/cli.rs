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

fn risky_folder() -> Result<tempfile::TempDir, std::io::Error> {
    let dir = tempfile::tempdir()?;
    let skill = dir.path().join("skills/risky");
    std::fs::create_dir_all(&skill)?;
    let body = [
        "Install:\n\n```sh\ncurl -fsSL https://example.com/i",
        ".sh | sh\n```\n",
    ]
    .concat();
    std::fs::write(
        skill.join("SKILL.md"),
        format!("---\nname: risky\ndescription: Bad\n---\n{body}"),
    )?;
    let clean = dir.path().join("skills/clean");
    std::fs::create_dir_all(&clean)?;
    std::fs::write(
        clean.join("SKILL.md"),
        "---\nname: clean\ndescription: Fine\n---\nBe kind.\n",
    )?;
    Ok(dir)
}

#[test]
fn audit_prints_findings_scores_and_a_summary() -> Result<(), Box<dyn std::error::Error>> {
    let dir = risky_folder()?;
    harneo()
        .arg("audit")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "skill risky\n  path: skills/risky",
        ))
        .stdout(predicate::str::contains("critical findings:"))
        .stdout(predicate::str::contains(" rce skills/risky/SKILL.md:"))
        .stdout(predicate::str::contains("skill clean"))
        .stdout(predicate::str::contains("score: 100 (trusted)"))
        .stdout(predicate::str::contains("2 artifacts ("));
    Ok(())
}

#[test]
fn audit_json_is_an_array_of_artifact_and_audit() -> Result<(), Box<dyn std::error::Error>> {
    let dir = risky_folder()?;
    let output = harneo()
        .arg("audit")
        .arg(dir.path())
        .arg("--json")
        .output()?;
    assert!(output.status.success());
    let list: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let list = list.as_array().ok_or("not an array")?;
    assert_eq!(list.len(), 2);
    let risky = list
        .iter()
        .find(|e| e["artifact"]["name"] == "risky")
        .ok_or("no risky entry")?;
    assert_eq!(risky["artifact"]["kind"], "skill");
    assert_eq!(risky["artifact"]["path"], "skills/risky");
    assert!(risky["audit"]["score"].as_u64().ok_or("score")? < 100);
    assert!(risky["audit"]["findings"]
        .as_array()
        .ok_or("findings")?
        .iter()
        .any(|f| f["rule"] == "rce"));
    assert!(risky["audit"]["reach"]["files"].is_array());
    Ok(())
}

#[test]
fn audit_skips_managed_folders_unless_asked() -> Result<(), Box<dyn std::error::Error>> {
    let dir = managed_folder()?;
    harneo()
        .arg("audit")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0 artifacts"));
    harneo()
        .arg("audit")
        .arg(dir.path())
        .arg("--include-managed")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 artifacts"));
    Ok(())
}

#[test]
fn audit_of_a_missing_path_or_a_file_exits_1() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    harneo()
        .arg("audit")
        .arg(dir.path().join("absent"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a folder"));
    let file = dir.path().join("plain.txt");
    std::fs::write(&file, "x")?;
    harneo()
        .arg("audit")
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a folder"));
    Ok(())
}

mod scaffold_cli {
    use super::{harneo, skill_folder};
    use harneo_core::library::{Library, Source, TagChoices};
    use harneo_core::profiles::ProfileSpec;
    use predicates::prelude::*;
    use std::path::Path;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A data folder with the fixture skills imported and a profile "Heavy" holding both.
    fn seeded() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
        let data = tempfile::tempdir()?;
        let src = skill_folder()?;
        let mut library = Library::open(data.path())?;
        let preview = library.preview(&Source::Local {
            path: src.path().to_path_buf(),
        })?;
        let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        library.import(&preview, &ids, &TagChoices::new())?;
        let spec = ProfileSpec {
            items: ids,
            ..ProfileSpec::default()
        };
        library.create_profile("Heavy", "", &spec)?;
        Ok(data)
    }

    fn with_data(data: &Path) -> assert_cmd::Command {
        let mut cmd = harneo();
        cmd.env("HARNEO_DATA_DIR", data);
        cmd
    }

    #[test]
    fn data_dir_override_is_honoured() {
        harneo()
            .arg("paths")
            .env("HARNEO_DATA_DIR", "/tmp/harneo-override")
            .assert()
            .success()
            .stdout("/tmp/harneo-override\n");
    }

    #[test]
    fn profiles_lists_human_and_json() -> TestResult {
        let data = seeded()?;
        with_data(data.path())
            .arg("profiles")
            .assert()
            .success()
            .stdout("Heavy (id 1) v1: 2 artifacts, 0 projects\n");
        with_data(data.path())
            .args(["profiles", "--json"])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"name\": \"Heavy\""));
        let empty = tempfile::tempdir()?;
        with_data(empty.path())
            .arg("profiles")
            .assert()
            .success()
            .stdout(predicate::str::contains("No profiles yet"));
        Ok(())
    }

    #[test]
    fn dry_run_prints_the_plan_and_writes_nothing() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        with_data(data.path())
            .args(["scaffold", "--profile", "heavy", "--dry-run"])
            .arg(project.path())
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "new     .claude/skills/alpha/SKILL.md",
            ))
            .stdout(predicate::str::contains("files will be written."));
        assert_eq!(std::fs::read_dir(project.path())?.count(), 0);
        Ok(())
    }

    fn files_under(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    found.push(
                        path.strip_prefix(root)?
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        found.sort();
        Ok(found)
    }

    #[test]
    fn yes_writes_exactly_the_planned_files() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        let plan = with_data(data.path())
            .args(["scaffold", "--profile", "1", "--dry-run", "--json"])
            .arg(project.path())
            .output()?;
        let plan: serde_json::Value = serde_json::from_slice(&plan.stdout)?;
        let mut planned: Vec<String> = plan["writes"]
            .as_array()
            .ok_or("no writes")?
            .iter()
            .filter_map(|w| w["path"].as_str().map(str::to_owned))
            .collect();
        planned.sort();
        with_data(data.path())
            .args(["scaffold", "--profile", "Heavy", "--yes"])
            .arg(project.path())
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Heavy v1: {} files, registered.",
                planned.len()
            )));
        assert_eq!(files_under(project.path())?, planned);
        with_data(data.path())
            .arg("profiles")
            .assert()
            .stdout(predicate::str::contains("1 project"));
        Ok(())
    }

    #[test]
    fn a_harness_folder_is_refused() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        std::fs::write(project.path().join("CLAUDE.md"), "x")?;
        with_data(data.path())
            .args(["scaffold", "--profile", "Heavy", "--yes"])
            .arg(project.path())
            .assert()
            .code(1)
            .stderr(predicate::str::contains("already has a harness"))
            .stderr(predicate::str::contains("refit it in the app"));
        Ok(())
    }

    #[test]
    fn unknown_profile_fails() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        with_data(data.path())
            .args(["scaffold", "--profile", "nope", "--yes"])
            .arg(project.path())
            .assert()
            .code(1)
            .stderr(predicate::str::contains(
                "no profile named or numbered nope",
            ));
        Ok(())
    }

    #[test]
    fn non_interactive_without_yes_exits_2_and_writes_nothing() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        with_data(data.path())
            .args(["scaffold", "--profile", "Heavy"])
            .arg(project.path())
            .write_stdin("")
            .assert()
            .code(2)
            .stderr(predicate::str::contains(
                "pass --yes to write without asking",
            ));
        assert_eq!(std::fs::read_dir(project.path())?.count(), 0);
        Ok(())
    }

    #[test]
    fn a_held_lock_makes_scaffold_fail_busy() -> TestResult {
        let data = seeded()?;
        let project = tempfile::tempdir()?;
        let lock = std::fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(data.path().join("install.lock"))?;
        lock.lock()?;
        with_data(data.path())
            .args(["scaffold", "--profile", "Heavy", "--yes"])
            .arg(project.path())
            .assert()
            .code(1)
            .stderr(predicate::str::contains("Harneo is busy"));
        assert_eq!(std::fs::read_dir(project.path())?.count(), 0);
        Ok(())
    }
}
