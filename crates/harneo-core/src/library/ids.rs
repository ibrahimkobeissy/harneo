//! Artifact and source ids. All are the first 16 hex characters of a SHA-256, so they are
//! stable: re-importing the same artifact from the same source gives the same id.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Separator that cannot appear in a path or a URL, so two keys never blur together.
const SEP: u8 = 0;

/// The key of a local source: its canonical absolute path.
pub fn local_source_key(path: &Path) -> Result<String> {
    let canonical = path.canonicalize().map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(format!("local:{}", canonical.display()))
}

/// The key of a git source: the normalised URL, the branch and the subfolder.
pub fn git_source_key(url: &str, branch: &str, subfolder: &str) -> String {
    format!(
        "git:{}\0{}\0{}",
        normalize_git_url(url),
        branch.trim(),
        subfolder.trim().trim_matches('/')
    )
}

/// The id of a source, from its key.
pub fn source_id(source_key: &str) -> String {
    short_hash(&[source_key.as_bytes()])
}

/// The id of an artifact: SHA-256 over the source key and the artifact's path in the source.
pub fn artifact_id(source_key: &str, path_in_source: &str) -> String {
    short_hash(&[source_key.as_bytes(), path_in_source.as_bytes()])
}

fn short_hash(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            hasher.update([SEP]);
        }
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Makes the common spellings of one repository equal: `https://github.com/a/b.git`,
/// `https://GitHub.com/a/b/` and `git@github.com:a/b` all become `github.com/a/b`.
/// The path keeps its case, since hosts may be case-sensitive there.
fn normalize_git_url(url: &str) -> String {
    let url = url.trim();
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    // Drop credentials (`user@`) in front of the host.
    let rest = match rest.split_once('@') {
        Some((_, after)) if !after.contains('@') => after,
        _ => rest,
    };
    // scp-style `host:owner/repo` becomes `host/owner/repo`.
    let rest = if url.contains("://") {
        rest.to_string()
    } else {
        rest.replacen(':', "/", 1)
    };
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    match rest.split_once('/') {
        Some((host, path)) => format!("{}/{}", host.to_lowercase(), path.trim_matches('/')),
        None => rest.to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_16_lowercase_hex_characters() {
        let id = artifact_id("local:/a", "agents/x.md");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    }

    #[test]
    fn artifact_id_is_stable_and_differs_by_source_and_path() {
        let id = artifact_id("local:/a", "agents/x.md");
        assert_eq!(id, artifact_id("local:/a", "agents/x.md"));
        assert_ne!(id, artifact_id("local:/b", "agents/x.md"));
        assert_ne!(id, artifact_id("local:/a", "agents/y.md"));
    }

    #[test]
    fn key_and_path_cannot_blur_together() {
        assert_ne!(artifact_id("ab", "c"), artifact_id("a", "bc"));
    }

    #[test]
    fn source_id_differs_by_key() {
        assert_eq!(source_id("local:/a"), source_id("local:/a"));
        assert_ne!(source_id("local:/a"), source_id("local:/b"));
    }

    #[test]
    fn git_key_ignores_url_spelling() {
        let key = git_source_key("https://github.com/a/b.git", "main", "");
        assert_eq!(key, git_source_key("https://GitHub.com/a/b/", "main", ""));
        assert_eq!(key, git_source_key("git@github.com:a/b", "main", "/"));
        assert_eq!(
            key,
            git_source_key("https://user@github.com/a/b", " main ", "")
        );
    }

    #[test]
    fn git_key_differs_by_repo_branch_and_subfolder() {
        let key = git_source_key("https://github.com/a/b", "main", "");
        assert_ne!(key, git_source_key("https://github.com/a/c", "main", ""));
        assert_ne!(key, git_source_key("https://github.com/A/b", "main", ""));
        assert_ne!(key, git_source_key("https://github.com/a/b", "dev", ""));
        assert_ne!(key, git_source_key("https://github.com/a/b", "main", "x"));
        assert_eq!(
            git_source_key("https://github.com/a/b", "main", "x/y"),
            git_source_key("https://github.com/a/b", "main", "/x/y/")
        );
    }

    #[test]
    fn git_and_local_keys_never_collide() {
        assert_ne!(
            git_source_key("/tmp/x", "main", ""),
            local_source_key(Path::new("/tmp")).expect("key")
        );
    }

    #[test]
    fn bare_host_url_is_lowercased() {
        assert_eq!(normalize_git_url("HOST"), "host");
    }

    #[test]
    fn local_key_is_the_canonical_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("a");
        std::fs::create_dir(&nested).expect("mkdir");
        let direct = local_source_key(&nested).expect("key");
        let dotted = local_source_key(&nested.join("..").join("a")).expect("key");
        assert_eq!(direct, dotted);
        assert!(direct.starts_with("local:/"));
    }

    #[test]
    fn local_key_of_a_missing_path_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(local_source_key(&dir.path().join("nope")).is_err());
    }
}
