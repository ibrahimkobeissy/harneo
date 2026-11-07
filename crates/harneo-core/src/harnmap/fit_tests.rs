//! Harnfit against fixture scripts standing in for `claude`. The real CLI is never started.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::params;
use serde_json::{json, Value};

use super::draw_tests::{draw, fixture, valid, Fixture};
use super::fit::{allot, budget_order};
use super::*;
use crate::artifact::ArtifactKind;
use crate::error::Error;
use crate::library::{Preview, Source, TagChoices};
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const LITE: &str =
    "---\nname: tdd-lite\ndescription: Lite test first\n---\nIntro line\nTest first,\nalways.\n";

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn source(files: &[(&str, &str)]) -> std::result::Result<tempfile::TempDir, std::io::Error> {
    let dir = tempfile::tempdir()?;
    for (rel, text) in files {
        write(dir.path(), rel, text)?;
    }
    Ok(dir)
}

fn preview_of(f: &Fixture, dir: &Path) -> std::result::Result<Preview, Box<dyn std::error::Error>> {
    Ok(f.library.preview(&Source::Local {
        path: dir.to_path_buf(),
    })?)
}

/// Imports `files` into the fixture's library and returns the ids of what came in.
fn import(
    f: &mut Fixture,
    files: &[(&str, &str)],
) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
    let dir = source(files)?;
    let preview = preview_of(f, dir.path())?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    Ok(ids)
}

fn lite(f: &mut Fixture) -> std::result::Result<String, Box<dyn std::error::Error>> {
    import(f, &[("skills/tdd-lite/SKILL.md", LITE)])?
        .into_iter()
        .next()
        .ok_or_else(|| "imported".into())
}

fn answer(quotes: Value) -> Value {
    json!({
        "verdict": "conflicts",
        "zone": "stage:Build",
        "why": "It overlaps tdd.",
        "change": "Merge the two.",
        "conflicts_with": ["a1"],
        "quotes": quotes,
    })
}

fn good() -> Value {
    answer(json!([
        {"artifact": "candidate", "text": "Test   first,\n always."},
        {"artifact": "a1", "text": "Write the failing test"},
        {"artifact": "candidate", "text": "this was never written anywhere"},
        {"artifact": "candidate", "text": "   "},
    ]))
}

/// A fixture with a drawn map whose Claude now answers `answers`.
fn ready(answers: &[Value]) -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let mut f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    f.claude(answers)?;
    Ok(f)
}

fn stored_file(f: &Fixture, id: &str) -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
    let folder = f.library.store.artifacts_dir().join(id);
    Ok(fs::read_dir(folder)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n == "SKILL.md"))
        .ok_or("SKILL.md")?)
}

fn set_digest(f: &Fixture, id: &str, digest: &str) -> TestResult {
    f.library.store.conn.execute(
        "UPDATE artifacts SET artifact = json_set(artifact, '$.digest', ?2) WHERE id = ?1",
        params![id, digest],
    )?;
    Ok(())
}

#[test]
fn a_verdict_is_saved_with_checked_quotes_and_an_invented_one_is_dropped() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    let view = f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    let v = &view.verdict;
    assert_eq!(v.verdict, Verdict::Conflicts);
    assert_eq!(v.why, "It overlaps tdd.");
    assert_eq!(v.change.as_deref(), Some("Merge the two."));
    assert_eq!(v.conflicts_with, vec![f.id("tdd")]);
    // Two quotes were found, two (invented, blank) were dropped and counted.
    assert_eq!(v.quotes.len(), 2);
    assert_eq!(v.dropped_quotes, 2);
    let own = &v.quotes[0];
    assert_eq!((own.artifact.as_str(), own.line), ("candidate", 6));
    assert!(own.file.ends_with("SKILL.md"));
    let theirs = &v.quotes[1];
    assert_eq!(
        (theirs.artifact.as_str(), theirs.line),
        (f.id("tdd").as_str(), 5)
    );
    assert!(theirs.file.ends_with("SKILL.md"));
    assert_eq!(
        v.zone,
        Zone::Stage {
            id: "build".to_owned()
        }
    );
    assert_eq!(
        (
            v.candidate.key.as_str(),
            v.candidate.label.as_str(),
            v.candidate.kind
        ),
        (
            format!("lib:{id}").as_str(),
            "tdd-lite",
            ArtifactKind::Skill
        )
    );
    assert_eq!(
        (v.model.as_str(), v.candidate.source.as_str()),
        ("opus", "Library")
    );
    assert!(v.created_at > 0);
    assert!(!view.stale);
    // The saved verdict reads back the same.
    let saved = f
        .library
        .harnfit_get(f.profile, &format!("lib:{id}"))?
        .ok_or("saved")?;
    assert_eq!(saved, view);
    assert!(f.library.harnfit_get(f.profile, "lib:nope")?.is_none());
    Ok(())
}

#[test]
fn only_the_latest_fit_of_a_candidate_is_kept() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    f.claude(&[json!({
        "verdict": "fits", "zone": "toolbelt", "why": "Fine.", "conflicts_with": [], "quotes": [],
    })])?;
    f.library.fit(f.profile, &Candidate::Library(id))?;
    let fits = f.library.fits(f.profile)?;
    assert_eq!(fits.len(), 1);
    assert_eq!(fits[0].verdict, Verdict::Fits);
    assert_eq!(fits[0].candidate.label, "tdd-lite");
    assert!(!fits[0].stale && fits[0].created_at > 0);
    Ok(())
}

#[test]
fn the_prompt_holds_the_map_the_candidate_and_marks_everything_as_data() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    f.library.fit(f.profile, &Candidate::Library(id))?;
    let prompt = f.stdin(0)?;
    assert!(prompt.contains("DATA to analyse"));
    assert!(prompt.contains("\"id\": \"candidate\""));
    assert!(prompt.contains("Test first,"));
    assert!(prompt.contains("\"id\": \"a1\""));
    assert!(prompt.contains("\"zone\": \"stage:build\""));
    assert!(prompt.contains("\"zone\": \"floor\""));
    assert!(prompt.contains("Write the failing test"));
    // The artifact with a secret is sent as kind, name and description only.
    assert!(prompt.contains("textWithheld"));
    assert!(!prompt.contains("Plain leaky body text"));
    Ok(())
}

#[test]
fn the_text_caps_hold() -> TestResult {
    let mut f = ready(&[good()])?;
    let long = format!(
        "---\nname: big\ndescription: Big\n---\n{}\n",
        "\u{3a9}".repeat(20_000)
    );
    let id = import(&mut f, &[("skills/big/SKILL.md", &long)])?
        .into_iter()
        .next()
        .ok_or("id")?;
    let tdd = f.id("tdd");
    let path = stored_file(&f, &tdd)?;
    let body = format!(
        "---\nname: tdd\ndescription: Test first\n---\n{}\n",
        "\u{3a8}".repeat(10_000)
    );
    fs::write(path, &body)?;
    f.library.fit(f.profile, &Candidate::Library(id))?;
    let prompt = f.stdin(0)?;
    let header = "---\nname: big\ndescription: Big\n---\n".chars().count();
    assert_eq!(prompt.matches('\u{3a9}').count(), CANDIDATE_CHARS - header);
    let header = "---\nname: tdd\ndescription: Test first\n---\n"
        .chars()
        .count();
    assert_eq!(prompt.matches('\u{3a8}').count(), 4000 - header);
    Ok(())
}

#[test]
fn same_kind_artifacts_come_first_and_the_total_cap_holds() {
    use ArtifactKind::{Command, Skill};
    let kinds = [Skill, Skill, Command, Skill, Command];
    assert_eq!(budget_order(&kinds, Command), vec![2, 4, 0, 1, 3]);
    assert_eq!(budget_order(&kinds, Skill), vec![0, 1, 3, 2, 4]);
    assert_eq!(budget_order(&[], Skill), Vec::<usize>::new());
    // 3 x 4000 would be 12000: with a cap of 9000, the third gets nothing, and nothing later
    // takes its place.
    let sent = allot(&[0, 1, 2, 3], &[4000, 4000, 4000, 100], 9000);
    assert_eq!(sent, vec![4000, 4000, 0, 0]);
    // Artifacts with no text do not use the budget.
    assert_eq!(allot(&[0, 1], &[0, 50], 10), vec![0, 0]);
    assert_eq!(allot(&[1, 0], &[0, 50], 100), vec![0, 50]);
}

#[test]
fn a_big_profile_sends_the_candidates_kind_first_and_the_rest_by_name_only() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let mut files: Vec<(String, String)> = (0..31)
        .map(|n| {
            (
                format!("skills/s{n:02}/SKILL.md"),
                format!(
                    "---\nname: s{n:02}\ndescription: Skill {n}\n---\n{}\n",
                    "x".repeat(5000)
                ),
            )
        })
        .collect();
    files.push((
        "commands/ship.md".to_owned(),
        format!(
            "---\ndescription: Ship it\n---\nCMD_MARKER {}\n",
            "y".repeat(5000)
        ),
    ));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let ids = import(&mut f, &refs)?;
    let spec = ProfileSpec {
        items: ids,
        ..ProfileSpec::default()
    };
    let profile = f
        .library
        .create_profile("Big", "", &spec, CategoryId::Software, None)?;
    f.claude(&[valid()])?;
    f.library.harnmap_draw(profile, "")?;
    let id = lite(&mut f)?;
    // The candidate is a command: the one command of the profile gets its text first.
    let cmd_candidate = import(
        &mut f,
        &[(
            "commands/other.md",
            "---\ndescription: Other\n---\nOther body\n",
        )],
    )?
    .into_iter()
    .next()
    .ok_or("id")?;
    let job = f.library.prepare_fit(
        profile,
        &Candidate::Library(cmd_candidate),
        Arc::new(AtomicBool::new(false)),
    )?;
    assert_eq!(job.artifact_count(), 32);
    // 30 x 4000 = 120000 fills the budget: the command and 29 skills have text.
    assert_eq!(job.text_count(), 30);
    assert!(job.prompt_for_test()?.contains("CMD_MARKER"));
    // A skill candidate gets no such preference: map order decides (the placed ones first).
    let job = f.library.prepare_fit(
        profile,
        &Candidate::Library(id),
        Arc::new(AtomicBool::new(false)),
    )?;
    assert_eq!(job.text_count(), 30);
    Ok(())
}

#[test]
fn a_fit_is_out_of_date_after_a_profile_or_candidate_change() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    let key = format!("lib:{id}");
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;

    // A profile artifact changes.
    let tdd = f.id("tdd");
    set_digest(&f, &tdd, "changed")?;
    let view = f.library.harnfit_get(f.profile, &key)?.ok_or("fit")?;
    assert!(view.stale);
    assert_eq!(view.changes.changed, vec![tdd.clone()]);
    assert!(f.library.fits(f.profile)?[0].stale);
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    assert!(!f.library.fits(f.profile)?[0].stale);

    // An artifact joins the profile.
    let extra = import(
        &mut f,
        &[(
            "skills/extra/SKILL.md",
            "---\nname: extra\ndescription: E\n---\nx\n",
        )],
    )?
    .remove(0);
    let mut spec = f.library.profile(f.profile, None)?.spec;
    spec.items.push(extra.clone());
    f.library.save_profile(f.profile, &spec)?;
    let view = f.library.harnfit_get(f.profile, &key)?.ok_or("fit")?;
    assert_eq!(view.changes.added, vec![extra.clone()]);
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    assert!(!f.library.fits(f.profile)?[0].stale);

    // An artifact leaves the profile.
    spec.items.retain(|i| *i != extra);
    f.library.save_profile(f.profile, &spec)?;
    let view = f.library.harnfit_get(f.profile, &key)?.ok_or("fit")?;
    assert_eq!(view.changes.removed, vec![extra]);
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;

    // The candidate changes.
    set_digest(&f, &id, "other")?;
    let view = f.library.harnfit_get(f.profile, &key)?.ok_or("fit")?;
    assert!(view.stale && view.changes.candidate_changed);
    assert!(view.changes.added.is_empty());
    Ok(())
}

#[test]
fn add_creates_a_profile_version_and_a_pinned_placement() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    let key = format!("lib:{id}");
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    let before = f.library.profile(f.profile, None)?;
    let after = f.library.fit_add(f.profile, &key)?;
    let now = f.library.profile(f.profile, None)?;
    assert_eq!(now.version, before.version + 1);
    assert!(now.spec.items.contains(&id));
    let map = f.library.harnmap(f.profile)?;
    let placed = map
        .doc
        .placements
        .iter()
        .find(|p| p.artifact == id)
        .ok_or("placed")?;
    assert!(placed.pinned);
    assert_eq!(placed.by, PlacedBy::User);
    assert_eq!(placed.why, "It overlaps tdd.");
    assert_eq!(placed.note, "It overlaps tdd.");
    assert_eq!(
        placed.zone,
        Zone::Stage {
            id: "build".to_owned()
        }
    );
    assert!(map.unplaced.is_empty() || !map.unplaced.contains(&id));
    // The profile changed, so the fit is out of date; adding again is refused.
    assert!(after.stale);
    assert!(matches!(
        f.library.fit_add(f.profile, &key),
        Err(Error::HarnfitAlreadyInProfile { .. })
    ));
    Ok(())
}

#[test]
fn add_covers_settings_mcp_and_long_reasons() -> TestResult {
    let long = "w".repeat(200);
    let answer_at = |zone: &str| json!({"verdict": "fits", "zone": zone, "why": long, "conflicts_with": [], "quotes": []});
    let mut f = ready(&[answer_at("toolbelt")])?;
    let ids = import(
        &mut f,
        &[
            (
                ".mcp.json",
                "{\"mcpServers\":{\"github\":{\"command\":\"npx\",\"args\":[\"x\"]}}}\n",
            ),
            (
                ".claude/settings.json",
                "{\"permissions\":{\"deny\":[\"Bash(rm:*)\"]}}\n",
            ),
        ],
    )?;
    let kinds: Vec<(String, ArtifactKind)> = ids
        .iter()
        .map(|i| {
            let k = f.library.store.conn.query_row(
                "SELECT json_extract(artifact, '$.kind') FROM artifacts WHERE id = ?1",
                params![i],
                |r| r.get::<_, String>(0),
            )?;
            Ok((i.clone(), serde_json::from_value(Value::String(k))?))
        })
        .collect::<std::result::Result<_, Box<dyn std::error::Error>>>()?;
    for (id, kind) in &kinds {
        let key = format!("lib:{id}");
        f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
        f.library.fit_add(f.profile, &key)?;
        let spec = f.library.profile(f.profile, None)?.spec;
        match kind {
            ArtifactKind::Mcp => assert!(spec.mcp.iter().any(|m| m.artifact == *id)),
            ArtifactKind::Settings => assert_eq!(spec.settings.as_deref(), Some(id.as_str())),
            other => return Err(format!("unexpected {other:?}").into()),
        }
        let placed = f
            .library
            .harnmap(f.profile)?
            .doc
            .placements
            .into_iter()
            .find(|p| p.artifact == *id)
            .ok_or("placed")?;
        assert_eq!(placed.zone, Zone::Toolbelt);
        assert_eq!(placed.note.chars().count(), 60);
        assert_eq!(placed.why, long);
    }
    assert_eq!(kinds.len(), 2);
    Ok(())
}

#[test]
fn a_second_settings_template_is_refused_and_nothing_changes() -> TestResult {
    let mut f = ready(&[answer(json!([]))])?;
    let first = import(
        &mut f,
        &[(
            ".claude/settings.json",
            "{\"permissions\":{\"deny\":[\"Bash(rm:*)\"]}}\n",
        )],
    )?
    .remove(0);
    let mut spec = f.library.profile(f.profile, None)?.spec;
    spec.settings = Some(first);
    f.library.save_profile(f.profile, &spec)?;
    let second = import(
        &mut f,
        &[(
            ".claude/settings.json",
            "{\"permissions\":{\"allow\":[\"Bash(ls:*)\"]}}\n",
        )],
    )?
    .remove(0);
    let key = format!("lib:{second}");
    f.library.fit(f.profile, &Candidate::Library(second))?;
    let version = f.library.profile(f.profile, None)?.version;
    assert!(matches!(
        f.library.fit_add(f.profile, &key),
        Err(Error::InvalidProfile { .. })
    ));
    assert_eq!(f.library.profile(f.profile, None)?.version, version);
    Ok(())
}

#[test]
fn add_refuses_previews_unknown_fits_and_a_stage_that_is_gone() -> TestResult {
    let mut f = ready(&[good()])?;
    assert!(matches!(
        f.library.fit_add(f.profile, "src:/x@local:skill/y"),
        Err(Error::ImportFirst)
    ));
    let id = lite(&mut f)?;
    let key = format!("lib:{id}");
    assert!(matches!(
        f.library.fit_add(f.profile, &key),
        Err(Error::FitNotFound)
    ));
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    // The proposed stage was removed meanwhile: the add is refused whole.
    f.library.harnmap_edit(
        f.profile,
        MapEdit::RemoveStage {
            id: "build".to_owned(),
        },
    )?;
    let version = f.library.profile(f.profile, None)?.version;
    assert!(f.library.fit_add(f.profile, &key).is_err());
    assert_eq!(f.library.profile(f.profile, None)?.version, version);
    Ok(())
}

#[test]
fn a_plugin_and_a_profile_without_a_map_are_refused() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let id = lite(&mut f)?;
    // No map yet.
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(id.clone())),
        Err(Error::NoMap)
    ));
    // A plugin taken whole, from the library and from a preview.
    f.library.store.conn.execute(
        "UPDATE artifacts SET artifact = json_set(artifact, '$.kind', 'plugin') WHERE id = ?1",
        params![id],
    )?;
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(id)),
        Err(Error::HarnfitPluginUnsupported)
    ));
    let dir = source(&[(
        "skills/p/SKILL.md",
        "---\nname: p\ndescription: P\n---\nBody\n",
    )])?;
    let mut preview = preview_of(&f, dir.path())?;
    let candidate = preview.candidates[0].id.clone();
    preview.candidates[0].artifact.kind = ArtifactKind::Plugin;
    assert!(matches!(
        f.library.fit(
            f.profile,
            &Candidate::Preview {
                preview: Box::new(preview),
                candidate
            }
        ),
        Err(Error::HarnfitPluginUnsupported)
    ));
    Ok(())
}

#[test]
fn unknown_and_present_candidates_are_refused() -> TestResult {
    let f = ready(&[good()])?;
    assert!(matches!(
        f.library
            .fit(f.profile, &Candidate::Library("nope".to_owned())),
        Err(Error::NotInLibrary { .. })
    ));
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(f.id("tdd"))),
        Err(Error::HarnfitAlreadyInProfile { .. })
    ));
    let dir = source(&[(
        "skills/p/SKILL.md",
        "---\nname: p\ndescription: P\n---\nBody\n",
    )])?;
    let preview = preview_of(&f, dir.path())?;
    assert!(matches!(
        f.library.fit(
            f.profile,
            &Candidate::Preview {
                preview: Box::new(preview),
                candidate: "nope".to_owned()
            }
        ),
        Err(Error::NotInPreview { .. })
    ));
    Ok(())
}

#[test]
fn a_preview_candidate_is_read_like_an_import_and_keyed_by_its_source() -> TestResult {
    let f = ready(&[good()])?;
    let token = ["gh", "p_", &"d".repeat(36)].concat();
    let dir = source(&[
        ("skills/tdd-lite/SKILL.md", LITE),
        (
            "skills/leaky/SKILL.md",
            &format!("---\nname: leaky2\ndescription: L\n---\nBody {token}\n"),
        ),
    ])?;
    let preview = preview_of(&f, dir.path())?;
    let pick = |name: &str| -> std::result::Result<String, Box<dyn std::error::Error>> {
        Ok(preview
            .candidates
            .iter()
            .find(|c| c.artifact.name == name)
            .ok_or("candidate")?
            .id
            .clone())
    };
    let (lite_id, leaky_id) = (pick("tdd-lite")?, pick("leaky2")?);
    let view = f.library.fit(
        f.profile,
        &Candidate::Preview {
            preview: Box::new(preview.clone()),
            candidate: lite_id,
        },
    )?;
    let key = &view.verdict.candidate.key;
    assert!(key.starts_with(&format!("src:{}@local:", dir.path().display())));
    assert!(key.ends_with(":skill/tdd-lite"));
    assert_eq!(
        view.verdict.candidate.source,
        dir.path().display().to_string()
    );
    assert_eq!(view.verdict.quotes.len(), 2);
    assert!(!view.stale);
    // The profile moving on makes it stale; the candidate is not read again.
    let tdd = f.id("tdd");
    set_digest(&f, &tdd, "changed")?;
    assert!(f.library.harnfit_get(f.profile, key)?.ok_or("fit")?.stale);
    assert!(matches!(
        f.library.fit_add(f.profile, key),
        Err(Error::ImportFirst)
    ));
    // A candidate with a secret in it is sent without its text.
    f.library.fit(
        f.profile,
        &Candidate::Preview {
            preview: Box::new(preview),
            candidate: leaky_id,
        },
    )?;
    let prompt = f.stdin(1)?;
    assert!(prompt.contains("leaky2"));
    assert!(prompt.contains("textWithheld"));
    assert!(!prompt.contains(&token));
    assert!(!prompt.contains("Body ghp"));
    Ok(())
}

#[test]
fn missing_mcp_servers_come_from_the_files_with_whether_the_library_has_them() -> TestResult {
    let mut f = ready(&[answer(json!([]))])?;
    import(
        &mut f,
        &[(
            ".mcp.json",
            "{\"mcpServers\":{\"github\":{\"command\":\"npx\",\"args\":[\"x\"]}}}\n",
        )],
    )?;
    let id = import(
        &mut f,
        &[(
            "agents/helper.md",
            "---\nname: helper\ndescription: Helps\nmcpServers:\n  - github\n  - sentry\n---\nBody.\n",
        )],
    )?
    .remove(0);
    let view = f.library.fit(f.profile, &Candidate::Library(id))?;
    assert_eq!(
        view.verdict.missing_mcp,
        vec![
            MissingMcpServer {
                server: "github".to_owned(),
                in_library: true
            },
            MissingMcpServer {
                server: "sentry".to_owned(),
                in_library: false
            },
        ]
    );
    // A skill naming its server through allowed tools; a skill naming none.
    let skill = import(
        &mut f,
        &[(
            "skills/gh/SKILL.md",
            "---\nname: gh\ndescription: G\nallowed-tools: mcp__github__list, mcp__github__get, Read\n---\nBody.\n",
        )],
    )?
    .remove(0);
    let view = f.library.fit(f.profile, &Candidate::Library(skill))?;
    assert_eq!(view.verdict.missing_mcp.len(), 1);
    assert_eq!(view.verdict.missing_mcp[0].server, "github");
    Ok(())
}

#[test]
fn a_bad_answer_is_retried_once_and_a_second_one_is_refused() -> TestResult {
    let mut bad_zone = good();
    bad_zone["zone"] = json!("stage:Nowhere");
    bad_zone["conflicts_with"] = json!(["a99"]);
    bad_zone["verdict"] = json!("maybe");
    bad_zone["why"] = json!(" ");
    bad_zone["quotes"] = json!([{"artifact": "a77", "text": "x"}]);
    let mut f = ready(&[bad_zone.clone(), good()])?;
    let id = lite(&mut f)?;
    let calls = f.calls();
    let view = f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    assert_eq!(view.verdict.verdict, Verdict::Conflicts);
    assert_eq!(f.calls(), calls + 2);
    let retry = f.stdin(calls + 1)?;
    assert!(retry.contains("rejected"));
    assert!(retry.contains("unknown stage"));
    assert!(retry.contains("unknown artifact"));
    assert!(retry.contains("verdict must be"));
    assert!(retry.contains("`why` is empty"));

    f.claude(&[bad_zone.clone(), bad_zone])?;
    match f.library.fit(f.profile, &Candidate::Library(id.clone())) {
        Err(Error::BadAnswer { reasons }) => assert!(!reasons.is_empty()),
        other => return Err(format!("{other:?}").into()),
    }
    // Not even valid JSON for the schema.
    f.claude(&[json!({"verdict": 1})])?;
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(id.clone())),
        Err(Error::BadAnswer { .. })
    ));
    // A zone that is not a zone.
    let mut odd = good();
    odd["zone"] = json!("the basement");
    f.claude(&[odd])?;
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(id)),
        Err(Error::BadAnswer { .. })
    ));
    Ok(())
}

#[test]
fn a_cancelled_fit_saves_nothing_and_claude_failures_pass_through() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let job = f
        .library
        .prepare_fit(f.profile, &Candidate::Library(id.clone()), cancel.clone())?;
    let result = job.run()?;
    cancel.store(true, Ordering::SeqCst);
    assert!(matches!(
        f.library.save_fit(&job, result),
        Err(Error::Cancelled)
    ));
    assert!(f.library.fits(f.profile)?.is_empty());
    f.library.claude.program = "/nonexistent/claude".into();
    assert!(matches!(
        f.library.fit(f.profile, &Candidate::Library(id)),
        Err(Error::ClaudeMissing | Error::ClaudeFailed { .. })
    ));
    Ok(())
}

#[test]
fn fits_for_lists_each_profile_with_a_fit_of_the_artifact() -> TestResult {
    let mut f = ready(&[good()])?;
    let id = lite(&mut f)?;
    assert!(f.library.fits_for(&id)?.is_empty());
    let spec = f.library.profile(f.profile, None)?.spec;
    let second = f
        .library
        .create_profile("Another", "", &spec, CategoryId::Software, None)?;
    f.claude(&[valid()])?;
    f.library.harnmap_draw(second, "")?;
    f.claude(&[good()])?;
    f.library.fit(second, &Candidate::Library(id.clone()))?;
    f.library.fit(f.profile, &Candidate::Library(id.clone()))?;
    let fits = f.library.fits_for(&id)?;
    assert_eq!(fits.len(), 2);
    assert_eq!(fits[0].profile_name, "Another");
    assert_eq!(fits[1].profile_name, "P");
    assert!(fits
        .iter()
        .all(|x| x.verdict == Verdict::Conflicts && !x.stale));
    let tdd = f.id("tdd");
    set_digest(&f, &tdd, "changed")?;
    assert!(f.library.fits_for(&id)?.iter().all(|x| x.stale));
    // The other candidate of the fixture has none.
    assert!(f.library.fits_for(&tdd)?.is_empty());
    Ok(())
}
