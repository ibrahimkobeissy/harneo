//! Subagents: `.claude/agents/**/*.md` (or `agents/` at a pack root). `name` and `description`
//! are required; a subfolder does not change a subagent's identity.

use std::path::Path;

use serde_json::Value;

use super::rule::{head, scan_markdown, stem, string_list};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, Reference, ScanReport};

const AGENT_DIRS: [&str; 2] = ["agents", ".claude/agents"];

/// The names a subagent points at: `skills` (a list of skill names) and `mcpServers` (strings
/// name configured servers; inline objects define a server in place and are not references).
fn references(metadata: &serde_json::Map<String, Value>) -> Vec<Reference> {
    let mut out = Vec::new();
    if let Some(skills) = metadata.get("skills").and_then(string_list) {
        out.extend(skills.into_iter().map(|target| Reference {
            kind: ArtifactKind::Skill,
            target,
            resolved: false,
        }));
    }
    if let Some(Value::Array(servers)) = metadata.get("mcpServers") {
        out.extend(servers.iter().filter_map(|s| match s {
            Value::String(t) if !t.trim().is_empty() => Some(Reference {
                kind: ArtifactKind::Mcp,
                target: t.clone(),
                resolved: false,
            }),
            _ => None,
        }));
    }
    out
}

/// Pure: reads one subagent.
pub(crate) fn parse(_base: &Path, rel: &Path, contents: &str) -> Artifact {
    let mut h = head(rel, contents, "agent.invalid-frontmatter");
    if h.valid {
        if h.name.is_none() {
            h.diagnostics.push(Diagnostic::error(
                "agent.missing-name",
                rel,
                "a subagent needs a `name`; Claude Code skips the file without one",
            ));
        }
        if h.description.is_none() {
            h.diagnostics.push(Diagnostic::error(
                "agent.missing-description",
                rel,
                "a subagent needs a `description`; Claude uses it to decide when to delegate",
            ));
        }
    }
    let references = references(&h.metadata);
    Artifact {
        kind: ArtifactKind::Agent,
        name: h.name.unwrap_or_else(|| stem(rel)),
        description: h.description,
        path: rel_string(rel),
        files: vec![rel_string(rel)],
        metadata: h.metadata,
        references,
        diagnostics: h.diagnostics,
        digest: None,
    }
}

/// Finds the subagents under `agents/` and `.claude/agents/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    scan_markdown(walker, &AGENT_DIRS, parse, report);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(contents: &str) -> Artifact {
        parse(
            Path::new(".claude/agents"),
            Path::new(".claude/agents/team/rev.md"),
            contents,
        )
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    const FULL: &str = "---\nname: browser-tester\ndescription: Tests in a browser\ntools: Read, Grep\nmodel: sonnet\nskills:\n  - web-testing\nmcpServers:\n  - playwright:\n      type: stdio\n      command: npx\n  - github\n---\nbody";

    #[test]
    fn reads_fields_and_references() {
        let a = agent(FULL);
        assert_eq!(a.name, "browser-tester");
        assert_eq!(a.description.as_deref(), Some("Tests in a browser"));
        assert_eq!(a.metadata["model"], "sonnet");
        assert_eq!(a.metadata["tools"], "Read, Grep");
        assert!(a.diagnostics.is_empty());
        let targets: Vec<(ArtifactKind, &str, bool)> = a
            .references
            .iter()
            .map(|r| (r.kind, r.target.as_str(), r.resolved))
            .collect();
        assert_eq!(
            targets,
            [
                (ArtifactKind::Skill, "web-testing", false),
                (ArtifactKind::Mcp, "github", false)
            ]
        );
    }

    #[test]
    fn missing_name_triggers_and_present_does_not() {
        let a = agent("---\ndescription: d\n---\n");
        assert_eq!(codes(&a), ["agent.missing-name"]);
        assert_eq!(a.name, "rev");
        assert!(
            !codes(&agent("---\nname: nm\ndescription: d\n---\n")).contains(&"agent.missing-name")
        );
    }

    #[test]
    fn missing_description_triggers_and_present_does_not() {
        assert_eq!(
            codes(&agent("---\nname: nm\n---\n")),
            ["agent.missing-description"]
        );
        assert_eq!(
            codes(&agent("---\nname: nm\ndescription: \" \"\n---\n")),
            ["agent.missing-description"]
        );
        assert!(codes(&agent("---\nname: nm\ndescription: d\n---\n")).is_empty());
    }

    #[test]
    fn invalid_frontmatter_triggers_alone_and_valid_does_not() {
        assert_eq!(
            codes(&agent("---\nname: [oops\n---\n")),
            ["agent.invalid-frontmatter"]
        );
        assert!(!codes(&agent(FULL)).contains(&"agent.invalid-frontmatter"));
    }

    #[test]
    fn malformed_reference_fields_yield_no_references() {
        let a = agent("---\nname: nm\ndescription: d\nskills: oops\nmcpServers: 3\n---\n");
        assert!(a.references.is_empty());
    }
}
