//! Skills: a folder holding `SKILL.md` (YAML frontmatter plus body) and its supporting files.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::artifact::frontmatter::{self, Frontmatter};
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanOptions, ScanReport};

const MANIFEST: &str = "SKILL.md";
/// Where Claude Code looks for skills, relative to a project or `~/.claude`-like root.
const SKILL_DIRS: [&str; 2] = ["skills", ".claude/skills"];

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
    }
}

/// Finds the skills under `skills/` and `.claude/skills/`.
pub(crate) fn scan(root: &Path, options: &ScanOptions, report: &mut ScanReport) {
    for dir in SKILL_DIRS {
        let Ok(entries) = fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut folders: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| Path::new(dir).join(e.file_name()))
            .collect();
        folders.sort();
        for folder in folders {
            read_skill(root, &folder, options, report);
        }
    }
}

fn read_skill(root: &Path, folder: &Path, options: &ScanOptions, report: &mut ScanReport) {
    let manifest = folder.join(MANIFEST);
    let abs = root.join(&manifest);
    if !abs.is_file() || fs::symlink_metadata(&abs).is_ok_and(|m| m.file_type().is_symlink()) {
        return;
    }
    let mut files = Vec::new();
    list_files(root, folder, &options.skip_dirs, &mut files);
    files.sort();
    let too_big = fs::metadata(&abs).map_or(true, |m| m.len() as usize > options.max_file_bytes);
    if too_big {
        report.diagnostics.push(Diagnostic::warning(
            "skill.file-too-large",
            &manifest,
            "SKILL.md is unreadable or over the size limit, so it was not parsed",
        ));
        report.artifacts.push(Artifact {
            diagnostics: Vec::new(),
            ..parse(folder, files, "")
        });
        return;
    }
    let contents = fs::read(&abs).map(|b| String::from_utf8_lossy(&b).into_owned());
    report
        .artifacts
        .push(parse(folder, files, &contents.unwrap_or_default()));
}

/// Lists regular files below `dir` (relative to `root`). Symlinks are never followed or listed.
fn list_files(root: &Path, dir: &Path, skip: &[String], out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let rel = dir.join(entry.file_name());
        if kind.is_dir() {
            if !skip.iter().any(|s| entry.file_name() == s.as_str()) {
                list_files(root, &rel, skip, out);
            }
        } else if kind.is_file() {
            out.push(rel_string(&rel));
        }
    }
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
