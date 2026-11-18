//! ADR-008: Claude lists each part and what it needs; the answer is stored and replayed. Claude
//! is a fixture script that counts its calls; the real CLI never runs.

use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::layout_parts::{NeedsBy, WITHOUT_CLAUDE};
use super::*;
use crate::bundles::claude_cli_tests::{config, script};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

struct Setup {
    _base: tempfile::TempDir,
    repo: PathBuf,
    bin: tempfile::TempDir,
    library: Library,
    _data: tempfile::TempDir,
}

/// A repo shaped like `addyosmani/agent-skills`: a plugin at the root, skills linking
/// `../../references/`, a `docs/` folder and commands that call `agent-skills:<skill>`.
fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let base = tempfile::tempdir()?;
    let repo = base.path().join("repo");
    write(
        &repo,
        ".claude-plugin/plugin.json",
        r#"{"name":"agent-skills","version":"1.0.0"}"#,
    )?;
    for skill in ["build", "ship"] {
        write(
            &repo,
            &format!("skills/{skill}/SKILL.md"),
            &format!(
                "---\nname: {skill}\ndescription: {skill} things\n---\nSee [checklist](../../references/checklist.md).\n"
            ),
        )?;
    }
    write(&repo, "references/checklist.md", "- item\n")?;
    write(&repo, "docs/guide.md", "# For people\n")?;
    write(
        &repo,
        ".claude/commands/go.md",
        "---\ndescription: go\n---\nUse agent-skills:build\n",
    )?;
    write(&repo, "README.md", "# agent skills\n")?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let bin = tempfile::tempdir()?;
    let mut s = Setup {
        _base: base,
        repo,
        bin,
        library,
        _data: data,
    };
    s.claude(&json!({"items": [], "ignore": [], "parts": [], "notNeeded": []}))?;
    Ok(s)
}

impl Setup {
    fn source(&self) -> Source {
        Source::Local {
            path: self.repo.clone(),
        }
    }

    /// The plugin at the root, unpacked into its parts.
    fn preview(&self) -> Result<Preview> {
        let options = ScanOptions {
            unpack: vec![String::new()],
            ..ScanOptions::default()
        };
        self.library.preview_with(&self.source(), &options)
    }

    fn calls(&self) -> usize {
        fs::read_dir(self.bin.path())
            .map(|d| {
                d.filter_map(std::result::Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("in-"))
                    .count()
            })
            .unwrap_or(0)
    }

    fn claude(&mut self, answer: &serde_json::Value) -> io::Result<()> {
        let d = self.bin.path().display();
        fs::write(self.bin.path().join("answer.json"), answer.to_string())?;
        let program = script(
            self.bin.path(),
            "claude",
            &format!(
                "n=$(ls '{d}' | grep -c '^in-')\ncat > \"{d}/in-$n.txt\"\n\
printf '{{\"is_error\":false,\"structured_output\":'\ncat '{d}/answer.json'\nprintf '}}'"
            ),
        )?;
        self.library.claude = config(&program);
        Ok(())
    }

    fn import_all(&mut self, preview: &Preview) -> Result<()> {
        let all: Vec<_> = preview.candidates.iter().map(|c| c.id.clone()).collect();
        self.library.import(preview, &all, &TagChoices::new())?;
        Ok(())
    }
}

fn part(path: &str, needs: &[&str]) -> serde_json::Value {
    json!({
        "kind": "skill", "name": path, "path": path,
        "needs": needs.iter().map(|p| json!({"path": p, "reason": "linked"})).collect::<Vec<_>>(),
    })
}

fn needs_of<'a>(preview: &'a Preview, path: &str) -> Vec<&'a str> {
    preview
        .part_needs
        .iter()
        .find(|p| p.path == path)
        .map(|p| p.needs.iter().map(|n| n.path.as_str()).collect())
        .unwrap_or_default()
}

fn skill_paths(preview: &Preview) -> Vec<String> {
    preview
        .candidates
        .iter()
        .filter(|c| c.artifact.kind == crate::artifact::ArtifactKind::Skill)
        .map(|c| c.artifact.path.clone())
        .collect()
}

#[test]
fn claude_gives_each_skill_its_references_and_drops_docs_and_the_answer_is_stored() -> TestResult {
    let mut s = setup()?;
    let preview = s.preview()?;
    let skills = skill_paths(&preview);
    assert_eq!(skills.len(), 2, "{skills:?}");
    s.claude(&json!({
        "items": [], "ignore": [],
        "parts": [part(&skills[0], &["references"]), part(&skills[1], &["references"])],
        "notNeeded": [{"path": "docs", "reason": "for people"}],
    }))?;
    let next = s.library.preview_layout(&preview, &|_| {})?;
    assert_eq!(s.calls(), 1);
    assert_eq!(next.needs_by, Some(NeedsBy::Claude));
    for skill in &skills {
        assert_eq!(needs_of(&next, skill), ["references"]);
    }
    assert_eq!(next.not_needed[0].path, "docs");
    assert!(!next.diagnostics.iter().any(|d| d.code == WITHOUT_CLAUDE));

    s.import_all(&next)?;
    let stored = records::stored_parts(&s.library.store.conn, &ids::source_id(&next.source.identity()?))?;
    assert_eq!(stored.parts.len(), skills.len());
    assert!(!stored.tree.is_empty());
    Ok(())
}

#[test]
fn an_unchanged_tree_makes_no_call_and_a_new_folder_asks_again_and_shows_the_change() -> TestResult {
    let mut s = setup()?;
    let preview = s.preview()?;
    let skills = skill_paths(&preview);
    s.claude(&json!({
        "items": [], "ignore": [],
        "parts": [part(&skills[0], &["references"])],
        "notNeeded": [],
    }))?;
    let next = s.library.preview_layout(&preview, &|_| {})?;
    s.import_all(&next)?;
    assert_eq!(s.calls(), 1);

    // An update with the same tree replays the stored answer: no call.
    let again = s.preview()?;
    assert_eq!(again.needs_by, Some(NeedsBy::Stored));
    assert_eq!(needs_of(&again, &skills[0]), ["references"]);
    let same = s.library.preview_layout(&again, &|_| {})?;
    assert_eq!(s.calls(), 1);
    assert_eq!(same, again);

    // A new top-level folder asks again and says what changed.
    write(&s.repo, "templates/t.md", "t\n")?;
    let changed = s.preview()?;
    assert!(changed
        .diagnostics
        .iter()
        .any(|d| d.code == layout_parts::TREE_CHANGED && d.message.contains("templates/")));
    s.claude(&json!({
        "items": [], "ignore": [],
        "parts": [part(&skills[0], &["references", "templates"])],
        "notNeeded": [],
    }))?;
    let asked = s.library.preview_layout(&changed, &|_| {})?;
    assert_eq!(s.calls(), 2);
    assert_eq!(needs_of(&asked, &skills[0]), ["references", "templates"]);
    Ok(())
}

#[test]
fn without_claude_a_part_needs_the_plugin_folder_minus_other_parts_and_the_import_says_so(
) -> TestResult {
    let s = setup()?;
    let preview = s.preview()?;
    assert_eq!(preview.needs_by, Some(NeedsBy::Fallback));
    assert!(preview.diagnostics.iter().any(|d| d.code == WITHOUT_CLAUDE));
    let skills = skill_paths(&preview);
    let needs = needs_of(&preview, &skills[0]);
    assert!(needs.contains(&"references/checklist.md"), "{needs:?}");
    assert!(needs.contains(&"docs/guide.md"), "{needs:?}");
    assert!(
        !needs.iter().any(|p| p.starts_with("skills/")),
        "other parts' folders are not taken: {needs:?}"
    );
    assert!(!needs.iter().any(|p| p.contains(".claude-plugin")));
    assert_eq!(s.calls(), 0);
    Ok(())
}

#[test]
fn a_bad_path_is_rejected_per_item_and_a_part_the_manifest_lacks_is_a_proposal() -> TestResult {
    let mut s = setup()?;
    symlink("/etc", s.repo.join("link"))?;
    let preview = s.preview()?;
    let skills = skill_paths(&preview);
    s.claude(&json!({
        "items": [], "ignore": [],
        "parts": [
            part(&skills[0], &["../outside", "missing", "link", "references"]),
            part("tools/extra", &["references"]),
        ],
        "notNeeded": [{"path": "/etc/passwd", "reason": "no"}],
    }))?;
    let next = s.library.preview_layout(&preview, &|_| {})?;
    assert_eq!(needs_of(&next, &skills[0]), ["references"]);
    let rejected = next
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.rejected")
        .count();
    assert_eq!(rejected, 4, "{:?}", next.diagnostics);
    assert!(next
        .diagnostics
        .iter()
        .any(|d| d.code == "layout.proposed-part" && d.file == "tools/extra"));
    assert!(next.not_needed.is_empty());
    Ok(())
}
