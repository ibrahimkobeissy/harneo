use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::watch::{ChangeScope, WatchPath, Watcher};

fn project(id: i64, root: &Path) -> WatchPath {
    WatchPath {
        scope: ChangeScope::Project { id },
        path: root.to_path_buf(),
    }
}

fn library(path: &Path) -> WatchPath {
    WatchPath {
        scope: ChangeScope::Library,
        path: path.to_path_buf(),
    }
}

fn start(paths: Vec<WatchPath>) -> (Watcher, Receiver<ChangeScope>) {
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(paths, move |scope| {
        let _ = tx.send(scope);
    })
    .expect("start");
    // Let the OS watches settle before the test writes.
    std::thread::sleep(Duration::from_millis(200));
    (watcher, rx)
}

const WITHIN: Duration = Duration::from_secs(3);
const QUIET: Duration = Duration::from_millis(1200);

#[test]
fn a_write_in_a_project_harness_fires_one_debounced_event() {
    let dir = tempfile::tempdir().expect("dir");
    fs::create_dir_all(dir.path().join(".claude/agents")).expect("mkdir");
    let (_watcher, rx) = start(vec![project(7, dir.path())]);
    for n in 0..3 {
        fs::write(dir.path().join(".claude/agents/a.md"), format!("v{n}")).expect("write");
    }
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("event"),
        ChangeScope::Project { id: 7 }
    );
    assert!(rx.recv_timeout(QUIET).is_err(), "exactly one callback");
}

#[test]
fn harness_files_in_the_root_fire_and_other_files_do_not() {
    let dir = tempfile::tempdir().expect("dir");
    let (_watcher, rx) = start(vec![project(1, dir.path())]);
    fs::write(dir.path().join("README.md"), "x").expect("write");
    fs::write(dir.path().join("src.rs"), "x").expect("write");
    assert!(rx.recv_timeout(QUIET).is_err(), "unwatched files are quiet");
    for name in ["CLAUDE.md", ".mcp.json", "HARNESS.md"] {
        fs::write(dir.path().join(name), "x").expect("write");
        assert_eq!(
            rx.recv_timeout(WITHIN).expect(name),
            ChangeScope::Project { id: 1 }
        );
    }
}

#[test]
fn a_claude_folder_created_later_is_seen_and_watched() {
    let dir = tempfile::tempdir().expect("dir");
    let (_watcher, rx) = start(vec![project(2, dir.path())]);
    fs::create_dir(dir.path().join(".claude")).expect("mkdir");
    assert!(rx.recv_timeout(WITHIN).is_ok(), "creation is seen");
    std::thread::sleep(Duration::from_millis(200));
    fs::write(dir.path().join(".claude/settings.json"), "{}").expect("write");
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("write inside"),
        ChangeScope::Project { id: 2 }
    );
}

#[test]
fn library_folder_changes_fire_library_recursively() {
    let dir = tempfile::tempdir().expect("dir");
    let store = dir.path().join("artifacts");
    fs::create_dir_all(store.join("abc")).expect("mkdir");
    let (_watcher, rx) = start(vec![library(&store)]);
    fs::write(store.join("abc/SKILL.md"), "x").expect("write");
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("event"),
        ChangeScope::Library
    );
}

#[test]
fn one_burst_in_two_places_fires_each_scope_once() {
    let a = tempfile::tempdir().expect("dir");
    let store = tempfile::tempdir().expect("dir");
    let (_watcher, rx) = start(vec![project(3, a.path()), library(store.path())]);
    fs::write(a.path().join("CLAUDE.md"), "x").expect("write");
    fs::write(store.path().join("f"), "x").expect("write");
    let mut got = vec![
        rx.recv_timeout(WITHIN).expect("first"),
        rx.recv_timeout(WITHIN).expect("second"),
    ];
    got.sort();
    assert_eq!(
        got,
        vec![ChangeScope::Library, ChangeScope::Project { id: 3 }]
    );
    assert!(rx.recv_timeout(QUIET).is_err());
}

#[test]
fn set_paths_switches_what_is_watched() {
    let a = tempfile::tempdir().expect("dir");
    let b = tempfile::tempdir().expect("dir");
    let (watcher, rx) = start(vec![project(1, a.path())]);
    watcher.set_paths(vec![project(2, b.path())]);
    std::thread::sleep(Duration::from_millis(300));
    fs::write(a.path().join("CLAUDE.md"), "x").expect("write");
    assert!(
        rx.recv_timeout(QUIET).is_err(),
        "the old project is dropped"
    );
    fs::write(b.path().join("CLAUDE.md"), "x").expect("write");
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("new project"),
        ChangeScope::Project { id: 2 }
    );
}

#[test]
fn a_missing_folder_is_skipped_and_dropping_stops_the_callbacks() {
    let dir = tempfile::tempdir().expect("dir");
    let gone = dir.path().join("gone");
    let (watcher, rx) = start(vec![project(1, &gone), project(2, dir.path())]);
    drop(watcher);
    fs::write(dir.path().join("CLAUDE.md"), "x").expect("write");
    assert!(rx.recv_timeout(QUIET).is_err());
}

#[test]
fn the_scope_serializes_as_the_event_payload() {
    assert_eq!(
        serde_json::to_string(&ChangeScope::Library).expect("json"),
        r#"{"scope":"library"}"#
    );
    assert_eq!(
        serde_json::to_string(&ChangeScope::Project { id: 4 }).expect("json"),
        r#"{"scope":"project","id":4}"#
    );
}

#[test]
fn a_library_lists_its_store_as_a_watch_path() {
    let dir = tempfile::tempdir().expect("dir");
    let library = crate::library::Library::open(dir.path()).expect("open");
    let paths = library.watch_paths().expect("paths");
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].scope, ChangeScope::Library);
    assert!(paths[0].path.is_dir());
    assert!(paths[0].path.starts_with(dir.path()));
}

#[test]
fn deleting_a_watched_project_root_fires_an_event() {
    let parent = tempfile::tempdir().expect("dir");
    let root = parent.path().join("proj");
    fs::create_dir(&root).expect("mkdir");
    let (_watcher, rx) = start(vec![project(9, &root)]);
    fs::remove_dir_all(&root).expect("remove");
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("event"),
        ChangeScope::Project { id: 9 }
    );
}

#[test]
fn a_re_added_claude_watch_is_dropped_by_set_paths() {
    use super::watch::State;
    let dir = tempfile::tempdir().expect("dir");
    let mut watcher = notify::recommended_watcher(|_| {}).expect("watcher");
    let mut state = State::default();
    state.apply(&mut watcher, vec![project(1, dir.path())]);
    let claude = dir.path().join(".claude");
    fs::create_dir(&claude).expect("mkdir");
    assert!(state.scope_of(&claude, &mut watcher).is_some());
    assert!(state.scope_of(&claude, &mut watcher).is_some());
    assert_eq!(
        state.registered.iter().filter(|p| **p == claude).count(),
        1,
        "registered once"
    );
    state.apply(&mut watcher, Vec::new());
    assert!(state.registered.is_empty(), "nothing leaks past set_paths");
}

#[test]
fn a_library_folder_created_later_is_watched_and_reported() {
    let dir = tempfile::tempdir().expect("dir");
    let store = dir.path().join("artifacts");
    let (_watcher, rx) = start(vec![library(&store)]);
    fs::create_dir_all(&store).expect("mkdir");
    assert_eq!(
        rx.recv_timeout(super::watch::RETRY * 3).expect("retried"),
        ChangeScope::Library
    );
    std::thread::sleep(Duration::from_millis(200));
    fs::write(store.join("f"), "x").expect("write");
    assert_eq!(
        rx.recv_timeout(WITHIN).expect("now watched"),
        ChangeScope::Library
    );
}
