//! The marketplace origin of a Plugin found through a marketplace in an imported source.
//!
//! A git or folder import stores every artifact as `Own`. A Plugin that was listed by a
//! marketplace is enabled from `settings.json` instead, which needs the marketplace's name and
//! the GitHub repo it lives in: the source's own repo.

use std::fs;
use std::path::Path;

use crate::artifact::{github_repo, Artifact, ArtifactKind, Origin};

/// The origin a Plugin should have given where it was imported from (`kind` is the source's
/// "git" or "local", `location` its URL or folder), or `None` to keep what it has. Only an `Own`
/// Plugin with a `marketplace` in its metadata and a GitHub repo for its source changes.
pub(crate) fn derive_plugin_origin(
    artifact: &Artifact,
    kind: &str,
    location: &str,
) -> Option<Origin> {
    if artifact.kind != ArtifactKind::Plugin || artifact.origin != Origin::Own {
        return None;
    }
    let name = artifact.metadata.get("marketplace")?.as_str()?;
    let repo = if kind == "git" {
        github_repo(location)?
    } else {
        local_repo(Path::new(location))?
    };
    let official = repo
        .split('/')
        .next()
        .is_some_and(|org| org.eq_ignore_ascii_case("anthropics"));
    Some(Origin::Marketplace {
        name: name.to_owned(),
        repo: Some(repo),
        official,
    })
}

/// The GitHub `owner/repo` of the `origin` remote of the git checkout holding `folder`.
fn local_repo(folder: &Path) -> Option<String> {
    let config = folder
        .ancestors()
        .map(|dir| dir.join(".git").join("config"))
        .find(|c| c.is_file())?;
    remote_url(&fs::read_to_string(config).ok()?).and_then(|url| github_repo(&url))
}

/// The `url` of `[remote "origin"]` in a git config file's text.
fn remote_url(config: &str) -> Option<String> {
    let mut in_origin = false;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            in_origin = line.replace(' ', "") == "[remote\"origin\"]";
        } else if in_origin {
            if let Some(("url", value)) = line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                return Some(value.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(market: Option<&str>) -> Artifact {
        let mut metadata = serde_json::Map::new();
        if let Some(m) = market {
            metadata.insert("marketplace".into(), m.into());
        }
        Artifact {
            kind: ArtifactKind::Plugin,
            name: "p".into(),
            description: None,
            path: "plugins/p".into(),
            files: Vec::new(),
            metadata,
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: Origin::Own,
        }
    }

    fn derived(a: &Artifact, kind: &str, loc: &str) -> Option<(String, Option<String>, bool)> {
        match derive_plugin_origin(a, kind, loc)? {
            Origin::Marketplace {
                name,
                repo,
                official,
            } => Some((name, repo, official)),
            _ => None,
        }
    }

    #[test]
    fn git_source_gives_the_repo() {
        let got = derived(
            &plugin(Some("mods")),
            "git",
            "https://github.com/Me/Claude-Mods.git",
        );
        assert_eq!(
            got,
            Some(("mods".into(), Some("me/claude-mods".into()), false))
        );
        let got = derived(&plugin(Some("o")), "git", "git@github.com:anthropics/x.git");
        assert_eq!(got.map(|g| g.2), Some(true));
    }

    #[test]
    fn no_marketplace_or_no_github_keeps_own() {
        assert!(derived(&plugin(None), "git", "https://github.com/a/b").is_none());
        assert!(derived(&plugin(Some("m")), "git", "https://gitlab.com/a/b").is_none());
        assert!(derived(&plugin(Some("m")), "local", "/nonexistent/folder").is_none());
    }

    #[test]
    fn local_folder_uses_its_origin_remote() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join(".git"))?;
        fs::write(
            dir.path().join(".git/config"),
            "[core]\n\tbare = false\n[remote \"upstream\"]\n\turl = https://github.com/x/y\n[remote \"origin\"]\n\turl = git@github.com:me/mods.git\n",
        )?;
        let got = derived(
            &plugin(Some("m")),
            "local",
            &dir.path().display().to_string(),
        );
        assert_eq!(got.and_then(|g| g.1), Some("me/mods".into()));
        Ok(())
    }
}
