use std::path::Path;

use super::remote::{fetch, locate, Failure, Session};
use crate::library::git::GitConfig;
use crate::library::SourceRecord;

fn record(kind: &str, location: &str, branch: Option<&str>) -> SourceRecord {
    SourceRecord {
        id: "0123456789abcdef".to_owned(),
        kind: kind.to_owned(),
        location: location.to_owned(),
        branch: branch.map(str::to_owned),
        subfolder: None,
        commit_sha: None,
    }
}

/// A git program that cannot run: if a refused source reached git, the failure would say so.
fn config() -> GitConfig {
    GitConfig {
        program: "harneo-no-such-git".into(),
        ..GitConfig::default()
    }
}

fn why(source: &SourceRecord) -> String {
    let session = Session::default();
    let root = Path::new("/nonexistent-harneo-root");
    let fetched = fetch(&config(), &session, root, source, None);
    let located = locate(&config(), &session, root, source);
    match (fetched, located) {
        (Err(Failure::Unreachable(a)), Err(Failure::Unreachable(b))) => {
            assert_eq!(a, b);
            a
        }
        other => format!("not refused: {other:?}"),
    }
}

#[test]
fn invalid_stored_git_sources_never_reach_git() {
    for source in [
        record("git", "--upload-pack=touch /tmp/x", None),
        record("git", "ext::sh -c touch% /tmp/x", None),
        record("git", "https://github.com/a/b", Some("--output=/tmp/x")),
        record("git", "https://github.com/a/b", Some("a b")),
        record("git", "https://github.com/a/b", Some("")),
        record("git", "", None),
        record("other", "https://github.com/a/b", None),
    ] {
        let reason = why(&source);
        assert!(
            reason.contains("valid git address") || reason.contains("not one Harneo can read"),
            "{source:?}: {reason}"
        );
    }
}

#[test]
fn a_local_source_must_be_an_absolute_path() {
    let reason = why(&record("local", "relative/dir", None));
    assert!(reason.contains("no source linked"), "{reason}");
    let reason = why(&record("local", "profile file Heavy", None));
    assert!(reason.contains("no source linked"), "{reason}");
}
