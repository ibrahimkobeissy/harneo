//! Skills: a folder holding `SKILL.md` (YAML frontmatter plus body) and its supporting files.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::artifact::frontmatter::{self, Frontmatter};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanReport};

const MANIFEST: &str = "SKILL.md";
/// Where Claude Code looks for skills, relative to a project or `~/.claude`-like root.
/// `.agents/skills` is the cross-agent location of the same SKILL.md format.
const SKILL_DIRS: [&str; 3] = ["skills", ".claude/skills", ".agents/skills"];

/// Pure: reads one skill from its `SKILL.md` text. `folder` is the skill folder relative to the
/// scan root, `files` everything inside it (relative to the root).
pub(crate) fn parse(folder: &Path, files: Vec<String>, contents: &str) -> Artifact {
    let manifest = folder.join(MANIFEST);
    let fm = frontmatter::parse(contents);
    let mut diagnostics = Vec::new();
    if let Frontmatter::Invalid { message, .. } = &fm {
        diagnostics.push(Diagnostic::error(
            "skill.invalid-frontmatter",
            &manifest,
            message,
        ));
    }
    let mut metadata = fm.data();
    let text = |v: Option<Value>| match v {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s),
        _ => None,
    };
    let declared = text(metadata.remove("name"));
    let description = text(metadata.remove("description"));
    let folder_name = folder.file_name().map(|n| n.to_string_lossy().into_owned());

    if !matches!(fm, Frontmatter::Invalid { .. }) {
        if description.is_none() {
            diagnostics.push(Diagnostic::warning(
                "skill.missing-description",
                &manifest,
                "a skill needs a `description`; Claude uses it to decide when to load the skill",
            ));
        }
        if let (Some(d), Some(f)) = (&declared, &folder_name) {
            if d != f {
                diagnostics.push(Diagnostic::warning(
                    "skill.name-mismatch",
                    &manifest,
                    &format!("the `name` is `{d}` but the folder is `{f}`"),
                ));
            }
        }
    }
    Artifact {
        kind: ArtifactKind::Skill,
        name: declared.or(folder_name).unwrap_or_else(|| "unnamed".into()),
        description,
        path: rel_string(folder),
        files,
        metadata,
        references: Vec::new(),
        diagnostics,
        digest: None,
        origin: crate::artifact::Origin::default(),
    }
}

/// Finds the skills under `skills/`, `.claude/skills/` and `.agents/skills/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    for dir in SKILL_DIRS {
        for folder in walker.list_dirs(Path::new(dir)) {
            if is_superseded(walker, &folder, report) {
                continue;
            }
            read_skill(walker, &folder, report);
        }
    }
}

/// True for `.agents/skills/<name>` when `.claude/skills/<name>` is also a skill: Claude's copy
/// is the one listed. Reports the other once as `scan.same-skill-ignored`.
pub(crate) fn is_superseded(walker: &Walker, folder: &Path, report: &mut ScanReport) -> bool {
    let (Some(parent), Some(name)) = (folder.parent(), folder.file_name()) else {
        return false;
    };
    if parent != Path::new(".agents/skills") {
        return false;
    }
    let claude = Path::new(".claude/skills").join(name);
    if !walker.exists(&claude.join(MANIFEST)) {
        return false;
    }
    let d = Diagnostic::info(
        "scan.same-skill-ignored",
        folder,
        &format!("same skill as {}", rel_string(&claude)),
    );
    if !report.diagnostics.contains(&d) {
        report.diagnostics.push(d);
    }
    true
}

pub(crate) fn read_skill(walker: &mut Walker, folder: &Path, report: &mut ScanReport) {
    let manifest = folder.join(MANIFEST);
    if !walker.exists(&manifest) {
        return;
    }
    let mut walk_diagnostics = Vec::new();
    let files = walker.list_files(folder, &mut walk_diagnostics);
    let text = walker.read_text(&manifest, &mut walk_diagnostics);
    let read = text.is_some();
    let mut artifact = parse(folder, files, &text.unwrap_or_default());
    if !read {
        // Not parsed: the walk's diagnostic says why, so no "missing description" on top.
        artifact.diagnostics.clear();
    }
    artifact.diagnostics.extend(walk_diagnostics);
    report.artifacts.push(artifact);
}

/// `CONTEXT.md` §2: skills install to `.claude/skills/<name>/`.
pub(crate) fn install_path(artifact: &Artifact) -> PathBuf {
    Path::new(".claude").join("skills").join(&artifact.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    fn skill(folder: &str, contents: &str) -> Artifact {
        parse(
            Path::new(folder),
            vec![format!("{folder}/SKILL.md")],
            contents,
        )
    }

    #[test]
    fn reads_name_description_and_metadata() {
        let a = skill(
            "skills/tdd",
            "---\nname: tdd\ndescription: Test first\ndisable-model-invocation: true\n---\nbody",
        );
        assert_eq!(a.name, "tdd");
        assert_eq!(a.description.as_deref(), Some("Test first"));
        assert_eq!(a.path, "skills/tdd");
        assert_eq!(a.metadata["disable-model-invocation"], true);
        assert!(!a.metadata.contains_key("name"));
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn invalid_frontmatter_triggers_and_valid_does_not() {
        let bad = skill("skills/a", "---\nname: [oops\n---\n");
        assert_eq!(codes(&bad), ["skill.invalid-frontmatter"]);
        assert_eq!(bad.name, "a");
        let ok = skill("skills/a", "---\ndescription: d\n---\n");
        assert!(!codes(&ok).contains(&"skill.invalid-frontmatter"));
    }

    #[test]
    fn missing_description_triggers_and_present_does_not() {
        assert_eq!(
            codes(&skill("skills/a", "# no frontmatter")),
            ["skill.missing-description"]
        );
        assert_eq!(
            codes(&skill(
                "skills/a",
                "---\nname: a\ndescription: \"  \"\n---\n"
            )),
            ["skill.missing-description"]
        );
        assert!(codes(&skill("skills/a", "---\ndescription: d\n---\n")).is_empty());
    }

    #[test]
    fn name_mismatch_triggers_and_match_or_absence_does_not() {
        let a = skill("skills/a", "---\nname: b\ndescription: d\n---\n");
        assert_eq!(codes(&a), ["skill.name-mismatch"]);
        assert_eq!(a.name, "b");
        assert!(codes(&skill("skills/a", "---\nname: a\ndescription: d\n---\n")).is_empty());
        assert!(codes(&skill("skills/a", "---\ndescription: d\n---\n")).is_empty());
    }

    #[test]
    fn skill_at_the_root_has_a_fallback_name() {
        assert_eq!(skill("", "---\ndescription: d\n---\n").name, "unnamed");
    }

    #[test]
    fn install_path_is_under_claude_skills() {
        let a = skill("skills/tdd", "---\ndescription: d\n---\n");
        assert_eq!(install_path(&a), Path::new(".claude/skills/tdd"));
    }
}
