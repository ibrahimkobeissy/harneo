use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::parse_tests::{assistant, tool, user, SECRET_TEXT};
use super::{IndexProgress, UsageIndex};

const TS: &str = "2026-10-20T10:00:00.000Z";

fn setup() -> (tempfile::TempDir, UsageIndex) {
    let dir = tempfile::tempdir().expect("temp dir");
    let index = UsageIndex::open(&dir.path().join("data")).expect("open");
    (dir, index)
}

fn logs(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let root = dir.path().join("projects");
    fs::create_dir_all(root.join("-work-app")).expect("mkdir");
    root
}

fn skill_line(session: &str, name: &str) -> String {
    assistant(
        session,
        TS,
        "/work/app",
        json!([tool("Skill", json!({ "skill": name }))]),
    )
}

fn write(path: &Path, lines: &[String]) {
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(path, text).expect("write log");
}

fn append(path: &Path, text: &str) {
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open");
    f.write_all(text.as_bytes()).expect("append");
}

fn count(index: &UsageIndex) -> i64 {
    index
        .connect()
        .expect("conn")
        .query_row("SELECT COUNT(*) FROM uses", [], |r| r.get(0))
        .expect("count")
}

#[test]
fn indexes_every_use_type_and_counts_a_session_once() {
    let (dir, index) = setup();
    let root = logs(&dir);
    write(
        &root.join("-work-app/a.jsonl"),
        &[
            skill_line("s1", "webperf"),
            skill_line("s1", "webperf"),
            skill_line("s1", "webperf"),
            assistant(
                "s1",
                TS,
                "/work/app",
                json!([
                    tool("Task", json!({"subagent_type": "rev"})),
                    tool("mcp__gh__x", json!({}))
                ]),
            ),
            user("s1", TS, "/work/app", "<command-name>/ship</command-name>"),
            skill_line("s2", "webperf"),
        ],
    );
    let report = index.index(&root, |_| {}).expect("index");
    assert_eq!(
        (report.files, report.files_read, report.failed_lines),
        (1, 1, 0)
    );
    assert_eq!(report.new_uses, 5);
    let today = super::day_number("2026-10-21").expect("day");
    let usage = index
        .usage_of_at(crate::artifact::ArtifactKind::Skill, "webperf", today)
        .expect("usage");
    assert_eq!(usage.sessions_8w, 2);
    assert_eq!(usage.last_used.as_deref(), Some("2026-10-20"));
    assert_eq!(usage.projects, vec!["/work/app".to_string()]);
    assert_eq!(usage.weekly[7], 2);
    for (kind, name) in [
        (crate::artifact::ArtifactKind::Agent, "rev"),
        (crate::artifact::ArtifactKind::Mcp, "gh"),
        (crate::artifact::ArtifactKind::Command, "ship"),
    ] {
        let u = index.usage_of_at(kind, name, today).expect("usage");
        assert_eq!(u.sessions_8w, 1, "{name}");
    }
}

#[test]
fn message_text_never_reaches_the_database_files() {
    let (dir, index) = setup();
    let root = logs(&dir);
    write(
        &root.join("-work-app/a.jsonl"),
        &[
            user("s1", TS, "/work/app", &format!("<command-name>/ship</command-name> {SECRET_TEXT}")),
            assistant(
                "s1",
                TS,
                "/work/app",
                json!([{"type": "text", "text": SECRET_TEXT}, tool("Skill", json!({"skill": "x", "args": SECRET_TEXT})), tool("Bash", json!({"command": SECRET_TEXT}))]),
            ),
            json!({"type": "user", "sessionId": "s1", "timestamp": TS, "message": {"content": SECRET_TEXT}}).to_string(),
        ],
    );
    index.index(&root, |_| {}).expect("index");
    assert!(count(&index) >= 2);
    let db_dir = dir.path().join("data/usage");
    let mut scanned = 0;
    for entry in fs::read_dir(db_dir).expect("dir") {
        let bytes = fs::read(entry.expect("entry").path()).expect("read");
        scanned += 1;
        assert!(
            !bytes
                .windows(SECRET_TEXT.len())
                .any(|w| w == SECRET_TEXT.as_bytes()),
            "message text found in the index"
        );
    }
    assert!(scanned >= 1);
}

#[test]
fn a_second_run_reads_nothing_and_an_append_reads_only_the_new_lines() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let file = root.join("-work-app/a.jsonl");
    write(&file, &[skill_line("s1", "a")]);
    assert_eq!(index.index(&root, |_| {}).expect("1").new_uses, 1);
    let again = index.index(&root, |_| {}).expect("2");
    assert_eq!((again.files, again.files_read, again.new_uses), (1, 0, 0));
    append(&file, &format!("{}\n", skill_line("s2", "b")));
    let third = index.index(&root, |_| {}).expect("3");
    assert_eq!((third.files_read, third.new_uses), (1, 1));
    assert_eq!(count(&index), 2);
}

#[test]
fn a_half_written_last_line_waits_for_its_newline() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let file = root.join("-work-app/a.jsonl");
    let line = skill_line("s1", "a");
    write(&file, &[]);
    fs::write(&file, &line).expect("no newline");
    assert_eq!(index.index(&root, |_| {}).expect("1").new_uses, 0);
    append(&file, "\n");
    assert_eq!(index.index(&root, |_| {}).expect("2").new_uses, 1);
}

#[test]
fn a_file_that_shrank_is_read_again_from_the_start() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let file = root.join("-work-app/a.jsonl");
    write(
        &file,
        &[
            skill_line("s1", "a"),
            skill_line("s1", "b"),
            skill_line("s1", "c"),
        ],
    );
    index.index(&root, |_| {}).expect("1");
    write(&file, &[skill_line("s9", "z")]);
    let report = index.index(&root, |_| {}).expect("2");
    assert_eq!((report.files_read, report.new_uses), (1, 1));
}

#[test]
fn symlinks_and_other_files_are_ignored() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).expect("mkdir");
    write(&outside.join("o.jsonl"), &[skill_line("s1", "linked")]);
    write(
        &root.join("-work-app/notes.txt"),
        &[skill_line("s1", "text")],
    );
    std::os::unix::fs::symlink(outside.join("o.jsonl"), root.join("-work-app/l.jsonl"))
        .expect("file link");
    std::os::unix::fs::symlink(&outside, root.join("dirlink")).expect("dir link");
    let report = index.index(&root, |_| {}).expect("index");
    assert_eq!((report.files, report.new_uses), (0, 0));
}

#[test]
fn broken_and_huge_lines_are_counted_and_skipped() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let file = root.join("-work-app/a.jsonl");
    write(
        &file,
        &[
            skill_line("s1", "a"),
            "not json at all".into(),
            String::new(),
            "{\"cut\": ".into(),
        ],
    );
    let huge = format!(
        "{{\"pad\":\"{}\"}}",
        "x".repeat(super::index::MAX_LINE + 10)
    );
    append(&file, &format!("{huge}\n{}\n", skill_line("s1", "b")));
    let report = index.index(&root, |_| {}).expect("index");
    assert_eq!(report.failed_lines, 3);
    assert_eq!(report.new_uses, 2);
}

#[test]
fn a_missing_root_is_an_empty_index_and_progress_is_reported() {
    let (dir, index) = setup();
    let report = index
        .index(&dir.path().join("nope"), |_| {})
        .expect("index");
    assert_eq!(report, super::IndexReport::default());

    let root = logs(&dir);
    write(&root.join("-work-app/a.jsonl"), &[skill_line("s1", "a")]);
    write(&root.join("b.jsonl"), &[skill_line("s2", "b")]);
    let mut seen: Vec<IndexProgress> = Vec::new();
    index.index(&root, |p| seen.push(p)).expect("index");
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].files_done, 0);
    assert_eq!(
        seen[2],
        IndexProgress {
            files_done: 2,
            files_total: 2,
            new_uses: 2
        }
    );
}

#[test]
fn one_hundred_megabytes_index_in_under_ten_seconds() {
    let (dir, index) = setup();
    let root = logs(&dir);
    let filler = user(
        "s0",
        TS,
        "/work/app",
        &"a plain message with no tool calls ".repeat(30),
    );
    let tool_line = skill_line("s1", "webperf");
    let mut chunk = String::new();
    for i in 0..1000 {
        chunk.push_str(if i % 10 == 0 { &tool_line } else { &filler });
        chunk.push('\n');
    }
    let target = 100 * 1024 * 1024;
    let mut written = 0;
    let mut n = 0;
    while written < target {
        let mut f = fs::File::create(root.join(format!("-work-app/f{n}.jsonl"))).expect("file");
        for _ in 0..20 {
            f.write_all(chunk.as_bytes()).expect("write");
            written += chunk.len();
        }
        n += 1;
    }
    let start = crate::test_support::Stopwatch::start();
    let report = index.index(&root, |_| {}).expect("index");
    let elapsed = start.elapsed();
    assert!(report.files >= 4);
    assert!(
        elapsed < crate::test_support::budget(std::time::Duration::from_secs(10)),
        "took {elapsed:?}"
    );
}
