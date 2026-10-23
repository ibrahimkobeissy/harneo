//! Detection: validation of Claude's answer, what is sent, batching and accepting links.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde_json::{json, Value};

use super::claude_cli_tests::{config, script};
use super::detect::{validate, TEXT_CHARS};
use super::*;
use crate::artifact::ArtifactKind;
use crate::library::{Source, TagChoices};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn validation_drops_everything_that_is_not_allowed() -> TestResult {
    let members = ["a", "b"].into_iter().collect::<HashSet<&str>>();
    let targets = set(&["a", "b", "c"]);
    let known: HashSet<(String, String)> = [("a".to_owned(), "c".to_owned())].into();
    let long = "r".repeat(300);
    let answer = json!({
        "links": [
            {"from": "a", "to": "b", "reason": long, "confidence": "high"},
            {"from": "a", "to": "b", "reason": "again", "confidence": "low"},
            {"from": "a", "to": "a", "reason": "self", "confidence": "high"},
            {"from": "a", "to": "zzz", "reason": "unknown target", "confidence": "high"},
            {"from": "c", "to": "a", "reason": "from outside the batch", "confidence": "high"},
            {"from": "a", "to": "c", "reason": "rejected before", "confidence": "high"},
            {"from": "b", "to": "c", "reason": "ok", "confidence": "surely"},
            {"from": 5, "to": "c"}
        ],
        "tags": [
            {"artifact": "a", "tags": ["review", "Review", "Bad Tag", "has_underscore", "", "x".repeat(41), "git", "ci", "extra"]},
            {"artifact": "zzz", "tags": ["fine"]},
            {"artifact": "b", "tags": ["UPPER"]}
        ]
    });
    let got = validate(&answer, &members, &targets, &known, &["Git".to_owned()])?;
    assert_eq!(got.links.len(), 2);
    assert_eq!(
        (got.links[0].from.as_str(), got.links[0].to.as_str()),
        ("a", "b")
    );
    assert_eq!(got.links[0].reason.chars().count(), 200);
    assert_eq!(got.links[0].confidence, Confidence::High);
    assert_eq!(
        (got.links[1].from.as_str(), got.links[1].to.as_str()),
        ("b", "c")
    );
    assert_eq!(got.links[1].confidence, Confidence::Low);
    // `Review` is not lowercase; `git` takes the spelling of the existing tag; at most 3.
    assert_eq!(
        got.tags,
        vec![TagSuggestion {
            artifact: "a".into(),
            tags: vec!["review".into(), "Git".into(), "ci".into()],
        }]
    );
    Ok(())
}

#[test]
fn an_answer_without_links_and_tags_arrays_is_bad_output() {
    let members = HashSet::new();
    for answer in [
        json!({}),
        json!({"links": []}),
        json!({"links": {}, "tags": []}),
    ] {
        let got = validate(&answer, &members, &HashSet::new(), &HashSet::new(), &[]);
        assert!(
            matches!(got, Err(Error::ClaudeBadOutput { .. })),
            "{answer}"
        );
    }
}

fn write(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

struct Setup {
    data: tempfile::TempDir,
    bin: tempfile::TempDir,
    library: Library,
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    Ok(Setup {
        data,
        bin: tempfile::tempdir()?,
        library,
    })
}

impl Setup {
    /// Points the library's claude at a script that records its stdin and arguments (one file per
    /// call: `in-N.txt`, `args-N.txt`) and prints `answer`.
    fn claude(&mut self, answer: &Value) -> std::io::Result<()> {
        let d = self.bin.path().display();
        let program = script(
            self.bin.path(),
            "claude",
            &format!(
                "n=$(ls '{d}' | grep -c '^in-')\ncat > \"{d}/in-$n.txt\"\nprintf '%s\\n' \"$@\" > \"{d}/args-$n.txt\"\ncat <<'JSON'\n{}\nJSON",
                json!({"is_error": false, "structured_output": answer})
            ),
        )?;
        self.library.claude = config(&program);
        Ok(())
    }

    fn calls(&self) -> std::io::Result<usize> {
        Ok(fs::read_dir(self.bin.path())?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("in-"))
            .count())
    }

    fn stdin(&self, n: usize) -> std::io::Result<String> {
        fs::read_to_string(self.bin.path().join(format!("in-{n}.txt")))
    }
}

fn local(path: &Path) -> Source {
    Source::Local {
        path: path.to_path_buf(),
    }
}

#[test]
fn a_preview_is_sent_without_secrets_and_without_text_of_flagged_artifacts() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    let token = ["gh", "p_", &"a".repeat(36)].concat();
    let api = ["sk", "-ant-", &"b".repeat(30)].concat();
    write(
        src.path(),
        "commands/ship.md",
        "---\nname: ship\ndescription: Ships it\n---\nRun the reviewer agent, then publish.\n",
    )?;
    write(
        src.path(),
        "agents/reviewer.md",
        &format!(
            "---\nname: reviewer\ndescription: Reviews code\n---\nToken {token} stays here.\n"
        ),
    )?;
    write(
        src.path(),
        "settings.json",
        &format!(r#"{{"env":{{"SERVICE_TOKEN":"{api}"}},"model":"x"}}"#),
    )?;
    let preview = s.library.preview(&local(src.path()))?;
    let id_of = |name: &str| {
        preview
            .candidates
            .iter()
            .find(|c| c.artifact.name == name)
            .map(|c| c.id.clone())
    };
    let ship = id_of("ship").ok_or("ship")?;
    let reviewer = id_of("reviewer").ok_or("reviewer")?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.claude(&json!({
        "links": [
            {"from": ship, "to": reviewer, "reason": "calls the reviewer", "confidence": "high"},
            {"from": ship, "to": "ffffffffffffffff", "reason": "ghost", "confidence": "high"}
        ],
        "tags": [{"artifact": ship, "tags": ["release"]}]
    }))?;
    let found = s.library.detect_for_preview(&preview, &ids)?;
    assert_eq!(found.links.len(), 1);
    assert_eq!(found.links[0].reason, "calls the reviewer");
    assert_eq!(found.tags[0].tags, vec!["release".to_owned()]);

    let sent = s.stdin(0)?;
    assert!(
        !sent.contains(&token),
        "secret from a flagged artifact sent"
    );
    assert!(!sent.contains(&api), "secret value sent");
    // The flagged artifact is named but its text is withheld; the clean one is read.
    assert!(sent.contains("reviewer") && sent.contains("Reviews code"));
    assert!(!sent.contains("stays here"));
    assert!(sent.contains("textWithheld"));
    assert!(sent.contains("Run the reviewer agent, then publish."));
    // The model is passed through, Haiku by default.
    let args = fs::read_to_string(s.bin.path().join("args-0.txt"))?;
    assert!(args.contains("--model\nhaiku\n"), "{args}");
    Ok(())
}

#[test]
fn a_library_pass_sends_stored_text_and_withholds_flagged_artifacts() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    let token = ["gh", "p_", &"c".repeat(36)].concat();
    write(
        src.path(),
        "skills/plain/SKILL.md",
        "---\nname: plain\ndescription: A plain skill\n---\nPlain body text.\n",
    )?;
    write(
        src.path(),
        "skills/leaky/SKILL.md",
        &format!("---\nname: leaky\ndescription: Has a token\n---\nUse {token} now.\n"),
    )?;
    write(
        src.path(),
        ".mcp.json",
        r#"{"mcpServers":{"fs":{"command":"npx","args":["fs"],"env":{"LEVEL":"debug"}}}}"#,
    )?;
    let preview = s.library.preview(&local(src.path()))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    s.library.settings_set("bundles.model", &json!("sonnet"))?;
    s.claude(&json!({"links": [], "tags": []}))?;

    let found = s.library.detect_in_library(None)?;
    assert!(found.links.is_empty() && found.tags.is_empty());
    let sent = s.stdin(0)?;
    assert!(sent.contains("Plain body text."));
    assert!(
        sent.contains("npx") && sent.contains("LEVEL"),
        "mcp definition missing"
    );
    assert!(!sent.contains(&token));
    assert!(sent.contains("leaky") && sent.contains("Has a token"));
    assert!(fs::read_to_string(s.bin.path().join("args-0.txt"))?.contains("--model\nsonnet\n"));

    // A subset by id; an unknown id is refused.
    let plain = s
        .library
        .find(ArtifactKind::Skill, "plain")?
        .ok_or("plain")?;
    s.library.detect_in_library(Some(&[plain]))?;
    assert!(matches!(
        s.library.detect_in_library(Some(&["nope".to_owned()])),
        Err(Error::NotInLibrary { .. })
    ));
    Ok(())
}

#[test]
fn an_invalid_model_setting_falls_back_to_haiku() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    write(
        src.path(),
        "agents/a.md",
        "---\nname: a\ndescription: A\n---\nx\n",
    )?;
    let preview = s.library.preview(&local(src.path()))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    s.library
        .settings_set("bundles.model", &json!("--dangerously-skip-permissions"))?;
    s.claude(&json!({"links": [], "tags": []}))?;
    s.library.detect_in_library(None)?;
    assert!(fs::read_to_string(s.bin.path().join("args-0.txt"))?.contains("--model\nhaiku\n"));
    Ok(())
}

#[test]
fn artifacts_go_in_batches_of_twenty_with_progress() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    for n in 0..45 {
        write(
            src.path(),
            &format!("agents/a{n:02}.md"),
            &format!(
                "---\nname: a{n:02}\ndescription: Agent {n}\n---\n{}\n",
                "z".repeat(TEXT_CHARS + 500)
            ),
        )?;
    }
    let preview = s.library.preview(&local(src.path()))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.claude(&json!({"links": [], "tags": []}))?;
    let job = s.library.prepare_preview_detection(&preview, &ids)?;
    assert_eq!(job.artifact_count(), 45);
    let ticks = Mutex::new(Vec::new());
    job.run(&|done, total| {
        if let Ok(mut t) = ticks.lock() {
            t.push((done, total));
        }
    })?;
    assert_eq!(s.calls()?, 3);
    assert_eq!(ticks.into_inner()?, vec![(0, 3), (1, 3), (2, 3), (3, 3)]);
    // The text is cut: 20 artifacts of at most 6,000 characters each.
    assert!(s.stdin(0)?.matches('z').count() <= 20 * TEXT_CHARS);
    // Later batches see the earlier ones in the catalogue.
    assert!(s.stdin(2)?.contains("\"name\": \"a00\""));
    Ok(())
}

#[test]
fn rejected_and_confirmed_pairs_are_not_proposed_again() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    for n in ["a", "b", "c"] {
        write(
            src.path(),
            &format!("agents/{n}.md"),
            &format!("---\nname: {n}\ndescription: {n}\n---\nx\n"),
        )?;
    }
    let preview = s.library.preview(&local(src.path()))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.library.import(&preview, &ids, &TagChoices::new())?;
    let [a, b, c] = [
        s.library.find(ArtifactKind::Agent, "a")?.ok_or("a")?,
        s.library.find(ArtifactKind::Agent, "b")?.ok_or("b")?,
        s.library.find(ArtifactKind::Agent, "c")?.ok_or("c")?,
    ];
    s.library.reject_link(&a, &b)?;
    s.library.add_link(&a, &c, LinkOrigin::Manual, "")?;
    s.claude(&json!({
        "links": [
            {"from": a, "to": b, "reason": "r", "confidence": "high"},
            {"from": a, "to": c, "reason": "r", "confidence": "high"},
            {"from": b, "to": c, "reason": "keep", "confidence": "medium"}
        ],
        "tags": []
    }))?;
    let found = s.library.detect_in_library(None)?;
    assert_eq!(found.links.len(), 1);
    assert_eq!(found.links[0].reason, "keep");

    // Accepting stores `ai` links; bad pairs are skipped; accepting twice stores nothing new.
    let chosen = vec![
        (b.clone(), c.clone(), "keep".to_owned()),
        (b.clone(), b.clone(), "self".to_owned()),
        (b.clone(), "ghost".to_owned(), "unknown".to_owned()),
    ];
    assert_eq!(s.library.accept_links(&chosen)?, 1);
    assert_eq!(s.library.accept_links(&chosen)?, 0);
    let needs = s.library.links_of(&b)?.needs;
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].origin, LinkOrigin::Ai);
    assert_eq!(needs[0].reason, "keep");
    Ok(())
}

#[test]
fn import_stores_the_accepted_links() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    for n in ["x", "y"] {
        write(
            src.path(),
            &format!("agents/{n}.md"),
            &format!("---\nname: {n}\ndescription: {n}\n---\nbody\n"),
        )?;
    }
    let preview = s.library.preview(&local(src.path()))?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    let links = vec![
        (ids[0].clone(), ids[1].clone(), "x needs y".to_owned()),
        (ids[0].clone(), "ghost".to_owned(), "dropped".to_owned()),
        (ids[1].clone(), ids[1].clone(), "self".to_owned()),
    ];
    s.library
        .import_with_links(&preview, &ids, &TagChoices::new(), &links)?;
    let needs = s.library.links_of(&ids[0])?.needs;
    assert_eq!(needs.len(), 1);
    assert_eq!(
        (needs[0].id.as_str(), needs[0].origin),
        (ids[1].as_str(), LinkOrigin::Ai)
    );
    assert!(s.library.links_of(&ids[1])?.needs.is_empty());
    let _ = &s.data;
    Ok(())
}

#[test]
fn a_preview_that_is_not_the_one_given_is_refused() -> TestResult {
    let mut s = setup()?;
    let src = tempfile::tempdir()?;
    write(
        src.path(),
        "agents/a.md",
        "---\nname: a\ndescription: A\n---\nx\n",
    )?;
    let preview = s.library.preview(&local(src.path()))?;
    s.claude(&json!({"links": [], "tags": []}))?;
    let got = s
        .library
        .detect_for_preview(&preview, &["0000000000000000".to_owned()]);
    assert!(matches!(got, Err(Error::NotInPreview { .. })));
    Ok(())
}

#[test]
fn a_git_preview_is_read_from_its_clone_and_the_clone_stays_for_import() -> TestResult {
    let mut s = setup()?;
    let repo = tempfile::tempdir()?;
    let git = |args: &[&str]| -> std::io::Result<()> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["-c", "user.name=T", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()?;
        out.status
            .success()
            .then_some(())
            .ok_or_else(|| std::io::Error::other("git failed"))
    };
    git(&["init", "-q", "-b", "main"])?;
    write(
        repo.path(),
        "agents/g.md",
        "---\nname: g\ndescription: G\n---\nFrom the clone.\n",
    )?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "one"])?;
    let source = Source::parse_git(&format!("file://{}", repo.path().display()), None, None)?;
    let preview = s.library.preview(&source)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    s.claude(&json!({"links": [], "tags": []}))?;
    s.library.detect_for_preview(&preview, &ids)?;
    assert!(s.stdin(0)?.contains("From the clone."));
    // The clone was only read: the import still finds it.
    let report = s.library.import(&preview, &ids, &TagChoices::new())?;
    assert_eq!(report.imported, ids);
    // A preview whose clone is gone cannot be read any more.
    assert!(matches!(
        s.library.detect_for_preview(&preview, &ids),
        Err(Error::PreviewExpired)
    ));
    Ok(())
}
