//! Local log files: one plain-text file per day in the log folder (`paths::log_dir`), one line
//! per event, each starting with an ISO 8601 timestamp (UTC).
//!
//! The logs never leave the machine. They hold what happened (which feature called Claude, the
//! paths it sent, how long it took, the exit code, the error text) and never prompt text, file
//! contents, secrets or environment values. A logging failure never fails the operation: every
//! error here is dropped.
//!
//! A file older than [`KEEP_DAYS`] days is deleted when a new day's file starts.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Files older than this many days are deleted.
pub const KEEP_DAYS: i64 = 14;
/// Paths listed in one event; the rest is only counted.
const MAX_PATHS: usize = 20;
/// Characters kept of an error text or a path.
const MAX_TEXT: usize = 300;

const PREFIX: &str = "harneo-";
const SUFFIX: &str = ".log";

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date (year, month, day) of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The file name of the day `days` since 1970-01-01.
fn file_name(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{PREFIX}{y:04}-{m:02}-{d:02}{SUFFIX}")
}

/// The day a log file name stands for, or `None` for any other file.
fn day_of(name: &str) -> Option<i64> {
    let date = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    let valid = parts.next().is_none()
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && file_name(days_from_civil(year, month, day)) == name;
    valid.then(|| days_from_civil(year, month, day))
}

fn timestamp(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        millis.rem_euclid(1000)
    )
}

/// Deletes the log files of `dir` older than [`KEEP_DAYS`] days before `today`.
fn prune(dir: &Path, today: i64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name();
        let old = name
            .to_str()
            .and_then(day_of)
            .is_some_and(|day| today - day > KEEP_DAYS);
        if old {
            // Best effort: a file that cannot be removed is left for the next day.
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// One event on one line: line breaks and other control characters become spaces.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Appends `event` to the file of the day `millis` falls on, starting that file (and deleting
/// the old ones) when it is the day's first line.
fn write_at(dir: &Path, millis: i64, event: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let today = millis.div_euclid(86_400_000);
    let file = dir.join(file_name(today));
    let fresh = !file.exists();
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Paths and errors are the user's own business: readable by the user only.
        options.mode(0o600);
    }
    let mut out = options.open(&file)?;
    writeln!(out, "{} {}", timestamp(millis), one_line(event))?;
    if fresh {
        prune(dir, today);
    }
    Ok(())
}

/// Writes one event to the log folder: `dir` when given (tests), else the platform's
/// ([`crate::paths::log_dir`]). Nothing is written by unit tests that name no folder, so running
/// them leaves the developer's real logs alone. Never fails.
pub(crate) fn record(dir: Option<&Path>, event: &str) {
    let folder = match dir {
        Some(dir) => dir.to_path_buf(),
        None => match default_dir() {
            Some(dir) => dir,
            None => return,
        },
    };
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0);
    // A logging failure never fails the operation.
    let _ = write_at(&folder, millis, event);
}

#[cfg(not(test))]
fn default_dir() -> Option<std::path::PathBuf> {
    crate::paths::log_dir().ok()
}

#[cfg(test)]
fn default_dir() -> Option<std::path::PathBuf> {
    None
}

/// `text` as one quoted, escaped token, cut to [`MAX_TEXT`] characters.
pub(crate) fn quoted(text: &str) -> String {
    let cut: String = text.chars().take(MAX_TEXT).collect();
    serde_json::to_string(&cut).unwrap_or_else(|_| "\"\"".to_owned())
}

/// A list of paths for one event: the first [`MAX_PATHS`], quoted, and the total.
pub(crate) fn paths_field(paths: &[String]) -> String {
    let shown: Vec<String> = paths.iter().take(MAX_PATHS).map(|p| quoted(p)).collect();
    let more = paths.len().saturating_sub(MAX_PATHS);
    if more > 0 {
        format!(
            "paths={} paths_total={} (+{more} more)",
            shown.join(","),
            paths.len()
        )
    } else {
        format!("paths={} paths_total={}", shown.join(","), paths.len())
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
