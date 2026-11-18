//! What a project's profile installed and why, read from the database: the profile's pinned
//! version and each artifact's own description. Nothing is read from the project's files
//! (Harneo writes no `HARNESS.md`, user decision 2026-11-18).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::ArtifactKind;
use crate::error::Result;
use crate::library::{ArtifactId, Library};

/// One artifact of a project's profile, with the reason it is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ProjectArtifact {
    pub id: ArtifactId,
    pub name: String,
    pub kind: ArtifactKind,
    /// The first sentence of the artifact's description; empty when it has none.
    pub why: String,
}

/// The first sentence of a description, on one line.
fn first_sentence(description: &str) -> String {
    let flat = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let end = flat
        .char_indices()
        .find(|&(i, c)| c == '.' && flat[i + 1..].chars().next().is_none_or(char::is_whitespace))
        .map_or(flat.len(), |(i, _)| i);
    flat[..end].to_owned()
}

impl Library {
    /// The artifacts the project's pinned profile version installs, each with its why. A removed
    /// project has none.
    pub fn project_artifacts(&self, id: i64) -> Result<Vec<ProjectArtifact>> {
        let project = self.project(id)?;
        if project.removed_at.is_some() {
            return Ok(Vec::new());
        }
        let view = self.profile(project.profile_id, Some(project.profile_version))?;
        let mut out = Vec::new();
        for item in self.resolve(&view.spec)?.artifacts {
            let why = self
                .detail(&item.id)?
                .artifact
                .description
                .as_deref()
                .map(first_sentence)
                .unwrap_or_default();
            out.push(ProjectArtifact {
                id: item.id,
                name: item.name,
                kind: item.kind,
                why,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::first_sentence;
    use crate::artifact::ArtifactKind;
    use crate::install::fixture::{fx, TestResult};

    #[test]
    fn the_first_sentence_stops_at_a_full_stop_followed_by_space() {
        assert_eq!(first_sentence("Does a thing. Then more."), "Does a thing");
        assert_eq!(first_sentence("v1.2 is\n  fine"), "v1.2 is fine");
        assert_eq!(first_sentence(""), "");
    }

    #[test]
    fn a_project_lists_its_artifacts_and_why_from_the_database() -> TestResult {
        let f = fx()?;
        let dir = tempfile::tempdir()?;
        let plan = f.library.plan_scaffold(dir.path(), f.profile, None)?;
        let id = f.library.apply(&plan)?.project_id;
        let artifacts = f.library.project_artifacts(id)?;
        let skill = artifacts
            .iter()
            .find(|a| a.name == "s")
            .ok_or("skill missing")?;
        assert_eq!(
            (skill.kind, skill.why.as_str()),
            (ArtifactKind::Skill, "Does a thing")
        );
        let agent = artifacts
            .iter()
            .find(|a| a.name == "rev")
            .ok_or("agent missing")?;
        assert_eq!(agent.why, "Reviews code");
        // No HARNESS.md is needed for any of it.
        assert!(!dir.path().join("HARNESS.md").exists());

        let removal = f.library.plan_removal(id)?;
        f.library.apply(&removal)?;
        assert!(f.library.project_artifacts(id)?.is_empty());
        Ok(())
    }
}
