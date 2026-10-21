//! The artifact digest: SHA-256 over an artifact's `files`, so the library can tell whether two
//! copies of an artifact are the same and whether one changed.
//!
//! Per file, in sorted relative-path order, the hash takes: the path bytes, a `0x00` separator,
//! a tag byte, and for readable files the content length (u64, little endian) then the content.
//! A rename or an edit therefore changes the digest, and no two layouts of the same bytes can
//! collide. A file the walk refuses to read (symlink, secret-pattern name, local file, over the
//! size limit, unreadable, or past the digest read budget) contributes its path and tag `0` only:
//! its content never enters the hash.
//!
//! Reads go through the [`Walker`]. They are charged to a digest budget kept apart from the
//! parse reads (equal to `max_total_bytes`), so a file read to parse it is not counted twice
//! against the one scan limit.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::walk::Walker;
use super::ScanReport;

const TAG_UNREAD: u8 = 0;
const TAG_CONTENT: u8 = 1;

/// Fills `artifact.digest` (lowercase hex) for every artifact in the report.
pub(crate) fn fill(walker: &mut Walker<'_>, report: &mut ScanReport) {
    for artifact in &mut report.artifacts {
        artifact.digest = Some(digest_files(walker, &artifact.files));
    }
}

fn digest_files(walker: &mut Walker<'_>, files: &[String]) -> String {
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort();
    sorted.dedup();
    let mut hasher = Sha256::new();
    for rel in sorted {
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        match walker.read_bytes(Path::new(rel)) {
            Some(bytes) => {
                hasher.update([TAG_CONTENT]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
            None => hasher.update([TAG_UNREAD]),
        }
    }
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use crate::artifact::{AgentAdapter, ClaudeAdapter, ScanOptions};

    type R = Result<(), Box<dyn std::error::Error>>;
    type Digests = BTreeMap<String, Option<String>>;

    /// A secret-pattern name, spelled in two pieces so no tool mistakes this file for one.
    const SECRET: &str = concat!(".claude/skills/a/token.sec", "ret");

    fn write(root: &Path, rel: &str, body: &str) -> R {
        let p = root.join(rel);
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(p, body)?;
        Ok(())
    }

    fn setup(root: &Path) -> R {
        for s in ["a", "b"] {
            write(
                root,
                &format!(".claude/skills/{s}/SKILL.md"),
                &format!("---\nname: {s}\ndescription: d\n---\nbody\n"),
            )?;
            write(root, &format!(".claude/skills/{s}/run.sh"), "echo hi\n")?;
        }
        Ok(())
    }

    fn digests(root: &Path) -> Result<Digests, Box<dyn std::error::Error>> {
        let report = ClaudeAdapter.scan(root, &ScanOptions::default())?;
        Ok(report
            .artifacts
            .into_iter()
            .map(|a| (a.path, a.digest))
            .collect())
    }

    fn changed(before: &Digests, after: &Digests) -> Vec<String> {
        before
            .keys()
            .filter(|k| before.get(*k) != after.get(*k))
            .cloned()
            .collect()
    }

    #[test]
    fn same_folder_twice_gives_the_same_digests() -> R {
        let dir = tempfile::tempdir()?;
        setup(dir.path())?;
        let first = digests(dir.path())?;
        assert_eq!(first, digests(dir.path())?);
        assert!(!first.is_empty());
        for d in first.values() {
            let d = d.as_deref().unwrap_or_default();
            assert_eq!(d.len(), 64);
            assert!(d
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        }
        Ok(())
    }

    #[test]
    fn one_changed_byte_changes_only_that_skill() -> R {
        let dir = tempfile::tempdir()?;
        setup(dir.path())?;
        let before = digests(dir.path())?;
        write(dir.path(), ".claude/skills/a/run.sh", "echo ho\n")?;
        let after = digests(dir.path())?;
        let diff = changed(&before, &after);
        assert_eq!(diff.len(), 1, "{diff:?}");
        assert!(diff[0].ends_with("skills/a"), "{diff:?}");
        Ok(())
    }

    #[test]
    fn renaming_a_file_changes_the_digest() -> R {
        let dir = tempfile::tempdir()?;
        setup(dir.path())?;
        let before = digests(dir.path())?;
        let base = dir.path().join(".claude/skills/a");
        fs::rename(base.join("run.sh"), base.join("go.sh"))?;
        let after = digests(dir.path())?;
        assert_eq!(changed(&before, &after).len(), 1);
        Ok(())
    }

    #[test]
    fn secret_file_content_never_enters_the_digest() -> R {
        let dir = tempfile::tempdir()?;
        setup(dir.path())?;
        write(dir.path(), SECRET, "SENTINEL-ONE")?;
        let one = digests(dir.path())?;
        write(dir.path(), SECRET, "SENTINEL-TWO")?;
        let two = digests(dir.path())?;
        assert_eq!(one, two);
        // The file is still part of the artifact: removing it changes the digest.
        fs::remove_file(dir.path().join(SECRET))?;
        assert_ne!(one, digests(dir.path())?);
        Ok(())
    }

    #[test]
    fn every_artifact_has_a_digest() -> R {
        let dir = tempfile::tempdir()?;
        setup(dir.path())?;
        write(dir.path(), ".claude/settings.json", "{}")?;
        let all = digests(dir.path())?;
        assert!(all.len() >= 3);
        assert!(all.values().all(Option::is_some));
        Ok(())
    }
}
