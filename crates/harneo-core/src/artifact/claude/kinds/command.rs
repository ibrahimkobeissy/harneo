//! Commands: `.claude/commands/**/*.md` (or `commands/` at a pack root). Claude Code namespaces a
//! command by its subfolders (`frontend/component.md` is `/frontend:component`) and takes the
//! name from the file, never from frontmatter.

use std::path::Path;

use serde_json::Value;

use super::rule::{head, scan_markdown, text};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, Reference, ScanReport};

const COMMAND_DIRS: [&str; 2] = ["commands", ".claude/commands"];
/// Subagent types Claude Code ships; they are not files, so they can never resolve.
const BUILT_IN_AGENTS: [&str; 3] = ["explore", "plan", "general-purpose"];

/// The slash-command name: the path below `base` without `.md`, folders joined with `:`.
fn command_name(base: &Path, rel: &Path) -> String {
    let inner = rel.strip_prefix(base).unwrap_or(rel).with_extension("");
    let parts: Vec<String> = inner
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        "unnamed".into()
    } else {
        parts.join(":")
    }
}

/// Pure: reads one command.
pub(crate) fn parse(base: &Path, rel: &Path, contents: &str) -> Artifact {
    let mut h = head(rel, contents, "command.invalid-frontmatter");
    let references = text(h.metadata.get("agent").cloned())
        .filter(|a| !BUILT_IN_AGENTS.contains(&a.to_ascii_lowercase().as_str()))
        .map(|target| Reference {
            kind: ArtifactKind::Agent,
            target,
            resolved: false,
        })
        .into_iter()
        .collect();
    if let Some(declared) = h.name.take() {
        // Not a command field; kept as written.
        h.metadata.insert("name".into(), Value::String(declared));
    }
    Artifact {
        kind: ArtifactKind::Command,
        name: command_name(base, rel),
        description: h.description,
        path: rel_string(rel),
        files: vec![rel_string(rel)],
        metadata: h.metadata,
        references,
        diagnostics: h.diagnostics,
        digest: None,
        origin: crate::artifact::Origin::default(),
    }
}

/// Finds the commands under `commands/` and `.claude/commands/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    scan_markdown(walker, &COMMAND_DIRS, parse, report);
}

/// `CONTEXT.md` §2: Commands install to `.claude/commands/`, keeping namespacing subfolders.
pub(crate) fn install_path(artifact: &crate::artifact::Artifact) -> std::path::PathBuf {
    use std::path::Path;
    Path::new(".claude/commands").join(super::below(&artifact.path, "commands"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(rel: &str, contents: &str) -> Artifact {
        parse(Path::new(".claude/commands"), Path::new(rel), contents)
    }

    #[test]
    fn name_comes_from_the_path_with_folders_as_namespaces() {
        let a = command(
            ".claude/commands/deploy.md",
            "---\ndescription: Ship\n---\n",
        );
        assert_eq!(a.name, "deploy");
        assert_eq!(a.description.as_deref(), Some("Ship"));
        let n = command(".claude/commands/frontend/component.md", "body");
        assert_eq!(n.name, "frontend:component");
        assert_eq!(n.files, [".claude/commands/frontend/component.md"]);
    }

    #[test]
    fn agent_field_is_an_unresolved_reference_except_for_built_ins() {
        let a = command(
            ".claude/commands/r.md",
            "---\ncontext: fork\nagent: reviewer\n---\n",
        );
        assert_eq!(
            a.references,
            [Reference {
                kind: ArtifactKind::Agent,
                target: "reviewer".into(),
                resolved: false
            }]
        );
        assert_eq!(a.metadata["context"], "fork");
        for built_in in ["Explore", "Plan", "general-purpose"] {
            let b = command(
                ".claude/commands/r.md",
                &format!("---\nagent: {built_in}\n---\n"),
            );
            assert!(b.references.is_empty(), "{built_in}");
        }
        assert!(command(".claude/commands/r.md", "body")
            .references
            .is_empty());
    }

    #[test]
    fn invalid_frontmatter_triggers_and_valid_does_not() {
        let bad = command(".claude/commands/a.md", "---\nx: [oops\n---\n");
        assert_eq!(bad.diagnostics[0].code, "command.invalid-frontmatter");
        assert!(command(".claude/commands/a.md", "---\nx: 1\n---\n")
            .diagnostics
            .is_empty());
    }
}

#[cfg(test)]
mod install_tests {
    use crate::artifact::{Artifact, ArtifactKind};

    fn install(kind: ArtifactKind, path: &str) -> std::path::PathBuf {
        let artifact = Artifact {
            kind,
            name: "x".into(),
            description: None,
            path: path.into(),
            files: Vec::new(),
            metadata: Default::default(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        };
        super::install_path(&artifact)
    }

    #[test]
    fn install_path_matches_the_context_table() {
        assert_eq!(
            install(ArtifactKind::Command, "commands/git/ship.md"),
            std::path::Path::new(".claude/commands/git/ship.md")
        );
        assert_eq!(
            install(ArtifactKind::Command, ".claude/commands/x.md"),
            std::path::Path::new(".claude/commands/x.md")
        );
    }
}
