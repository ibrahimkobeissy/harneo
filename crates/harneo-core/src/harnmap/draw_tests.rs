//! Draw against fixture scripts standing in for `claude`. The real CLI is never started.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::draw::{changes, draw_model};
use super::*;
use crate::artifact::ArtifactKind;
use crate::bundles::claude_cli_tests::{config, script};
use crate::bundles::FailureKind;
use crate::error::Error;
use crate::library::{Library, Source, TagChoices};
use crate::profiles::ProfileSpec;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

pub(super) struct Fixture {
    _data: tempfile::TempDir,
    bin: tempfile::TempDir,
    pub(super) library: Library,
    pub(super) profile: i64,
}

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

pub(super) fn fixture(
    category: CategoryId,
    note: Option<&str>,
) -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let src = tempfile::tempdir()?;
    let r = src.path();
    write(
        r,
        "skills/tdd/SKILL.md",
        "---\nname: tdd\ndescription: Test first\n---\nWrite the failing test, then the code.\n",
    )?;
    write(
        r,
        "skills/review/SKILL.md",
        "---\nname: review\ndescription: Review code\n---\nRead the diff.\n",
    )?;
    write(
        r,
        "skills/leaky/SKILL.md",
        "---\nname: leaky\ndescription: A leaky skill\n---\nPlain leaky body text.\n",
    )?;
    write(r, "CLAUDE.md", "# Project\nAlways be kind.\n")?;
    let mut library = Library::open(data.path())?;
    let preview = library.preview(&Source::Local {
        path: r.to_path_buf(),
    })?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    library.import(&preview, &ids, &TagChoices::new())?;
    // The leaky skill is edited after import to hold a token: its text must never be sent.
    let leaky = library.find(ArtifactKind::Skill, "leaky")?.ok_or("leaky")?;
    let folder = library.store.artifacts_dir().join(&leaky);
    let skill = fs::read_dir(&folder)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n == "SKILL.md"))
        .ok_or("stored SKILL.md")?;
    let token = ["gh", "p_", &"d".repeat(36)].concat();
    fs::write(
        &skill,
        format!(
            "---\nname: leaky\ndescription: A leaky skill\n---\nPlain leaky body text. {token}\n"
        ),
    )?;
    let mut spec = ProfileSpec::default();
    for (kind, name) in [
        (ArtifactKind::Skill, "tdd"),
        (ArtifactKind::Skill, "review"),
        (ArtifactKind::Skill, "leaky"),
        (ArtifactKind::ClaudeMd, "CLAUDE.md"),
    ] {
        let id = library.find(kind, name)?.ok_or(name)?;
        spec.items.push(id);
    }
    let profile = library.create_profile("P", "", &spec, category, note)?;
    let bin = tempfile::tempdir()?;
    let mut f = Fixture {
        _data: data,
        bin,
        library,
        profile,
    };
    f.claude(&[valid()])?;
    Ok(f)
}

impl Fixture {
    /// Claude answers with `answers` in turn (the last one repeats); each call is recorded as
    /// `in-N.txt` and `args-N.txt`.
    pub(super) fn claude(&mut self, answers: &[Value]) -> std::io::Result<()> {
        let d = self.bin.path().display().to_string();
        for entry in fs::read_dir(self.bin.path())? {
            fs::remove_file(entry?.path())?;
        }
        for (n, a) in answers.iter().enumerate() {
            let body = json!({"is_error": false, "structured_output": a}).to_string();
            fs::write(self.bin.path().join(format!("ans-{n}")), &body)?;
            if n + 1 == answers.len() {
                fs::write(self.bin.path().join("ans-last"), &body)?;
            }
        }
        let program = script(
            self.bin.path(),
            "claude",
            &format!(
                "n=$(ls '{d}' | grep -c '^in-')\ncat > \"{d}/in-$n.txt\"\nprintf '%s\\n' \"$@\" > \"{d}/args-$n.txt\"\nf=\"{d}/ans-$n\"\n[ -f \"$f\" ] || f=\"{d}/ans-last\"\ncat \"$f\""
            ),
        )?;
        self.library.claude = config(&program);
        Ok(())
    }

    fn script(&mut self, body: &str) -> std::io::Result<()> {
        let program = script(self.bin.path(), "claude", body)?;
        self.library.claude = config(&program);
        Ok(())
    }

    pub(super) fn calls(&self) -> usize {
        fs::read_dir(self.bin.path())
            .map(|d| {
                d.filter_map(std::result::Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("in-"))
                    .count()
            })
            .unwrap_or(0)
    }

    pub(super) fn stdin(&self, n: usize) -> std::io::Result<String> {
        fs::read_to_string(self.bin.path().join(format!("in-{n}.txt")))
    }

    pub(super) fn id(&self, name: &str) -> String {
        let kind = if name == "CLAUDE.md" {
            ArtifactKind::ClaudeMd
        } else {
            ArtifactKind::Skill
        };
        self.library
            .find(kind, name)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn draws(&self) -> i64 {
        self.library
            .store
            .conn
            .query_row("SELECT COUNT(*) FROM harnmap_draws", [], |r| r.get(0))
            .unwrap_or(-1)
    }
}

fn stage_names() -> Vec<Value> {
    ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"]
        .iter()
        .map(|n| json!({"name": n}))
        .collect()
}

pub(super) fn placement(short: &str, zone: &str) -> Value {
    json!({"artifact": short, "zone": zone, "note": "n", "why": "w", "role": "r", "when": "t", "hands_off": "h"})
}

pub(super) fn valid() -> Value {
    json!({
        "stages": stage_names(),
        "loops": [],
        "placements": [placement("a1", "stage:Build"), placement("a2", "stage:review"), placement("a4", "floor")],
    })
}

fn invalid() -> Value {
    json!({"stages": stage_names(), "loops": [], "placements": [placement("a99", "stage:Build")]})
}

pub(super) fn draw(f: &Fixture) -> Result<DrawOutcome> {
    f.library.harnmap_draw(f.profile, "Spec first")
}

use crate::error::Result;

#[test]
fn a_valid_answer_is_saved_as_the_next_draw_with_model_and_duration() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let first = draw(&f)?;
    assert_eq!((first.no, first.model.as_str()), (1, "opus"));
    let second = draw(&f)?;
    assert_eq!(second.no, 2);
    let view = f.library.harnmap(f.profile)?;
    assert_eq!((view.no, view.model.as_str()), (2, "opus"));
    assert_eq!(view.description, "Spec first");
    assert!(view.created_at > 0);
    assert_eq!(view.duration_ms, second.duration_ms);
    assert_eq!(view.doc.placements.len(), 3);
    assert_eq!(view.doc.stages.len(), 7);
    // The stage names were matched by id or name, case-insensitively.
    let build = view
        .doc
        .stages
        .iter()
        .find(|s| s.name == "Build")
        .ok_or("build")?;
    assert_eq!(build.origin, StageOrigin::Template);
    let placed = &view.doc.placements[0];
    assert_eq!(
        placed.zone,
        Zone::Stage {
            id: build.id.clone()
        }
    );
    assert_eq!(placed.by, PlacedBy::Claude);
    assert!(!placed.pinned);
    // One artifact (the flagged skill) is not placed.
    assert!(view
        .doc
        .changes
        .contains(&TemplateChange::Unplaced { count: 1 }));
    assert_eq!(f.draws(), 2);
    Ok(())
}

#[test]
fn nothing_is_drawn_yet_for_a_new_profile() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let view = f.library.harnmap(f.profile)?;
    assert!(!view.drawn && view.no == 0 && view.doc.stages.len() == 7);
    assert_eq!(view.unplaced.len(), 4);
    Ok(())
}

#[test]
fn changes_from_the_template_are_computed() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let mut stages = stage_names();
    stages[3] = json!({"id": "build", "name": "Implement"});
    stages.remove(0);
    stages.push(json!({"name": "Operate & Learn"}));
    f.claude(&[json!({
        "stages": stages,
        "loops": [{"from": "Operate & Learn", "to": "spec", "label": "lessons"}],
        "placements": [placement("a1", "stage:implement")],
    })])?;
    let out = draw(&f)?;
    let c = &out.changes;
    assert!(c.contains(&TemplateChange::StageKept { id: "spec".into() }));
    assert!(c.contains(&TemplateChange::StageRenamed {
        id: "build".into(),
        from: "Build".into(),
        to: "Implement".into()
    }));
    assert!(c.contains(&TemplateChange::StageRemoved {
        id: "idea".into(),
        name: "Idea".into()
    }));
    assert!(c.contains(&TemplateChange::StageAdded {
        id: "operate-learn".into(),
        name: "Operate & Learn".into()
    }));
    assert!(c.contains(&TemplateChange::LoopAdded {
        from: "operate-learn".into(),
        to: "spec".into(),
        label: "lessons".into()
    }));
    assert!(c.contains(&TemplateChange::Unplaced { count: 3 }));
    let view = f.library.harnmap(f.profile)?;
    let added = view
        .doc
        .stages
        .iter()
        .find(|s| s.id == "operate-learn")
        .ok_or("added")?;
    assert_eq!(added.origin, StageOrigin::Claude);
    Ok(())
}

#[test]
fn an_invalid_answer_is_retried_once_with_the_reasons() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    f.claude(&[invalid(), valid()])?;
    let out = draw(&f)?;
    assert_eq!(out.no, 1);
    assert_eq!(f.calls(), 2);
    assert!(!f.stdin(0)?.contains("rejected"));
    let retry = f.stdin(1)?;
    assert!(
        retry.contains("rejected") && retry.contains("a99"),
        "{retry}"
    );
    Ok(())
}

#[test]
fn two_invalid_answers_save_nothing_and_return_the_reasons() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    f.claude(&[invalid()])?;
    match draw(&f) {
        Err(Error::BadAnswer { reasons }) => {
            assert!(reasons.iter().any(|r| r.contains("a99")), "{reasons:?}");
        }
        other => panic!("expected BadAnswer, got {other:?}"),
    }
    assert_eq!(f.calls(), 2);
    assert_eq!(f.draws(), 0);
    Ok(())
}

#[test]
fn every_kind_of_bad_answer_is_named() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let cases = [
        (json!({"stages": "x"}), "does not match the schema"),
        (
            json!({"stages": [], "loops": [], "placements": []}),
            "no stages",
        ),
        (
            json!({"stages": [{"name": " "}], "loops": [], "placements": []}),
            "has no name",
        ),
        (
            json!({"stages": stage_names(), "loops": [{"from": "Idea", "to": "Nowhere", "label": ""}], "placements": []}),
            "unknown stage \"Nowhere\"",
        ),
        (
            json!({"stages": stage_names(), "loops": [{"from": "Nowhere", "to": "Idea", "label": ""}], "placements": []}),
            "unknown stage \"Nowhere\"",
        ),
        (
            json!({"stages": stage_names(), "loops": [], "placements": [placement("a1", "stage:Nowhere")]}),
            "unknown stage \"Nowhere\"",
        ),
        (
            json!({"stages": stage_names(), "loops": [], "placements": [placement("a1", "attic")]}),
            "must be stage:<stage>, toolbelt or floor",
        ),
        (
            json!({"stages": stage_names(), "loops": [], "placements": [placement("a1", "floor"), placement("a1", "toolbelt")]}),
            "placed twice",
        ),
        (
            json!({"stages": stage_names(), "loops": [], "placements": [], "shape": "blob"}),
            "never",
        ),
        (
            json!({"stages": [{"name": "Idea"}, {"id": "idea", "name": "Again"}], "loops": [], "placements": []}),
            "appears twice",
        ),
        (
            json!({"stages": (0..13).map(|i| json!({"name": format!("S{i}")})).collect::<Vec<_>>(), "loops": [], "placements": []}),
            "more than 12",
        ),
    ];
    for (answer, expect) in cases {
        if expect == "never" {
            // A non-custom map keeps its template's shape; a bad shape is simply ignored.
            f.claude(&[answer])?;
            assert!(draw(&f).is_ok());
            continue;
        }
        f.claude(&[answer])?;
        match draw(&f) {
            Err(Error::BadAnswer { reasons }) => assert!(
                reasons.iter().any(|r| r.contains(expect)),
                "{expect}: {reasons:?}"
            ),
            other => panic!("{expect}: expected BadAnswer, got {other:?}"),
        }
    }
    Ok(())
}

#[test]
fn an_answer_that_is_not_json_is_retried_like_any_bad_answer() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    f.script("cat > /dev/null\nprintf '%s' 'not json at all'")?;
    assert!(matches!(draw(&f), Err(Error::BadAnswer { .. })));
    assert_eq!(f.draws(), 0);
    Ok(())
}

#[test]
fn over_long_text_is_cut_not_refused() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let mut p = placement("a1", "toolbelt");
    p["note"] = json!("n".repeat(200));
    p["why"] = json!("w".repeat(900));
    f.claude(&[json!({"stages": stage_names(), "loops": [], "placements": [p]})])?;
    draw(&f)?;
    let doc = f.library.harnmap(f.profile)?.doc;
    assert_eq!(doc.placements[0].note.chars().count(), MAX_NOTE);
    assert_eq!(doc.placements[0].why.chars().count(), MAX_WHY);
    Ok(())
}

#[test]
fn pins_and_user_stages_survive_an_answer_that_moved_them() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    // The user pins tdd on Test, adds their own stage "Demo" with review pinned on it, and
    // renames Plan.
    let mut view = f.library.harnmap(f.profile)?;
    let tdd = f.id("tdd");
    let review = f.id("review");
    view.doc.placements.clear();
    let pin = |artifact: &str, zone: Zone, note: &str| Placement {
        artifact: artifact.to_owned(),
        zone,
        order: 0,
        note: note.to_owned(),
        why: String::new(),
        role: String::new(),
        when: String::new(),
        hands_off: String::new(),
        next: Vec::new(),
        by: PlacedBy::User,
        pinned: true,
    };
    view.doc
        .placements
        .push(pin(&tdd, Zone::Stage { id: "test".into() }, "mine"));
    view.doc
        .placements
        .push(pin(&review, Zone::Stage { id: "demo".into() }, "demo it"));
    view.doc.stages.push(Stage {
        id: "demo".into(),
        name: "Demo".into(),
        origin: StageOrigin::User,
    });
    view.doc.stages[2].name = "Planning".into();
    view.doc.stages[2].origin = StageOrigin::User;
    f.library.store.conn.execute(
        "UPDATE harnmap_draws SET map = ?1",
        [serde_json::to_string(&view.doc)?],
    )?;
    // Claude drops Demo, renames Plan back, and moves both pinned artifacts elsewhere.
    f.claude(&[json!({
        "stages": stage_names(),
        "loops": [],
        "placements": [placement("a1", "floor"), placement("a2", "toolbelt"), placement("a4", "stage:Ship")],
    })])?;
    let out = draw(&f)?;
    assert_eq!(out.no, 2);
    let sent = f.stdin(0)?;
    assert!(
        sent.contains("editedByTheUser") && sent.contains("\"pinned\""),
        "{sent}"
    );
    assert!(sent.contains("stage:demo"), "{sent}");
    let doc = f.library.harnmap(f.profile)?.doc;
    let at = |a: &str| {
        doc.placements
            .iter()
            .filter(|p| p.artifact == a)
            .collect::<Vec<_>>()
    };
    assert_eq!(at(&tdd).len(), 1);
    assert_eq!(at(&tdd)[0].zone, Zone::Stage { id: "test".into() });
    assert_eq!(at(&tdd)[0].note, "mine");
    assert!(at(&tdd)[0].pinned);
    assert_eq!(at(&review)[0].zone, Zone::Stage { id: "demo".into() });
    assert_eq!(at(&review).len(), 1);
    let planning = doc.stages.iter().find(|s| s.id == "plan").ok_or("plan")?;
    assert_eq!(planning.name, "Planning");
    let demo_at = doc
        .stages
        .iter()
        .position(|s| s.id == "demo")
        .ok_or("demo")?;
    assert_eq!(demo_at, doc.stages.len() - 1);
    assert!(doc.stages.iter().any(|s| s.id == "demo"));
    Ok(())
}

#[test]
fn a_pin_keeps_the_stage_it_sits_on_even_when_claude_removed_it() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    let mut view = f.library.harnmap(f.profile)?;
    view.doc.placements = vec![Placement {
        artifact: f.id("tdd"),
        zone: Zone::Stage { id: "test".into() },
        order: 0,
        note: String::new(),
        why: String::new(),
        role: String::new(),
        when: String::new(),
        hands_off: String::new(),
        next: Vec::new(),
        by: PlacedBy::User,
        pinned: true,
    }];
    f.library.store.conn.execute(
        "UPDATE harnmap_draws SET map = ?1",
        [serde_json::to_string(&view.doc)?],
    )?;
    let mut stages = stage_names();
    stages.remove(4);
    f.claude(&[json!({"stages": stages, "loops": [], "placements": []})])?;
    draw(&f)?;
    let doc = f.library.harnmap(f.profile)?.doc;
    let at = doc
        .stages
        .iter()
        .position(|s| s.id == "test")
        .ok_or("test kept")?;
    assert_eq!(doc.stages[at - 1].id, "build");
    assert_eq!(doc.placements.len(), 1);
    Ok(())
}

#[test]
fn a_changed_category_starts_from_the_new_template() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    f.library
        .set_category(f.profile, CategoryId::Research, None)?;
    let job = f
        .library
        .prepare_draw(f.profile, "", Arc::new(AtomicBool::new(false)))?;
    let prompt = job.prompt_for_test()?;
    assert!(
        prompt.contains("\"Sources\"") && !prompt.contains("\"Ship\""),
        "{prompt}"
    );
    Ok(())
}

#[test]
fn the_prompt_holds_the_template_the_description_and_facts_but_no_secret_text() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    let sent = f.stdin(0)?;
    assert!(sent.contains("DATA to analyse"));
    assert!(sent.contains("\"category\": \"software\""));
    assert!(sent.contains("\"shape\": \"line\""));
    assert!(sent.contains("Spec first"));
    assert!(sent.contains("\"Build\""));
    assert!(sent.contains("Write the failing test, then the code."));
    assert!(sent.contains("\"id\": \"a1\""));
    assert!(sent.contains("Always be kind."));
    // The flagged skill is listed by kind, name and description; its text never leaves.
    assert!(sent.contains("leaky") && sent.contains("A leaky skill"));
    assert!(sent.contains("textWithheld"));
    assert!(
        !sent.contains("Plain leaky body text"),
        "secret-flagged text sent"
    );
    assert!(!sent.contains("ghp_"), "token sent");
    Ok(())
}

#[test]
fn the_main_file_is_cut_to_4000_characters() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let id = f.id("tdd");
    let folder = f.library.store.artifacts_dir().join(&id);
    let skill = fs::read_dir(&folder)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n == "SKILL.md"))
        .ok_or("SKILL.md")?;
    fs::write(
        &skill,
        format!(
            "---\nname: tdd\ndescription: Test first\n---\n{}",
            "z".repeat(9000)
        ),
    )?;
    draw(&f)?;
    let sent = f.stdin(0)?;
    let z = sent.matches('z').count();
    assert!((3900..=4000).contains(&z), "{z}");
    Ok(())
}

#[test]
fn custom_asks_for_a_shape_and_sends_its_description() -> TestResult {
    let mut f = fixture(CategoryId::Custom, Some("Make a podcast"))?;
    f.claude(&[json!({"stages": [{"id": "open", "name": "Record"}, {"name": "Publish"}], "loops": [], "placements": []})])?;
    match draw(&f) {
        Err(Error::BadAnswer { reasons }) => {
            assert!(
                reasons.iter().any(|r| r.contains("needs a shape")),
                "{reasons:?}"
            );
        }
        other => panic!("expected BadAnswer, got {other:?}"),
    }
    let sent = f.stdin(0)?;
    assert!(sent.contains("Make a podcast") && sent.contains("shapeIsYours"));
    f.claude(&[json!({"shape": "Diamond", "stages": [{"id": "open", "name": "Record"}, {"name": "Publish"}], "loops": [], "placements": [placement("a1", "stage:record")]})])?;
    draw(&f)?;
    let doc = f.library.harnmap(f.profile)?.doc;
    assert_eq!(doc.shape, Shape::Diamond);
    assert_eq!(doc.category, CategoryId::Custom);
    assert_eq!(doc.stages[0].id, "open");
    assert_eq!(doc.stages[1].id, "publish");
    Ok(())
}

#[test]
fn the_model_comes_from_settings_and_is_opus_or_sonnet_only() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let model_arg = |n: usize| -> std::io::Result<String> {
        let args = fs::read_to_string(f.bin.path().join(format!("args-{n}.txt")))?;
        let lines: Vec<&str> = args.lines().collect();
        let at = lines.iter().position(|l| *l == "--model").unwrap_or(0);
        Ok(lines.get(at + 1).copied().unwrap_or_default().to_owned())
    };
    draw(&f)?;
    assert_eq!(model_arg(0)?, "opus");
    f.library.settings_set("harnmap.model", &json!("sonnet"))?;
    assert_eq!(draw(&f)?.model, "sonnet");
    assert_eq!(model_arg(1)?, "sonnet");
    f.library.settings_set("harnmap.model", &json!("haiku"))?;
    assert_eq!(draw(&f)?.model, "opus");
    assert_eq!(draw_model(None), "opus");
    assert_eq!(draw_model(Some(&json!(7))), "opus");
    assert_eq!(draw_model(Some(&json!(" sonnet "))), "sonnet");
    Ok(())
}

#[test]
fn an_empty_profile_is_refused_before_claude_is_asked() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    // Every artifact of the profile left the library: nothing resolves.
    let tdd = f.id("tdd");
    let mut spec = ProfileSpec::default();
    spec.items.push(tdd.clone());
    let empty = f
        .library
        .create_profile("Empty", "", &spec, CategoryId::Software, None)?;
    f.library
        .store
        .conn
        .execute("DELETE FROM artifacts WHERE id = ?1", [&tdd])?;
    assert!(matches!(
        f.library.harnmap_draw(empty, ""),
        Err(Error::EmptyProfile)
    ));
    assert_eq!(f.calls(), 0);
    assert!(matches!(
        f.library.harnmap_draw(9999, ""),
        Err(Error::ProfileNotFound { .. })
    ));
    Ok(())
}

#[test]
fn the_basis_records_the_version_and_every_digest() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    draw(&f)?;
    let text: String =
        f.library
            .store
            .conn
            .query_row("SELECT basis FROM harnmap_draws", [], |r| r.get(0))?;
    let basis: Basis = serde_json::from_str(&text)?;
    assert_eq!(basis.profile_version, 1);
    assert_eq!(basis.artifacts.len(), 4);
    assert!(basis.artifacts.iter().all(|a| !a.digest.is_empty()));
    assert!(basis.artifacts.iter().any(|a| a.id == f.id("tdd")));
    Ok(())
}

fn failure_of(f: &Fixture) -> DrawFailure {
    match draw(f) {
        Err(e) => DrawFailure::from(&e),
        Ok(_) => DrawFailure::Cancelled,
    }
}

#[test]
fn every_failure_kind_is_returned_as_such_and_saves_nothing() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let kind = |failure: DrawFailure| match failure {
        DrawFailure::ClaudeFailed { kind, .. } => Some(kind),
        _ => None,
    };
    f.script("cat > /dev/null\necho 'Not logged in - please run /login' >&2\nexit 1")?;
    assert_eq!(kind(failure_of(&f)), Some(FailureKind::NotLoggedIn));
    f.script("cat > /dev/null\necho 'You have hit your usage limit' >&2\nexit 1")?;
    assert_eq!(kind(failure_of(&f)), Some(FailureKind::Limit));
    f.script("cat > /dev/null\necho 'model opus not found' >&2\nexit 1")?;
    assert_eq!(kind(failure_of(&f)), Some(FailureKind::ModelUnavailable));
    f.script("cat > /dev/null\necho 'boom' >&2\nexit 1")?;
    assert_eq!(kind(failure_of(&f)), Some(FailureKind::Other));
    // Timeout: the job's limit is shortened for the test.
    f.script("cat > /dev/null\nsleep 20")?;
    let mut job = f
        .library
        .prepare_draw(f.profile, "", Arc::new(AtomicBool::new(false)))?;
    job.config.timeout = Duration::from_millis(300);
    let err = job.run().err().ok_or("timeout")?;
    assert_eq!(kind(DrawFailure::from(&err)), Some(FailureKind::Timeout));
    // Missing.
    f.library.claude = config(Path::new("/nonexistent/claude-xyz"));
    assert_eq!(failure_of(&f), DrawFailure::ClaudeMissing);
    assert_eq!(f.draws(), 0);
    Ok(())
}

#[test]
fn other_errors_convert_to_failures() {
    assert_eq!(
        DrawFailure::from(&Error::BadAnswer {
            reasons: vec!["x".into()]
        }),
        DrawFailure::BadAnswer {
            reasons: vec!["x".into()]
        }
    );
    assert_eq!(
        DrawFailure::from(&Error::ClaudeBadOutput {
            message: "m".into()
        }),
        DrawFailure::BadAnswer {
            reasons: vec!["m".into()]
        }
    );
    assert_eq!(DrawFailure::from(&Error::Cancelled), DrawFailure::Cancelled);
    assert_eq!(
        DrawFailure::from(&Error::EmptyProfile),
        DrawFailure::EmptyProfile
    );
    assert!(matches!(
        DrawFailure::from(&Error::NoDataDir),
        DrawFailure::Other { .. }
    ));
}

#[test]
fn cancelling_kills_claude_and_saves_nothing() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let marker = f.bin.path().join("started");
    f.script(&format!(
        "cat > /dev/null\ntouch '{}'\nsleep 30",
        marker.display()
    ))?;
    let registry = DrawRegistry::new();
    let flag = registry.start(f.profile);
    let job = f.library.prepare_draw(f.profile, "", flag)?;
    let started = std::time::Instant::now();
    let result = std::thread::scope(|s| {
        let handle = s.spawn(|| job.run());
        while !marker.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        registry.cancel(f.profile);
        handle.join()
    });
    let result = result.map_err(|_| "worker panicked")?;
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the process was killed"
    );
    assert_eq!(f.draws(), 0);
    registry.finish(f.profile);
    // Cancelling after the run ended (or when nothing runs) does nothing.
    registry.cancel(f.profile);
    Ok(())
}

#[test]
fn a_draw_cancelled_before_it_is_saved_saves_nothing() -> TestResult {
    let f = fixture(CategoryId::Software, None)?;
    let flag = Arc::new(AtomicBool::new(false));
    let job = f.library.prepare_draw(f.profile, "", flag.clone())?;
    let result = job.run()?;
    flag.store(true, Ordering::SeqCst);
    assert!(matches!(
        f.library.save_draw(&job, result),
        Err(Error::Cancelled)
    ));
    assert_eq!(f.draws(), 0);
    // A flag set before the call starts stops it without running claude.
    let calls = f.calls();
    assert!(matches!(job.run(), Err(Error::Cancelled)));
    assert_eq!(f.calls(), calls);
    Ok(())
}

#[test]
fn changes_count_unplaced_and_ignore_nothing() {
    let template = MapDoc::from_template(CategoryId::Software);
    let same = changes(&template, &template, 0);
    assert_eq!(same.len(), template.stages.len());
    assert!(same
        .iter()
        .all(|c| matches!(c, TemplateChange::StageKept { .. })));
    let counted = changes(&template, &template, 2);
    assert_eq!(counted.last(), Some(&TemplateChange::Unplaced { count: 2 }));
}

#[test]
fn the_slug_of_a_new_stage_is_unique() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    f.claude(&[json!({
        "stages": [{"id": "build", "name": "Build"}, {"name": "Build It"}, {"name": "???"}, {"name": "???"}],
        "loops": [],
        "placements": []
    })])?;
    draw(&f)?;
    let doc = f.library.harnmap(f.profile)?.doc;
    let ids: Vec<&str> = doc.stages.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["build", "build-it", "stage", "stage-2"]);
    Ok(())
}

#[test]
fn the_registry_cancels_only_the_profile_asked_for() {
    let registry = DrawRegistry::default();
    let one = registry.start(1);
    let two = registry.start(2);
    registry.cancel(1);
    assert!(one.load(Ordering::SeqCst));
    assert!(!two.load(Ordering::SeqCst));
}

#[test]
fn next_steps_are_kept_as_ids_without_self_unknown_or_repeats() -> TestResult {
    let mut f = fixture(CategoryId::Software, None)?;
    let mut first = placement("a1", "stage:Build");
    first["next"] = json!(["a2", "a1", "a99", "a2", "a4", "a3", "a2"]);
    let mut second = placement("a2", "stage:review");
    second["next"] = json!(["a4"]);
    f.claude(&[json!({
        "stages": stage_names(),
        "loops": [],
        "placements": [first, second, placement("a4", "floor")],
    })])?;
    draw(&f)?;
    let view = f.library.harnmap(f.profile)?;
    let ids: Vec<&str> = view
        .doc
        .placements
        .iter()
        .map(|p| p.artifact.as_str())
        .collect();
    let next = &view.doc.placements[0].next;
    assert_eq!(next.len(), 3, "a2, a4 and a3, in that order: {next:?}");
    assert_eq!(&next[..2], [ids[1], ids[2]]);
    assert!(!next.contains(&view.doc.placements[0].artifact));
    assert_eq!(view.doc.placements[1].next, [ids[2]]);
    // An answer that names no next steps keeps an empty list.
    assert!(view.doc.placements[2].next.is_empty());
    assert!(f.stdin(0)?.contains("natural next steps"));
    Ok(())
}

#[test]
fn at_most_four_next_steps_are_kept() -> TestResult {
    let mut doc_next = vec!["x".to_owned(); 0];
    for n in 0..7 {
        doc_next.push(format!("id{n}"));
    }
    let mut p = Placement {
        artifact: "me".into(),
        zone: Zone::Floor,
        order: 0,
        note: String::new(),
        why: String::new(),
        role: String::new(),
        when: String::new(),
        hands_off: String::new(),
        next: doc_next,
        by: PlacedBy::Claude,
        pinned: false,
    };
    let mut doc = MapDoc::from_template(CategoryId::Software);
    doc.placements.push(p.clone());
    doc.cut_to_limits();
    p.next.truncate(super::MAX_NEXT);
    assert_eq!(doc.placements[0].next, p.next);
    assert_eq!(doc.placements[0].next.len(), 4);
    Ok(())
}
