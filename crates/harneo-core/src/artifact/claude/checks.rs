//! Cross-artifact checks that need the whole report: reference resolution and name clashes.
//!
//! Reference resolution sets `Reference.resolved` when an artifact of the referenced kind exists
//! in the same report:
//! - Skill, Agent, OutputStyle and Mcp (server name) match the artifact's `name` exactly.
//! - Plugin targets from `enabledPlugins` look like `plugin@marketplace`. The part before `@`
//!   must equal a Plugin's name; when the target has an `@marketplace` part and that Plugin
//!   records a `marketplace` in its metadata, they must match too. A Plugin without a recorded
//!   marketplace matches any, and a target without `@` matches by name only.
//! - Every other kind matches by name.
//!
//! Diagnostic codes (both warnings, attached to each artifact involved, `file` = its own path):
//! `scan.duplicate-name` (two artifacts of one kind share a name, case-sensitive; ClaudeMd and
//! Settings are exempt because they legitimately live in two locations with fixed names) and
//! `scan.skill-command-clash` (on the command; the skill wins in Claude Code).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::artifact::{ArtifactKind, Diagnostic, ScanReport};

/// Resolves references and adds the cross-artifact diagnostics, in the report's order.
pub(crate) fn run(report: &mut ScanReport) {
    let mut names: HashSet<(ArtifactKind, String)> = HashSet::new();
    let mut plugins: HashMap<String, Vec<Option<String>>> = HashMap::new();
    let mut counts: HashMap<(ArtifactKind, String), usize> = HashMap::new();
    for a in &report.artifacts {
        names.insert((a.kind, a.name.clone()));
        *counts.entry((a.kind, a.name.clone())).or_default() += 1;
        if a.kind == ArtifactKind::Plugin {
            let marketplace = crate::plugin::marketplace_of(a).map(str::to_owned);
            plugins.entry(a.name.clone()).or_default().push(marketplace);
        }
    }
    for a in &mut report.artifacts {
        for r in &mut a.references {
            r.resolved = if r.kind == ArtifactKind::Plugin {
                resolve_plugin(&plugins, &r.target)
            } else {
                names.contains(&(r.kind, r.target.clone()))
            };
        }
        let exempt = matches!(a.kind, ArtifactKind::ClaudeMd | ArtifactKind::Settings);
        let file = a.path.clone();
        if !exempt && counts.get(&(a.kind, a.name.clone())).copied().unwrap_or(0) > 1 {
            a.diagnostics.push(Diagnostic::warning(
                "scan.duplicate-name",
                Path::new(&file),
                &format!("another {:?} is also named `{}`", a.kind, a.name),
            ));
        }
        if a.kind == ArtifactKind::Command && names.contains(&(ArtifactKind::Skill, a.name.clone()))
        {
            a.diagnostics.push(Diagnostic::warning(
                "scan.skill-command-clash",
                Path::new(&file),
                &format!(
                    "a skill is also named `{}`; Claude Code uses the skill and ignores this command",
                    a.name
                ),
            ));
        }
    }
}

fn resolve_plugin(plugins: &HashMap<String, Vec<Option<String>>>, target: &str) -> bool {
    let (name, marketplace) = match target.split_once('@') {
        Some((n, m)) => (n, Some(m)),
        None => (target, None),
    };
    let Some(found) = plugins.get(name) else {
        return false;
    };
    found.iter().any(|m| match (marketplace, m) {
        (Some(want), Some(have)) => want == have,
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{Artifact, Reference};

    fn art(kind: ArtifactKind, name: &str, path: &str) -> Artifact {
        Artifact {
            kind,
            name: name.into(),
            description: None,
            path: path.into(),
            files: Vec::new(),
            metadata: serde_json::Map::new(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        }
    }

    fn reference(kind: ArtifactKind, target: &str) -> Reference {
        Reference {
            kind,
            target: target.into(),
            resolved: false,
        }
    }

    fn report(artifacts: Vec<Artifact>) -> ScanReport {
        let mut r = ScanReport {
            artifacts,
            diagnostics: Vec::new(),
            ..ScanReport::default()
        };
        run(&mut r);
        r
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn each_kind_resolves_and_stays_unresolved_when_absent() {
        let kinds = [
            ArtifactKind::Skill,
            ArtifactKind::Agent,
            ArtifactKind::OutputStyle,
            ArtifactKind::Mcp,
        ];
        for kind in kinds {
            let mut holder = art(ArtifactKind::Settings, "settings", "settings.json");
            holder.references = vec![reference(kind, "x"), reference(kind, "missing")];
            let r = report(vec![holder, art(kind, "x", "p/x")]);
            let got: Vec<_> = r.artifacts[0]
                .references
                .iter()
                .map(|r| r.resolved)
                .collect();
            assert_eq!(got, [true, false], "{kind:?}");
        }
    }

    #[test]
    fn a_reference_needs_the_right_kind() {
        let mut holder = art(ArtifactKind::Agent, "a", "agents/a.md");
        holder.references = vec![reference(ArtifactKind::Skill, "x")];
        let r = report(vec![holder, art(ArtifactKind::Agent, "x", "agents/x.md")]);
        assert!(!r.artifacts[0].references[0].resolved);
    }

    #[test]
    fn plugin_references_match_name_and_marketplace() {
        let mut p = art(ArtifactKind::Plugin, "tools", "plugins/tools");
        p.metadata.insert("marketplace".into(), "acme".into());
        let bare = art(ArtifactKind::Plugin, "bare", "plugins/bare");
        let mut holder = art(ArtifactKind::Settings, "settings", "settings.json");
        holder.references = [
            "tools@acme",
            "tools@other",
            "tools",
            "bare@any",
            "nope@acme",
        ]
        .iter()
        .map(|t| reference(ArtifactKind::Plugin, t))
        .collect();
        let r = report(vec![holder, p, bare]);
        let got: Vec<_> = r.artifacts[0]
            .references
            .iter()
            .map(|r| r.resolved)
            .collect();
        assert_eq!(got, [true, false, true, true, false]);
    }

    #[test]
    fn duplicate_names_warn_on_each_and_exempt_claude_md_and_settings() {
        let r = report(vec![
            art(ArtifactKind::Skill, "a", "skills/a"),
            art(ArtifactKind::Skill, "a", ".claude/skills/a"),
            art(ArtifactKind::Skill, "A", "skills/upper"),
            art(ArtifactKind::Agent, "a", "agents/a.md"),
            art(ArtifactKind::ClaudeMd, "CLAUDE.md", "CLAUDE.md"),
            art(ArtifactKind::ClaudeMd, "CLAUDE.md", ".claude/CLAUDE.md"),
            art(ArtifactKind::Settings, "settings", "settings.json"),
            art(ArtifactKind::Settings, "settings", ".claude/settings.json"),
        ]);
        let got: Vec<_> = r.artifacts.iter().map(codes).collect();
        let dup = vec!["scan.duplicate-name"];
        let none: Vec<&str> = Vec::new();
        assert_eq!(
            got,
            [
                dup.clone(),
                dup,
                none.clone(),
                none.clone(),
                none.clone(),
                none.clone(),
                none.clone(),
                none
            ]
        );
        assert_eq!(r.artifacts[1].diagnostics[0].file, ".claude/skills/a");
    }

    #[test]
    fn skill_command_clash_warns_on_the_command_only() {
        let r = report(vec![
            art(ArtifactKind::Skill, "deploy", "skills/deploy"),
            art(ArtifactKind::Command, "deploy", "commands/deploy.md"),
            art(ArtifactKind::Command, "other", "commands/other.md"),
        ]);
        assert!(codes(&r.artifacts[0]).is_empty());
        assert_eq!(codes(&r.artifacts[1]), ["scan.skill-command-clash"]);
        assert_eq!(r.artifacts[1].diagnostics[0].file, "commands/deploy.md");
        assert!(codes(&r.artifacts[2]).is_empty());
    }
}
