//! The live rescan watcher: tells the app when the library store or a registered project's
//! harness files change on disk. It only reads; what to do about a change is the caller's.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, Result};

/// Quiet time after the last change before the callback fires.
pub const DEBOUNCE: Duration = Duration::from_millis(500);
/// A steady stream of changes still fires this long after the first one.
const MAX_WAIT: Duration = Duration::from_secs(2);
/// What of a project the watcher looks at, relative to its root.
const CLAUDE_DIR: &str = ".claude";
const HARNESS_FILES: [&str; 3] = ["CLAUDE.md", ".mcp.json", "HARNESS.md"];

/// Where a watched path belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "scope", rename_all = "lowercase")]
#[ts(export)]
pub enum ChangeScope {
    /// The library store changed.
    Library,
    /// A registered project's harness files changed.
    Project {
        /// The project's registry id.
        #[ts(type = "number")]
        id: i64,
    },
}

/// One thing to watch: the library artifacts folder, or a project's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchPath {
    pub scope: ChangeScope,
    pub path: PathBuf,
}

enum Msg {
    Event(notify::Result<Event>),
    SetPaths(Vec<WatchPath>),
    Stop,
}

/// Watches until dropped. Changes arrive debounced, one callback per scope per burst.
pub struct Watcher {
    tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
}

fn io_error(path: &str, err: notify::Error) -> Error {
    Error::Io {
        path: path.to_string(),
        source: std::io::Error::other(err),
    }
}

impl Watcher {
    /// Starts watching `paths` and calls `on_change` from a background thread.
    pub fn start(
        paths: Vec<WatchPath>,
        on_change: impl Fn(ChangeScope) + Send + 'static,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let sender = tx.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            // A closed channel means the watcher is being dropped.
            let _ = sender.send(Msg::Event(event));
        })
        .map_err(|e| io_error("file watcher", e))?;
        let mut state = State::default();
        state.apply(&mut watcher, paths);
        let worker = std::thread::Builder::new()
            .name("harneo-watch".into())
            .spawn(move || run(rx, watcher, state, on_change))
            .map_err(|source| Error::Io {
                path: "file watcher".into(),
                source,
            })?;
        Ok(Self {
            tx,
            worker: Some(worker),
        })
    }

    /// Replaces what is watched (the registry changed); pending changes still fire.
    pub fn set_paths(&self, paths: Vec<WatchPath>) {
        // A closed channel means the worker has stopped; there is nothing left to retarget.
        let _ = self.tx.send(Msg::SetPaths(paths));
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The targets in force and the folders registered with the OS for them.
#[derive(Default)]
struct State {
    targets: Vec<WatchPath>,
    registered: Vec<PathBuf>,
}

impl State {
    fn apply(&mut self, watcher: &mut RecommendedWatcher, paths: Vec<WatchPath>) {
        for path in self.registered.drain(..) {
            let _ = watcher.unwatch(&path);
        }
        for target in &paths {
            match target.scope {
                ChangeScope::Library => {
                    self.register(watcher, &target.path, RecursiveMode::Recursive);
                }
                ChangeScope::Project { .. } => {
                    self.register(watcher, &target.path, RecursiveMode::NonRecursive);
                    self.register(
                        watcher,
                        &target.path.join(CLAUDE_DIR),
                        RecursiveMode::Recursive,
                    );
                }
            }
        }
        self.targets = paths;
    }

    /// A folder that is missing is skipped: a project that vanished shows as Missing elsewhere.
    fn register(&mut self, watcher: &mut RecommendedWatcher, path: &Path, mode: RecursiveMode) {
        if watcher.watch(path, mode).is_ok() {
            self.registered.push(path.to_path_buf());
        }
    }

    /// The scope an event on `path` belongs to, if it is one the watcher cares about.
    fn scope_of(&self, path: &Path, watcher: &mut RecommendedWatcher) -> Option<ChangeScope> {
        for target in &self.targets {
            let Ok(rest) = path.strip_prefix(&target.path) else {
                continue;
            };
            if matches!(target.scope, ChangeScope::Library) {
                return Some(target.scope);
            }
            let first = rest.components().next()?.as_os_str();
            if first == CLAUDE_DIR {
                if rest.as_os_str() == CLAUDE_DIR {
                    // `.claude/` appeared (or came back): watch inside it too.
                    let _ = watcher.watch(path, RecursiveMode::Recursive);
                }
                return Some(target.scope);
            }
            if HARNESS_FILES.iter().any(|name| first == *name) {
                return Some(target.scope);
            }
        }
        None
    }
}

fn run(
    rx: mpsc::Receiver<Msg>,
    mut watcher: RecommendedWatcher,
    mut state: State,
    on_change: impl Fn(ChangeScope),
) {
    let mut pending: BTreeSet<ChangeScope> = BTreeSet::new();
    let mut first_at = Instant::now();
    let mut last_at = first_at;
    loop {
        let message = if pending.is_empty() {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            let due = (last_at + DEBOUNCE).min(first_at + MAX_WAIT);
            rx.recv_timeout(due.saturating_duration_since(Instant::now()))
        };
        match message {
            Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Msg::SetPaths(paths)) => state.apply(&mut watcher, paths),
            Ok(Msg::Event(Ok(event))) => {
                if matches!(event.kind, EventKind::Access(_)) {
                    continue;
                }
                for path in &event.paths {
                    if let Some(scope) = state.scope_of(path, &mut watcher) {
                        let now = Instant::now();
                        if pending.is_empty() {
                            first_at = now;
                        }
                        last_at = now;
                        pending.insert(scope);
                    }
                }
            }
            // An error from the OS watcher carries no path; the next change still arrives.
            Ok(Msg::Event(Err(_))) => {}
            Err(RecvTimeoutError::Timeout) => {
                for scope in std::mem::take(&mut pending) {
                    on_change(scope);
                }
            }
        }
    }
}

impl crate::library::Library {
    /// What the live rescan watches: the library store and every registered project.
    pub fn watch_paths(&self) -> Result<Vec<WatchPath>> {
        let mut paths = vec![WatchPath {
            scope: ChangeScope::Library,
            path: self.store.artifacts_dir(),
        }];
        for project in self.projects()? {
            paths.push(WatchPath {
                scope: ChangeScope::Project { id: project.id },
                path: PathBuf::from(project.path),
            });
        }
        Ok(paths)
    }
}
