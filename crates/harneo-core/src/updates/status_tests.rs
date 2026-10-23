//! Pure pieces of the update check: the digest of a stored copy, classification and versions.

use super::tests::{agent, write, TestResult};
use super::*;
use crate::artifact::{AgentAdapter, ClaudeAdapter, ScanOptions};

fn scanned(root: &Path) -> Result<Artifact> {
    let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
    report
        .artifacts
        .into_iter()
        .next()
        .ok_or(Error::NothingFound {
            source_name: "test".into(),
        })
}

fn stored(artifact: &Artifact) -> Stored {
    Stored {
        id: "x".into(),
        source_path: artifact.path.clone(),
        digest: artifact.digest.clone(),
        copy_digest: None,
        artifact: artifact.clone(),
    }
}

#[test]
fn copy_digest_covers_names_and_contents_but_not_order() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "a.md", "one")?;
    write(dir.path(), "sub/b.md", "two")?;
    let first = copy_digest(dir.path())?;
    assert_eq!(first.len(), 64);
    assert_eq!(copy_digest(dir.path())?, first);
    write(dir.path(), "sub/b.md", "three")?;
    assert_ne!(copy_digest(dir.path())?, first);
    write(dir.path(), "sub/b.md", "two")?;
    assert_eq!(copy_digest(dir.path())?, first);
    fs::rename(dir.path().join("sub/b.md"), dir.path().join("sub/c.md"))?;
    assert_ne!(copy_digest(dir.path())?, first, "a rename is an edit");
    Ok(())
}

#[test]
fn classify_flags_only_a_changed_digest_or_a_missing_artifact() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "agents/a.md", &agent("a", "one"))?;
    let before = scanned(dir.path())?;
    let item = stored(&before);
    assert!(matches!(classify(&item, Some(&before)), Verdict::Current));
    assert!(matches!(classify(&item, None), Verdict::Removed));
    write(dir.path(), "agents/a.md", &agent("a", "two"))?;
    let after = scanned(dir.path())?;
    assert!(matches!(
        classify(&item, Some(&after)),
        Verdict::Outdated(_)
    ));
    let undigested = Stored {
        digest: None,
        ..stored(&before)
    };
    assert!(matches!(
        classify(&undigested, Some(&after)),
        Verdict::Current
    ));
    Ok(())
}

#[test]
fn versions_prefer_the_version_field_then_commits_then_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "agents/a.md", &agent("a", "one"))?;
    let plain = scanned(dir.path())?;
    let mut old = plain.clone();
    let mut new = plain.clone();
    old.metadata.insert("version".into(), "1.0".into());
    new.metadata.insert("version".into(), "1.1".into());
    assert_eq!(
        versions(&old, Some(&new), Some("aaaaaaaaaa"), Some("bbbbbbbbbb")),
        (Some("1.0".into()), Some("1.1".into()))
    );
    assert_eq!(
        versions(&plain, Some(&plain), Some("aaaaaaaaaa"), Some("bbbbbbbbbb")),
        (Some("aaaaaaa".into()), Some("bbbbbbb".into()))
    );
    assert_eq!(versions(&plain, Some(&plain), None, None), (None, None));
    assert_eq!(
        versions(&plain, Some(&plain), Some("aaaaaaaaaa"), Some("aaaaaaaaaa")),
        (None, None)
    );
    Ok(())
}
