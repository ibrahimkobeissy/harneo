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
