//! Where an import comes from.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ids;
use crate::error::{Error, Result};

/// A place artifacts can be imported from. Git joins as another variant, so match on it with a
/// wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
#[non_exhaustive]
pub enum Source {
    /// A folder on this machine (`~/.claude`, a project, a clone).
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Local {
        /// The folder, as the user gave it.
        #[ts(type = "string")]
        path: PathBuf,
    },
    /// A git repository, shallow-cloned for the import. Build it with [`Source::parse_git`].
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Git {
        /// The clone URL.
        url: String,
        /// Branch or tag. `None` means the default branch; a preview fills in the resolved name.
        branch: Option<String>,
        /// Folder inside the repository to scan; `None` is the root.
        subfolder: Option<String>,
    },
}

impl Source {
    /// The source a stored record was imported from.
    pub(crate) fn from_record(record: &super::records::SourceRecord) -> Result<Self> {
        Ok(if record.kind == "git" {
            Self::Git {
                url: record.location.clone(),
                branch: record.branch.clone(),
                subfolder: record.subfolder.clone(),
            }
        } else {
            Self::Local {
                path: PathBuf::from(&record.location),
            }
        })
    }

    /// The stable key the ids are derived from (see [`ids`]).
    pub(crate) fn identity(&self) -> Result<String> {
        match self {
            Self::Local { path } => ids::local_source_key(path),
            Self::Git {
                url,
                branch,
                subfolder,
            } => Ok(ids::git_source_key(
                url,
                branch.as_deref().unwrap_or(""),
                subfolder.as_deref().unwrap_or(""),
            )),
        }
    }

    /// Reads a git URL the user typed or pasted. Accepted: `github.com/owner/repo`,
    /// `https://host/owner/repo(.git)`, `git@host:owner/repo.git`, `ssh://`, `git://`, `file://`,
    /// and GitHub/GitLab browser URLs `…/tree/<branch>/<subfolder>` (GitLab `…/-/tree/…`).
    /// In a browser URL the branch is the first segment after `tree`, so a branch name containing
    /// a slash needs the explicit `branch` argument. Explicit `branch` and `subfolder` override
    /// the URL's.
    pub fn parse_git(input: &str, branch: Option<&str>, subfolder: Option<&str>) -> Result<Self> {
        let input = input.trim();
        let invalid = || Error::InvalidGitUrl {
            input: redact(input),
        };
        if input.is_empty()
            || input.starts_with('-')
            || input.contains("::")
            || input.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(invalid());
        }
        let (url, url_branch, url_subfolder) = if let Some((scheme, _)) = input.split_once("://") {
            match scheme.to_ascii_lowercase().as_str() {
                "http" | "https" => split_tree(input),
                "ssh" | "git" | "file" => (input.to_owned(), None, None),
                _ => return Err(invalid()),
            }
        } else if is_scp_like(input) {
            (input.to_owned(), None, None)
        } else if is_bare_host_path(input) {
            split_tree(&format!("https://{input}"))
        } else {
            return Err(invalid());
        };
        let url = url.trim_end_matches('/').to_owned();
        let clean = |value: Option<&str>| {
            value
                .map(|v| v.trim().trim_matches('/'))
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let branch = clean(branch).or_else(|| clean(url_branch.as_deref()));
        if branch.as_deref().is_some_and(|b| b.starts_with('-')) {
            return Err(invalid());
        }
        Ok(Self::Git {
            url,
            branch,
            subfolder: clean(subfolder).or_else(|| clean(url_subfolder.as_deref())),
        })
    }
}

/// `[user@]host:path`, the scp spelling git accepts.
fn is_scp_like(input: &str) -> bool {
    match input.split_once(':') {
        Some((host, path)) => {
            !host.is_empty() && !host.contains('/') && !path.is_empty() && !path.starts_with('/')
        }
        None => false,
    }
}

/// `github.com/owner/repo`: a dotted host, then a path.
fn is_bare_host_path(input: &str) -> bool {
    match input.split_once('/') {
        Some((host, path)) => host.contains('.') && !path.is_empty() && !input.contains(':'),
        None => false,
    }
}

/// Splits `https://host/owner/repo/tree/<branch>/<sub…>` into the repository URL, branch and
/// subfolder. URLs without a `tree` part come back whole.
fn split_tree(url: &str) -> (String, Option<String>, Option<String>) {
    for marker in ["/-/tree/", "/tree/"] {
        if let Some(at) = url.find(marker) {
            let rest = url[at + marker.len()..].trim_matches('/');
            let (branch, sub) = match rest.split_once('/') {
                Some((b, s)) => (b, Some(s.to_owned())),
                None => (rest, None),
            };
            return (
                url[..at].to_owned(),
                Some(branch.to_owned()).filter(|b| !b.is_empty()),
                sub,
            );
        }
    }
    (url.to_owned(), None, None)
}

/// Removes credentials from every URL in `text`: any `user@` of an http(s) URL, and the
/// `user:password@` of other schemes. Safe for error messages and stored locations.
pub(crate) fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        let scheme = head[..at].to_ascii_lowercase();
        let web = scheme.ends_with("http") || scheme.ends_with("https");
        match authority.rfind('@') {
            Some(i) if web || authority[..i].contains(':') => out.push_str(&authority[i + 1..]),
            _ => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(url: &str, branch: Option<&str>, sub: Option<&str>) -> Source {
        Source::Git {
            url: url.to_owned(),
            branch: branch.map(str::to_owned),
            subfolder: sub.map(str::to_owned),
        }
    }

    fn parse(input: &str) -> Result<Source> {
        Source::parse_git(input, None, None)
    }

    #[test]
    fn bare_github_path_gets_https() -> Result<()> {
        let want = git("https://github.com/o/r", None, None);
        assert_eq!(parse("github.com/o/r")?, want);
        assert_eq!(parse("  https://github.com/o/r/ ")?, want);
        Ok(())
    }

    #[test]
    fn full_urls_are_kept_as_given() -> Result<()> {
        assert_eq!(
            parse("https://example.org/o/r.git")?,
            git("https://example.org/o/r.git", None, None)
        );
        assert_eq!(
            parse("git@github.com:o/r.git")?,
            git("git@github.com:o/r.git", None, None)
        );
        assert_eq!(
            parse("ssh://git@host/o/r.git")?,
            git("ssh://git@host/o/r.git", None, None)
        );
        assert_eq!(
            parse("file:///tmp/repo")?,
            git("file:///tmp/repo", None, None)
        );
        Ok(())
    }

    #[test]
    fn tree_urls_give_branch_and_subfolder() -> Result<()> {
        assert_eq!(
            parse("https://github.com/o/r/tree/dev")?,
            git("https://github.com/o/r", Some("dev"), None)
        );
        assert_eq!(
            parse("github.com/o/r/tree/dev/skills/notes/")?,
            git("https://github.com/o/r", Some("dev"), Some("skills/notes"))
        );
        assert_eq!(
            parse("https://gitlab.com/g/s/r/-/tree/main/.claude")?,
            git("https://gitlab.com/g/s/r", Some("main"), Some(".claude"))
        );
        Ok(())
    }

    #[test]
    fn explicit_branch_and_subfolder_override_the_url() -> Result<()> {
        let got = Source::parse_git("github.com/o/r/tree/dev/a", Some("main"), Some("/b/"))?;
        assert_eq!(got, git("https://github.com/o/r", Some("main"), Some("b")));
        let blank = Source::parse_git("github.com/o/r", Some(" "), Some(""))?;
        assert_eq!(blank, git("https://github.com/o/r", None, None));
        Ok(())
    }

    #[test]
    fn junk_is_refused() {
        for bad in [
            "",
            "owner/repo",
            "just words",
            "ftp://host/r",
            "ext::sh -c id",
            "--upload-pack=x",
            "https://github.com/o/r --x",
        ] {
            assert!(
                matches!(parse(bad), Err(Error::InvalidGitUrl { .. })),
                "{bad}"
            );
        }
        assert!(Source::parse_git("github.com/o/r", Some("-x"), None).is_err());
    }

    #[test]
    fn redact_drops_credentials_only() {
        assert_eq!(
            redact("fatal: https://tok:pw@github.com/o/r failed"),
            "fatal: https://github.com/o/r failed"
        );
        assert_eq!(
            redact("https://tok@github.com/o/r"),
            "https://github.com/o/r"
        );
        assert_eq!(redact("ssh://git@host/o/r"), "ssh://git@host/o/r");
        assert_eq!(redact("ssh://u:p@host/o/r"), "ssh://host/o/r");
        assert_eq!(redact("no urls here"), "no urls here");
    }
}
