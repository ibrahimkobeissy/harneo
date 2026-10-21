//! `CLAUDE.md`: project instructions, plus the files it pulls in with `@path` imports.
//!
//! Imports follow the Claude Code docs: `@path` outside Markdown code spans and fenced blocks,
//! resolved relative to the importing file, followed recursively. A path with spaces escapes
//! each space with a backslash; a quoted path is not an import. Imports that leave the scan root
//! (`~/…`, absolute, `..` past the root) are reported and never read.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanReport};

/// Where a CLAUDE.md can sit: a project, pack or `~/.claude` root, or a project's `.claude/`.
const LOCATIONS: [&str; 2] = ["CLAUDE.md", ".claude/CLAUDE.md"];
/// How many levels of imports are followed (the docs allow four hops; the ticket allows five).
pub(crate) const MAX_IMPORT_DEPTH: usize = 5;

/// What reading an imported file gave.
pub(crate) enum Loaded {
    Missing,
    /// Exists but was not read (the walk's diagnostic says why); it still counts as a file.
    Unread,
    Text(String),
}

/// Pure: the raw `@path` imports of a CLAUDE.md text, in order, without duplicates.
pub(crate) fn parse_imports(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some((ch, len)) = fence_of(trimmed) {
            match fence {
                None => fence = Some((ch, len)),
                Some((c, l)) if c == ch && len >= l && trimmed[len..].trim().is_empty() => {
                    fence = None;
                }
                _ => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        for import in line_imports(line) {
            if !out.contains(&import) {
                out.push(import);
            }
        }
    }
    out
}

/// A line opening or closing a fenced block: its fence character and length.
fn fence_of(trimmed: &str) -> Option<(char, usize)> {
    let ch = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = trimmed.chars().take_while(|c| *c == ch).count();
    (len >= 3).then_some((ch, len))
}

/// The imports on one line, skipping inline code spans.
fn line_imports(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '`' => {
                let run = chars[i..].iter().take_while(|c| **c == '`').count();
                i += run;
                // A span closes on a backtick run of the same length; unclosed, it is plain text.
                if let Some(end) = find_run(&chars, i, run) {
                    i = end + run;
                }
            }
            '@' if i == 0 || chars[i - 1].is_whitespace() || "([-*>".contains(chars[i - 1]) => {
                let (path, next) = read_path(&chars, i + 1);
                if let Some(p) = path {
                    out.push(p);
                }
                i = next.max(i + 1);
            }
            _ => i += 1,
        }
    }
    out
}

fn find_run(chars: &[char], from: usize, run: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == '`' {
            let n = chars[i..].iter().take_while(|c| **c == '`').count();
            if n == run {
                return Some(i);
            }
            i += n;
        } else {
            i += 1;
        }
    }
    None
}

/// Reads a path starting after the `@`: it ends at the first unescaped whitespace.
fn read_path(chars: &[char], start: usize) -> (Option<String>, usize) {
    let mut path = String::new();
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&' ') {
            path.push(' ');
            i += 2;
        } else if c.is_whitespace() {
            break;
        } else {
            path.push(c);
            i += 1;
        }
    }
    // Sentence punctuation after a path is not part of it.
    let path = path.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']']);
    let quoted = path.starts_with(['"', '\'']);
    let ok = !path.is_empty() && !quoted;
    (ok.then(|| path.to_string()), i)
}

/// How one import resolves.
enum Target {
    Inside(PathBuf),
    Outside,
}

fn resolve(folder: &Path, import: &str) -> Target {
    if import == "~" || import.starts_with("~/") {
        return Target::Outside;
    }
    match resolve_reference(folder, import) {
        Ok(p) => Target::Inside(p),
        Err(_) => Target::Outside,
    }
}

/// Reads one CLAUDE.md and everything it imports. `load` reads a root-relative file.
pub(crate) fn build(path: &Path, text: &str, load: &mut dyn FnMut(&Path) -> Loaded) -> Artifact {
    let mut files = vec![rel_string(path)];
    let mut diagnostics = Vec::new();
    let mut raw_imports = parse_imports(text);
    let mut seen: HashSet<PathBuf> = HashSet::from([path.to_path_buf()]);
    // (importing file, its imports, depth of those imports)
    let mut queue: Vec<(PathBuf, Vec<String>, usize)> =
        vec![(path.to_path_buf(), raw_imports.clone(), 1)];
    while let Some((from, imports, depth)) = queue.pop() {
        let folder = from.parent().unwrap_or(Path::new("")).to_path_buf();
        for import in imports {
            match resolve(&folder, &import) {
                Target::Outside => diagnostics.push(Diagnostic::warning(
                    "claude-md.import-outside-root",
                    &from,
                    &format!(
                        "the import `@{import}` points outside the scanned folder and was not read"
                    ),
                )),
                Target::Inside(target) => {
                    if !seen.insert(target.clone()) {
                        continue; // already included: also what stops a cycle
                    }
                    match load(&target) {
                        Loaded::Missing => diagnostics.push(Diagnostic::warning(
                            "claude-md.missing-import",
                            &from,
                            &format!("the import `@{import}` does not exist"),
                        )),
                        Loaded::Unread => files.push(rel_string(&target)),
                        Loaded::Text(t) => {
                            files.push(rel_string(&target));
                            if depth < MAX_IMPORT_DEPTH {
                                queue.push((target, parse_imports(&t), depth + 1));
                            }
                        }
                    }
                }
            }
        }
    }
    files.sort();
    diagnostics.sort_by(|a, b| (&a.file, &a.code, &a.message).cmp(&(&b.file, &b.code, &b.message)));
    let mut metadata = serde_json::Map::new();
    if !raw_imports.is_empty() {
        metadata.insert(
            "imports".into(),
            Value::Array(raw_imports.drain(..).map(Value::String).collect()),
        );
    }
    Artifact {
        kind: ArtifactKind::ClaudeMd,
        name: "CLAUDE.md".into(),
        description: None,
        path: rel_string(path),
        files,
        metadata,
        references: Vec::new(),
        diagnostics,
        digest: None,
    }
}

/// Finds `CLAUDE.md` and `.claude/CLAUDE.md`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    for location in LOCATIONS {
        let path = Path::new(location);
        if !walker.exists(path) {
            continue;
        }
        let mut walk_diagnostics = Vec::new();
        let text = walker.read_text(path, &mut walk_diagnostics);
        let mut artifact = build(path, text.as_deref().unwrap_or(""), &mut |rel| {
            if !walker.exists(rel) {
                return Loaded::Missing;
            }
            match walker.read_text(rel, &mut walk_diagnostics) {
                Some(t) => Loaded::Text(t),
                None => Loaded::Unread,
            }
        });
        artifact.diagnostics.extend(walk_diagnostics);
        report.artifacts.push(artifact);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    fn run(files: &[(&str, &str)], root: &str) -> Artifact {
        let map: HashMap<&str, &str> = files.iter().copied().collect();
        let text = map.get(root).copied().unwrap_or("");
        build(
            Path::new(root),
            text,
            &mut |p| match map.get(rel_string(p).as_str()) {
                Some(t) => Loaded::Text((*t).to_string()),
                None => Loaded::Missing,
            },
        )
    }

    #[test]
    fn parses_imports_and_skips_code() {
        let text = "See @README.md and @docs/a.md.\n`@span`\n```\n@fenced\n```\n~~~\n@tilde\n~~~\nmail me@x.com \"@quoted\" @Design\\ Docs/x.md\n- @~/p.md";
        assert_eq!(
            parse_imports(text),
            ["README.md", "docs/a.md", "Design Docs/x.md", "~/p.md"]
        );
        assert!(parse_imports("no imports here").is_empty());
        assert_eq!(parse_imports("`` a ` @b `` @c"), ["c"]);
    }

    #[test]
    fn imported_and_nested_files_join_files() {
        let a = run(
            &[
                ("CLAUDE.md", "@docs/a.md"),
                ("docs/a.md", "@b.md and @../c.md"),
                ("docs/b.md", "x"),
                ("c.md", "y"),
            ],
            "CLAUDE.md",
        );
        assert_eq!(a.files, ["CLAUDE.md", "c.md", "docs/a.md", "docs/b.md"]);
        assert!(a.diagnostics.is_empty());
        assert_eq!(a.metadata["imports"][0], "docs/a.md");
        assert_eq!(a.name, "CLAUDE.md");
    }

    #[test]
    fn missing_import_warns_and_present_does_not() {
        let a = run(&[("CLAUDE.md", "@nope.md")], "CLAUDE.md");
        assert_eq!(codes(&a), ["claude-md.missing-import"]);
        assert_eq!(a.files, ["CLAUDE.md"]);
        let ok = run(&[("CLAUDE.md", "@a.md"), ("a.md", "")], "CLAUDE.md");
        assert!(ok.diagnostics.is_empty());
    }

    #[test]
    fn outside_root_imports_warn_and_are_never_loaded() {
        let a = build(
            Path::new("CLAUDE.md"),
            "@~/x.md @/etc/passwd @../up.md",
            &mut |_| panic!("must not load"),
        );
        assert_eq!(codes(&a), ["claude-md.import-outside-root"; 3]);
        let ok = run(&[("CLAUDE.md", "@a/../b.md"), ("b.md", "")], "CLAUDE.md");
        assert!(ok.diagnostics.is_empty());
    }

    #[test]
    fn cycles_terminate() {
        let a = run(
            &[
                ("CLAUDE.md", "@a.md"),
                ("a.md", "@b.md"),
                ("b.md", "@a.md @CLAUDE.md"),
            ],
            "CLAUDE.md",
        );
        assert_eq!(a.files, ["CLAUDE.md", "a.md", "b.md"]);
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn nesting_stops_at_the_depth_limit() {
        let chain: Vec<(String, String)> = (0..10)
            .map(|i| (format!("f{i}.md"), format!("@f{}.md", i + 1)))
            .collect();
        let mut files: Vec<(&str, &str)> = vec![("CLAUDE.md", "@f0.md")];
        files.extend(chain.iter().map(|(a, b)| (a.as_str(), b.as_str())));
        let a = run(&files, "CLAUDE.md");
        // CLAUDE.md plus imports at depth 1..=5.
        assert_eq!(a.files.len(), 1 + MAX_IMPORT_DEPTH);
    }
}
