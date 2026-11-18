use std::fs;
use std::path::Path;

use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

/// 2026-11-18T12:34:56.789Z in milliseconds since the epoch.
const NOV_18: i64 = 1_795_005_296_789;

fn names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut out: Vec<String> = fs::read_dir(dir)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    Ok(out)
}

#[test]
fn civil_dates_round_trip_and_the_timestamp_is_iso() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    assert_eq!(
        civil_from_days(days_from_civil(2026, 11, 18)),
        (2026, 11, 18)
    );
    assert_eq!(timestamp(NOV_18), "2026-11-18T12:34:56.789Z");
    assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
}

#[test]
fn an_event_is_one_timestamped_line_in_the_days_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_at(dir.path(), NOV_18, "claude feature=layout\nsecond line")?;
    write_at(dir.path(), NOV_18 + 1000, "claude feature=harnmap")?;
    assert_eq!(names(dir.path())?, ["harneo-2026-11-18.log"]);
    let text = fs::read_to_string(dir.path().join("harneo-2026-11-18.log"))?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        [
            "2026-11-18T12:34:56.789Z claude feature=layout second line",
            "2026-11-18T12:34:57.789Z claude feature=harnmap",
        ]
    );
    Ok(())
}

#[test]
fn a_new_days_file_starts_and_removes_only_log_files_older_than_14_days() -> TestResult {
    let dir = tempfile::tempdir()?;
    let today = NOV_18.div_euclid(86_400_000);
    for age in [15, 14, 3, 40] {
        fs::write(dir.path().join(file_name(today - age)), "old\n")?;
    }
    fs::write(dir.path().join("notes.txt"), "mine\n")?;
    fs::write(dir.path().join("harneo-2026-13-45.log"), "odd\n")?;
    // Today's first line starts the file and prunes.
    write_at(dir.path(), NOV_18, "first")?;
    assert_eq!(
        names(dir.path())?,
        [
            "harneo-2026-11-04.log",
            "harneo-2026-11-15.log",
            "harneo-2026-11-18.log",
            "harneo-2026-13-45.log",
            "notes.txt",
        ]
    );
    // A later line of the same day does not prune again.
    fs::write(dir.path().join(file_name(today - 30)), "late\n")?;
    write_at(dir.path(), NOV_18 + 5, "second")?;
    assert!(dir.path().join(file_name(today - 30)).exists());
    Ok(())
}

#[test]
fn a_folder_that_cannot_be_written_is_not_an_error_for_the_caller() -> TestResult {
    let dir = tempfile::tempdir()?;
    let blocker = dir.path().join("file");
    fs::write(&blocker, "x")?;
    // The log folder is a path below a file: nothing can be created, and nothing panics.
    record(Some(&blocker.join("logs")), "event");
    assert!(write_at(&blocker.join("logs"), NOV_18, "event").is_err());
    Ok(())
}

#[test]
fn record_without_a_folder_writes_nothing_in_unit_tests() {
    assert!(default_dir().is_none());
    record(None, "event");
}

#[test]
fn paths_and_text_are_quoted_and_capped() {
    assert_eq!(quoted("a \"b\"\n"), r#""a \"b\"\n""#);
    assert_eq!(quoted(&"x".repeat(400)).len(), MAX_TEXT + 2);
    let many: Vec<String> = (0..25).map(|i| format!("p{i}")).collect();
    let field = paths_field(&many);
    assert!(field.contains("paths_total=25"), "{field}");
    assert!(field.contains("(+5 more)"), "{field}");
    assert!(field.contains("\"p19\"") && !field.contains("\"p20\""));
    assert_eq!(paths_field(&[]), "paths= paths_total=0");
}
