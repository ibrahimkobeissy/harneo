//! Rules: one Markdown file per topic under `.claude/rules/` (or `rules/` at a pack root).
//! Subfolders are fine; the whole tree is read. Also home of the helper the other
//! single-file Markdown types (commands, subagents, output styles) share.

use std::path::Path;

use serde_json::{Map, Value};

use crate::artifact::frontmatter::{self, Frontmatter};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, Reference, ScanReport};

const RULE_DIRS: [&str; 2] = ["rules", ".claude/rules"];

/// A single-file type: `base` is the type's folder, `rel` the file, both relative to the root.
pub(super) type Parser = fn(base: &Path, rel: &Path, contents: &str) -> Artifact;

/// Reads every `.md` file below each of `dirs` with `parse`. A file that cannot be read is still
/// listed, with the walk's finding and nothing else.
pub(super) fn scan_markdown(
    walker: &mut Walker,
    dirs: &[&str],
    parse: Parser,
    report: &mut ScanReport,
) {
    for dir in dirs {
        let base = Path::new(dir);
        let mut listing = Vec::new();
        let files = walker.list_files(base, &mut listing);
        for file in files {
            let rel = Path::new(&file);
            let is_md = rel
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_md {
                // Findings about other files in the folder belong to no artifact.
                let own: Vec<Diagnostic> =
                    listing.iter().filter(|d| d.file == file).cloned().collect();
                report.diagnostics.extend(own);
                continue;
            }
            let mut walk_diagnostics = Vec::new();
            let text = walker.read_text(rel, &mut walk_diagnostics);
            let read = text.is_some();
            let mut artifact = parse(base, rel, &text.unwrap_or_default());
            if !read {
                artifact.diagnostics.clear();
            }
            artifact.diagnostics.extend(walk_diagnostics);
            report.artifacts.push(artifact);
        }
    }
}

/// A non-blank string value.
pub(super) fn text(v: Option<Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

/// The file name without its extension.
pub(super) fn stem(rel: &Path) -> String {
    rel.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unnamed".into())
}

/// A list of strings: `Some` when `v` is a list whose items are all non-blank strings.
pub(super) fn string_list(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|i| match i {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// The common front half of every single-file type: frontmatter, its error, name, description.
pub(super) struct Head {
    pub(super) metadata: Map<String, Value>,
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) valid: bool,
}

pub(super) fn head(rel: &Path, contents: &str, invalid_code: &str) -> Head {
    let fm = frontmatter::parse(contents);
    let mut diagnostics = Vec::new();
    let valid = !matches!(fm, Frontmatter::Invalid { .. });
    if let Frontmatter::Invalid { message, .. } = &fm {
        diagnostics.push(Diagnostic::error(invalid_code, rel, message));
    }
    let mut metadata = fm.data();
    let name = text(metadata.remove("name"));
    let description = text(metadata.remove("description"));
    Head {
        metadata,
        name,
        description,
        diagnostics,
        valid,
    }
}

/// Pure: reads one rule. `base` is the rules folder, `rel` the file.
pub(crate) fn parse(_base: &Path, rel: &Path, contents: &str) -> Artifact {
    let mut h = head(rel, contents, "rule.invalid-frontmatter");
    if let Some(paths) = h.metadata.get("paths") {
        if string_list(paths).is_none() {
            h.diagnostics.push(Diagnostic::warning(
                "rule.invalid-paths",
                rel,
                "`paths` must be a list of glob strings; Claude Code will not scope this rule",
            ));
        }
    }
    Artifact {
        kind: ArtifactKind::Rule,
        name: h.name.unwrap_or_else(|| stem(rel)),
        description: h.description,
        path: rel_string(rel),
        files: vec![rel_string(rel)],
        metadata: h.metadata,
        references: Vec::<Reference>::new(),
        diagnostics: h.diagnostics,
        digest: None,
    }
}

/// Finds the rules under `rules/` and `.claude/rules/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    scan_markdown(walker, &RULE_DIRS, parse, report);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(contents: &str) -> Artifact {
        parse(
            Path::new(".claude/rules"),
            Path::new(".claude/rules/api/style.md"),
            contents,
        )
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn name_falls_back_to_the_file_stem_and_paths_are_kept() {
        let a = rule("---\npaths:\n  - \"src/**/*.ts\"\n---\nbody");
        assert_eq!(a.name, "style");
        assert_eq!(a.path, ".claude/rules/api/style.md");
        assert_eq!(a.files, [".claude/rules/api/style.md"]);
        assert_eq!(a.metadata["paths"][0], "src/**/*.ts");
        assert!(a.diagnostics.is_empty());
        assert_eq!(rule("---\nname: house\n---\n").name, "house");
        assert!(rule("no frontmatter").diagnostics.is_empty());
    }

    #[test]
    fn invalid_frontmatter_triggers_and_valid_does_not() {
        assert_eq!(
            codes(&rule("---\nname: [oops\n---\n")),
            ["rule.invalid-frontmatter"]
        );
        assert!(codes(&rule("---\nname: a\n---\n")).is_empty());
    }

    #[test]
    fn invalid_paths_triggers_and_a_list_does_not() {
        assert_eq!(
            codes(&rule("---\npaths: src/**\n---\n")),
            ["rule.invalid-paths"]
        );
        assert_eq!(
            codes(&rule("---\npaths: [1, 2]\n---\n")),
            ["rule.invalid-paths"]
        );
        assert!(codes(&rule("---\npaths: [\"a/**\"]\n---\n")).is_empty());
    }
}
