use std::fs;

use serde_json::{Map, Value};

use super::facts::{join_names, list_field, normalise, rules_text, server_of, Facts};
use super::*;
use crate::artifact::ArtifactKind;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Fixture {
    _data: tempfile::TempDir,
    _src: tempfile::TempDir,
    library: Library,
}

impl Fixture {
    fn id(&self, kind: ArtifactKind, name: &str) -> std::result::Result<String, String> {
        self.library
            .find(kind, name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("{name} not in library"))
    }

    fn profile(&self, names: &[(ArtifactKind, &str)], mcp: &[&str]) -> TestResult2 {
        let mut spec = ProfileSpec::default();
        for (kind, name) in names {
            let id = self.id(*kind, name)?;
            if *kind == ArtifactKind::Settings {
                spec.settings = Some(id);
            } else {
                spec.items.push(id);
            }
        }
        for name in mcp {
            spec.mcp.push(crate::profiles::McpChoice {
                artifact: self.id(ArtifactKind::Mcp, name)?,
                env: Default::default(),
            });
        }
        Ok(self
            .library
            .create_profile("P", "", &spec, CategoryId::Software, None)?)
    }
}

type TestResult2 = std::result::Result<i64, Box<dyn std::error::Error>>;

fn write(root: &std::path::Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn fixture() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    let r = src.path();
    write(
        r,
        "skills/tdd/SKILL.md",
        "---\nname: tdd\ndescription: tdd\n---\nBody.\n",
    )?;
    write(
        r,
        "skills/review/SKILL.md",
        "---\nname: review\ndescription: review\ndisable-model-invocation: true\nallowed-tools: mcp__github__*, Read, mcp__ghost__lookup\n---\nBody.\n",
    )?;
    write(
        r,
        "commands/ship.md",
        "---\nname: ship\ndescription: ship\nallowed-tools:\n  - mcp__github__create_pr\n  - Bash(git push:*)\n---\nBody.\n",
    )?;
    write(
        r,
        "agents/tester.md",
        "---\nname: tester\ndescription: tester\nskills:\n  - tdd\nmcpServers:\n  - github\n  - sentry\n---\nBody.\n",
    )?;
    write(
        r,
        "rules/rust.md",
        "---\ndescription: rust\npaths:\n  - \"**/*.rs\"\n  - Cargo.toml\n---\nBody.\n",
    )?;
    write(r, "CLAUDE.md", "# Project\n")?;
    write(
        r,
        "output-styles/terse.md",
        "---\nname: terse\ndescription: terse\n---\nBody.\n",
    )?;
    write(
        r,
        "settings.json",
        r#"{"permissions":{"deny":["Read(.env)","Bash(rm:*)"],"allow":["Read"]},
"hooks":{"PreToolUse":[{"matcher":"Write|Edit","hooks":[{"type":"command","command":"scripts/path-guard.sh"}]}],
"SessionStart":[{"hooks":[{"type":"command","command":"scripts/hello.sh"}]}]},
"statusLine":{"type":"command","command":"scripts/line.sh"}}
"#,
    )?;
    write(r, "scripts/path-guard.sh", "#!/bin/sh\n")?;
    write(r, "scripts/hello.sh", "#!/bin/sh\n")?;
    write(r, "scripts/line.sh", "#!/bin/sh\n")?;
    write(
        r,
        ".mcp.json",
        "{\"mcpServers\":{\"github\":{\"command\":\"npx\",\"args\":[\"x\"]}}}\n",
    )?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: r.to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    Ok(Fixture {
        _data: data,
        _src: src,
        library,
    })
}

fn texts<'a>(all: &'a Facts, f: &Fixture, kind: ArtifactKind, name: &str) -> Vec<&'a str> {
    let id = f.id(kind, name).unwrap_or_default();
    all.get(&id)
        .map(|v| v.iter().map(|x| x.text.as_str()).collect())
        .unwrap_or_default()
}

fn full_profile(f: &Fixture) -> TestResult2 {
    f.profile(
        &[
            (ArtifactKind::Skill, "tdd"),
            (ArtifactKind::Skill, "review"),
            (ArtifactKind::Command, "ship"),
            (ArtifactKind::Agent, "tester"),
            (ArtifactKind::Rule, "rust"),
            (ArtifactKind::ClaudeMd, "CLAUDE.md"),
            (ArtifactKind::OutputStyle, "terse"),
            (ArtifactKind::Settings, "settings"),
        ],
        &["github"],
    )
}

#[test]
fn every_fact_kind_comes_from_a_fixture_artifact() -> TestResult {
    let f = fixture()?;
    let tester = f.id(ArtifactKind::Agent, "tester")?;
    let tdd = f.id(ArtifactKind::Skill, "tdd")?;
    f.library
        .add_link(&tester, &tdd, crate::bundles::LinkOrigin::Manual, "")?;
    let profile = full_profile(&f)?;
    let all = f.library.harnmap_facts(profile)?;
    let kinds = |kind, name| texts(&all, &f, kind, name);

    let t = kinds(ArtifactKind::Agent, "tester");
    assert!(t.contains(&"Calls github and tdd (bundle)"), "{t:?}");
    assert!(t.contains(&"Subagent skills field: tdd"));
    assert!(t.contains(&"Subagent mcpServers field: github, sentry"));
    assert_eq!(
        kinds(ArtifactKind::Skill, "tdd"),
        ["Called by tester (bundle)"]
    );

    let review = kinds(ArtifactKind::Skill, "review");
    assert!(
        review.contains(&"Allowed tools: mcp__github__*, mcp__ghost__lookup"),
        "{review:?}"
    );
    assert!(review.contains(&"Manual only: disable-model-invocation is set"));
    let ship = kinds(ArtifactKind::Command, "ship");
    assert_eq!(ship, ["Allowed tools: mcp__github__create_pr"]);

    assert_eq!(
        kinds(ArtifactKind::Rule, "rust"),
        ["Rule paths: **/*.rs, Cargo.toml"]
    );
    assert_eq!(
        kinds(ArtifactKind::ClaudeMd, "CLAUDE.md"),
        ["CLAUDE.md: loaded in every session"]
    );
    assert_eq!(
        kinds(ArtifactKind::OutputStyle, "terse"),
        ["Output style: applies to every reply"]
    );
    let settings = kinds(ArtifactKind::Settings, "settings");
    assert!(
        settings.contains(&"settings.json: 2 deny rules · 1 allow rule"),
        "{settings:?}"
    );
    assert!(
        settings
            .iter()
            .any(|t| t.starts_with("Hook: PreToolUse · Write|Edit → ")
                && t.ends_with("path-guard.sh")),
        "{settings:?}"
    );
    assert!(settings
        .iter()
        .any(|t| t.starts_with("Hook: SessionStart → ") && t.ends_with("hello.sh")));
    assert!(settings
        .iter()
        .any(|t| t.starts_with("Status line: ") && t.ends_with("line.sh")));
    assert_eq!(
        kinds(ArtifactKind::Mcp, "github"),
        ["Called by tester (bundle)"]
    );

    let kind_of = |name: &str| -> Vec<FactKind> {
        let id = f.id(ArtifactKind::Settings, name).unwrap_or_default();
        all[&id].iter().map(|x| x.kind.clone()).collect()
    };
    let k = kind_of("settings");
    for want in [FactKind::Permissions, FactKind::Hook, FactKind::StatusLine] {
        assert!(k.contains(&want), "{want:?}");
    }
    Ok(())
}

#[test]
fn a_server_the_profile_lacks_is_a_missing_mcp_fact() -> TestResult {
    let f = fixture()?;
    let all = f.library.harnmap_facts(full_profile(&f)?)?;
    let missing = |kind, name| -> Vec<String> {
        let id = f.id(kind, name).unwrap_or_default();
        all.get(&id)
            .into_iter()
            .flatten()
            .filter_map(|x| match &x.kind {
                FactKind::MissingMcp { server } => Some(server.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(missing(ArtifactKind::Agent, "tester"), ["sentry"]);
    assert_eq!(missing(ArtifactKind::Skill, "review"), ["ghost"]);
    assert!(missing(ArtifactKind::Command, "ship").is_empty());
    Ok(())
}

#[test]
fn leaving_the_server_out_makes_it_missing_and_text_says_so() -> TestResult {
    let f = fixture()?;
    let profile = f.profile(&[(ArtifactKind::Command, "ship")], &[])?;
    let all = f.library.harnmap_facts(profile)?;
    let id = f.id(ArtifactKind::Command, "ship")?;
    let found = &all[&id];
    assert!(found.iter().any(|x| x.kind
        == FactKind::MissingMcp {
            server: "github".into()
        }
        && x.text.contains("github")));
    Ok(())
}

#[test]
fn bundles_are_the_linked_groups_of_the_profile() -> TestResult {
    let f = fixture()?;
    let tester = f.id(ArtifactKind::Agent, "tester")?;
    let tdd = f.id(ArtifactKind::Skill, "tdd")?;
    let review = f.id(ArtifactKind::Skill, "review")?;
    f.library
        .add_link(&tester, &tdd, crate::bundles::LinkOrigin::Manual, "")?;
    f.library
        .add_link(&review, &tester, crate::bundles::LinkOrigin::Manual, "")?;
    let profile = full_profile(&f)?;
    let groups = f.library.harnmap_bundles(profile)?;
    assert_eq!(groups.len(), 1, "{groups:?}");
    let mut group = groups[0].clone();
    group.sort();
    // The import already linked the agent to the github server it names.
    let github = f.id(ArtifactKind::Mcp, "github")?;
    let mut want = vec![tester, tdd, review, github];
    want.sort();
    assert_eq!(group, want);
    Ok(())
}

#[test]
fn links_to_artifacts_outside_the_profile_are_not_facts() -> TestResult {
    let f = fixture()?;
    let tester = f.id(ArtifactKind::Agent, "tester")?;
    let tdd = f.id(ArtifactKind::Skill, "tdd")?;
    f.library
        .add_link(&tester, &tdd, crate::bundles::LinkOrigin::Manual, "")?;
    let profile = f.profile(&[(ArtifactKind::Rule, "rust")], &[])?;
    assert!(f.library.harnmap_bundles(profile)?.is_empty());
    Ok(())
}

#[test]
fn helpers_read_lists_and_names() {
    assert_eq!(join_names(&[]), "");
    assert_eq!(
        join_names(&["a".into(), "b".into(), "c".into()]),
        "a, b and c"
    );
    assert_eq!(server_of("mcp__github__*"), Some("github"));
    assert_eq!(server_of("Read"), None);
    assert_eq!(server_of("mcp____x"), None);
    assert_eq!(normalise("my.server"), "my_server");
    let mut m = Map::new();
    m.insert("allowed-tools".into(), Value::String("Read, Bash".into()));
    assert_eq!(list_field(&m, "allowed-tools"), ["Read", "Bash"]);
    assert!(list_field(&m, "paths").is_empty());
    assert_eq!(rules_text(1, "deny").as_deref(), Some("1 deny rule"));
    assert_eq!(rules_text(0, "deny"), None);
}

#[test]
fn the_view_carries_hooks_bundles_and_facts_for_the_rail_and_groups() -> TestResult {
    let f = fixture()?;
    let tester = f.id(ArtifactKind::Agent, "tester")?;
    let tdd = f.id(ArtifactKind::Skill, "tdd")?;
    f.library
        .add_link(&tester, &tdd, crate::bundles::LinkOrigin::Manual, "")?;
    let profile = full_profile(&f)?;
    let view = f.library.harnmap(profile)?;
    assert_eq!(
        view.hooks,
        vec![
            Hook {
                event: "PreToolUse".to_owned(),
                matcher: "Write|Edit".to_owned()
            },
            Hook {
                event: "SessionStart".to_owned(),
                matcher: String::new()
            },
        ]
    );
    assert!(view
        .bundles
        .iter()
        .any(|g| g.contains(&tester) && g.contains(&tdd)));
    assert!(view.facts.contains_key(&tester));
    assert!(view.stale.is_none() && !view.drawn);
    Ok(())
}
