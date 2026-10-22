//! Workflows: `.claude/workflows/*.js`. The file starts with `export const meta = { … }`, a plain
//! object literal. It is read as text by a small scanner; the JavaScript is never run.

use std::path::Path;

use serde_json::{Map, Number, Value};

use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, Diagnostic, ScanReport};

/// Where Claude Code looks for workflows, relative to a project or `~/.claude`-like root.
const WORKFLOW_DIRS: [&str; 2] = [".claude/workflows", "workflows"];

/// The readable top-level entries of `meta`: `None` when the object cannot be found or closed,
/// otherwise each key with its value when that value is a plain literal.
type Meta = Vec<(String, Option<Value>)>;

/// Pure: reads one workflow from its text. `path` is the file relative to the scan root.
pub(crate) fn parse(path: &Path, contents: &str) -> Artifact {
    let meta = read_meta(contents);
    let mut entries: Map<String, Value> = Map::new();
    for (key, value) in meta.unwrap_or_default() {
        if let Some(v) = value {
            entries.insert(key, v);
        }
    }
    let text = |v: Option<Value>| match v {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s),
        _ => None,
    };
    let declared = text(entries.remove("name"));
    let description = text(entries.remove("description"));

    let mut diagnostics = Vec::new();
    if declared.is_none() || description.is_none() {
        diagnostics.push(Diagnostic::warning(
            "workflow.missing-meta",
            path,
            "a workflow needs `export const meta = { name, description }` with plain string \
             values; Claude Code leaves the workflow out of `/` autocomplete otherwise",
        ));
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    Artifact {
        kind: ArtifactKind::Workflow,
        name: declared.or(stem).unwrap_or_else(|| "unnamed".into()),
        description,
        path: rel_string(path),
        files: vec![rel_string(path)],
        metadata: entries,
        references: Vec::new(),
        diagnostics,
        digest: None,
    }
}

/// Finds the workflows directly inside `.claude/workflows/` and `workflows/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    for dir in WORKFLOW_DIRS {
        let folder = Path::new(dir);
        let mut walk_diagnostics = Vec::new();
        let files = walker.list_files(folder, &mut walk_diagnostics);
        report.diagnostics.extend(walk_diagnostics);
        for file in files {
            let rel = Path::new(&file);
            let direct = rel.parent() == Some(folder);
            if !direct || rel.extension().is_none_or(|e| e != "js") {
                continue;
            }
            let mut diags = Vec::new();
            let Some(text) = walker.read_text(rel, &mut diags) else {
                // Not read: the walk's diagnostic says why.
                report.diagnostics.extend(diags);
                continue;
            };
            let mut artifact = parse(rel, &text);
            artifact.diagnostics.extend(diags);
            report.artifacts.push(artifact);
        }
    }
}

// --- the text scanner -------------------------------------------------------------------------

/// Locates `export const meta = {` outside strings and comments and reads its top-level entries.
fn read_meta(src: &str) -> Option<Meta> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '/' if c.get(i + 1) == Some(&'/') || c.get(i + 1) == Some(&'*') => {
                i = skip_comment(&c, i);
            }
            '"' | '\'' | '`' => i = skip_string(&c, i),
            'e' if starts_word(&c, i, "export") => {
                if let Some(open) = meta_object_start(&c, i + 6) {
                    return read_object(&c, open);
                }
                i += 6;
            }
            _ => i += 1,
        }
    }
    None
}

/// After `export`: `const meta =` then `{`. Returns the index of the `{`.
fn meta_object_start(c: &[char], mut i: usize) -> Option<usize> {
    for word in ["const", "meta"] {
        i = skip_trivia(c, i);
        if !starts_word(c, i, word) {
            return None;
        }
        i += word.len();
    }
    i = skip_trivia(c, i);
    if c.get(i) != Some(&'=') {
        return None;
    }
    i = skip_trivia(c, i + 1);
    (c.get(i) == Some(&'{')).then_some(i)
}

fn starts_word(c: &[char], i: usize, word: &str) -> bool {
    let n = word.chars().count();
    let before_ok = i == 0 || !is_ident(c[i - 1]);
    before_ok
        && c.get(i..i + n)
            .is_some_and(|s| s.iter().copied().eq(word.chars()))
        && c.get(i + n).is_none_or(|ch| !is_ident(*ch))
}

fn is_ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// Skips whitespace and comments.
fn skip_trivia(c: &[char], mut i: usize) -> usize {
    loop {
        match c.get(i) {
            Some(ch) if ch.is_whitespace() => i += 1,
            Some('/') if matches!(c.get(i + 1), Some('/') | Some('*')) => i = skip_comment(c, i),
            _ => return i,
        }
    }
}

/// `i` is at `//` or `/*`; returns the index after the comment.
fn skip_comment(c: &[char], i: usize) -> usize {
    let mut j = i + 2;
    if c[i + 1] == '/' {
        while j < c.len() && c[j] != '\n' {
            j += 1;
        }
        return j;
    }
    while j < c.len() && !(c[j] == '*' && c.get(j + 1) == Some(&'/')) {
        j += 1;
    }
    (j + 2).min(c.len())
}

/// `i` is at a quote; returns the index after the closing quote (or the end when unterminated).
fn skip_string(c: &[char], i: usize) -> usize {
    let quote = c[i];
    let mut j = i + 1;
    while j < c.len() {
        match c[j] {
            '\\' => j += 2,
            ch if ch == quote => return j + 1,
            _ => j += 1,
        }
    }
    c.len()
}

/// `i` is at a quote. The unescaped text when it holds no `${`, else `None`. Also returns the
/// index after the string.
fn read_string(c: &[char], i: usize) -> (Option<String>, usize) {
    let quote = c[i];
    let mut out = String::new();
    let mut interpolated = false;
    let mut j = i + 1;
    while j < c.len() {
        match c[j] {
            '\\' => {
                match c.get(j + 1) {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(&other) => out.push(other),
                    None => {}
                }
                j += 2;
            }
            ch if ch == quote => {
                return ((!interpolated).then_some(out), j + 1);
            }
            '$' if quote == '`' && c.get(j + 1) == Some(&'{') => {
                interpolated = true;
                j += 1;
            }
            ch => {
                out.push(ch);
                j += 1;
            }
        }
    }
    (None, c.len())
}

/// Reads the entries of the object whose `{` is at `open`. `None` when it is never closed.
fn read_object(c: &[char], open: usize) -> Option<Meta> {
    let mut entries = Vec::new();
    let mut i = open + 1;
    loop {
        i = skip_trivia(c, i);
        match c.get(i)? {
            '}' => return Some(entries),
            ',' => i += 1,
            _ => {
                let (key, after_key) = read_key(c, i);
                i = skip_trivia(c, after_key);
                let Some(key) = key.filter(|_| c.get(i) == Some(&':')) else {
                    // A spread, shorthand or method: not a literal entry.
                    i = skip_value(c, i)?;
                    continue;
                };
                i = skip_trivia(c, i + 1);
                let (value, after) = read_value(c, i);
                let end = skip_value(c, after)?;
                // A literal followed by more tokens (`'a' + b`) is not a plain literal.
                let plain = skip_trivia(c, after) >= end
                    || matches!(c.get(skip_trivia(c, after)), Some(',') | Some('}'));
                entries.push((key, value.filter(|_| plain)));
                i = end;
            }
        }
    }
}

/// An identifier or string key. Returns the key (if any) and the index after it.
fn read_key(c: &[char], i: usize) -> (Option<String>, usize) {
    if matches!(c[i], '"' | '\'') {
        return read_string(c, i);
    }
    let mut j = i;
    while j < c.len() && is_ident(c[j]) {
        j += 1;
    }
    if j == i {
        (None, i)
    } else {
        (Some(c[i..j].iter().collect()), j)
    }
}

/// A plain literal at `i` (string, number, `true`, `false`), and the index after it.
fn read_value(c: &[char], i: usize) -> (Option<Value>, usize) {
    match c.get(i) {
        Some('"' | '\'' | '`') => {
            let (s, after) = read_string(c, i);
            (s.map(Value::String), after)
        }
        Some(_) => {
            let mut j = i;
            while j < c.len() && (is_ident(c[j]) || matches!(c[j], '.' | '-' | '+')) {
                j += 1;
            }
            let word: String = c[i..j].iter().collect();
            let value = match word.as_str() {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                w => number(w),
            };
            (value, j)
        }
        None => (None, i),
    }
}

fn number(word: &str) -> Option<Value> {
    if let Ok(n) = word.parse::<i64>() {
        return Some(Value::Number(n.into()));
    }
    word.parse::<f64>()
        .ok()
        .and_then(Number::from_f64)
        .map(Value::Number)
}

/// Skips one value or entry: up to the `,` or `}` at depth 0 (not consumed). `None` when the
/// text ends first.
fn skip_value(c: &[char], mut i: usize) -> Option<usize> {
    let mut depth = 0usize;
    while i < c.len() {
        match c[i] {
            '/' if matches!(c.get(i + 1), Some('/') | Some('*')) => i = skip_comment(c, i),
            '"' | '\'' | '`' => i = skip_string(c, i),
            '{' | '[' | '(' => {
                depth += 1;
                i += 1;
            }
            '}' | ']' | ')' if depth == 0 => return (c[i] == '}').then_some(i),
            '}' | ']' | ')' => {
                depth -= 1;
                i += 1;
            }
            ',' if depth == 0 => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// `CONTEXT.md` §2: Workflows install to `.claude/workflows/<file name>`.
pub(crate) fn install_path(artifact: &crate::artifact::Artifact) -> std::path::PathBuf {
    use std::path::Path;
    Path::new(".claude/workflows").join(super::below(&artifact.path, "workflows"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ScanOptions;
    use std::fs;

    fn wf(src: &str) -> Artifact {
        parse(Path::new(".claude/workflows/audit-routes.js"), src)
    }

    fn codes(a: &Artifact) -> Vec<&str> {
        a.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn reads_name_description_and_plain_keys() {
        let a = wf("export const meta = {\n  name: 'audit',\n  description: \"Audit {all} routes\",\n  version: 2,\n  beta: true,\n}\nawait agent('x')");
        assert_eq!(a.kind, ArtifactKind::Workflow);
        assert_eq!(a.name, "audit");
        assert_eq!(a.description.as_deref(), Some("Audit {all} routes"));
        assert_eq!(a.metadata["version"], 2);
        assert_eq!(a.metadata["beta"], true);
        assert!(!a.metadata.contains_key("name"));
        assert!(a.diagnostics.is_empty());
        assert_eq!(a.path, ".claude/workflows/audit-routes.js");
    }

    #[test]
    fn nested_values_comments_and_braces_in_strings() {
        let a = wf("// export const meta = { name: 'decoy' }\n/* } */\nexport const meta = {\n  phases: [{ title: 'a}' }, { title: \"b{\" }], // } {\n  /* name: 'no', */\n  opts: { deep: { x: [1, 2] } },\n  name: `tpl`,\n  description: 'it\\'s fine',\n};\nconst x = { name: 'later' }");
        assert_eq!(a.name, "tpl");
        assert_eq!(a.description.as_deref(), Some("it's fine"));
        assert!(!a.metadata.contains_key("phases"));
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn missing_meta_triggers_and_present_does_not() {
        let a = wf("const x = 1\nawait agent('x')");
        assert_eq!(codes(&a), ["workflow.missing-meta"]);
        assert_eq!(a.name, "audit-routes");
        assert!(a.description.is_none());
        assert!(codes(&wf("export const meta = { name: 'a', description: 'd' }")).is_empty());
    }

    #[test]
    fn non_literal_values_fall_back_without_failing() {
        let a = wf("const n = 'x'\nexport const meta = { name: n, description: `d ${n}`, ...extra, v: 1 + 2 }");
        assert_eq!(codes(&a), ["workflow.missing-meta"]);
        assert_eq!(a.name, "audit-routes");
        assert!(a.description.is_none());
        assert!(a.metadata.is_empty());
        let half = wf("export const meta = { name: 'a', description: make() }");
        assert_eq!(half.name, "a");
        assert_eq!(codes(&half), ["workflow.missing-meta"]);
    }

    #[test]
    fn unclosed_meta_and_garbage_never_panic() {
        for src in [
            "export const meta = { name: 'a'",
            "export const meta = {",
            "export",
            "export const meta = { 'x",
            "",
            "/*",
            "export const meta = { a: (1,",
        ] {
            assert_eq!(codes(&wf(src)), ["workflow.missing-meta"], "{src}");
        }
    }

    #[test]
    fn scan_finds_both_locations_and_reports_unreadable_files() {
        let tmp = std::env::temp_dir().join(format!("harneo-wf-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join(".claude/workflows/sub")).unwrap();
        fs::create_dir_all(tmp.join("workflows")).unwrap();
        fs::write(
            tmp.join(".claude/workflows/a.js"),
            "export const meta = { name: 'alpha', description: 'd' }",
        )
        .unwrap();
        fs::write(tmp.join(".claude/workflows/notes.md"), "x").unwrap();
        fs::write(tmp.join(".claude/workflows/sub/nested.js"), "x").unwrap();
        fs::write(tmp.join("workflows/b.js"), "await agent('x')").unwrap();
        fs::write(tmp.join("workflows/bad.js"), [0xff, 0xfe, 0x00]).unwrap();

        let options = ScanOptions::default();
        let mut report = ScanReport::default();
        let mut walker = Walker::new(&tmp, &options);
        scan(&mut walker, &mut report);
        report.diagnostics.extend(walker.finish());
        let names: Vec<&str> = report.artifacts.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["alpha", "b"]);
        assert!(report.artifacts[0].diagnostics.is_empty());
        assert_eq!(codes(&report.artifacts[1]), ["workflow.missing-meta"]);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.file.ends_with("bad.js")));
        let _ = fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod install_tests {
    use crate::artifact::{Artifact, ArtifactKind};

    fn install(kind: ArtifactKind, path: &str) -> std::path::PathBuf {
        let artifact = Artifact {
            kind,
            name: "x".into(),
            description: None,
            path: path.into(),
            files: Vec::new(),
            metadata: Default::default(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
        };
        super::install_path(&artifact)
    }

    #[test]
    fn install_path_matches_the_context_table() {
        assert_eq!(
            install(ArtifactKind::Workflow, "workflows/audit.js"),
            std::path::Path::new(".claude/workflows/audit.js")
        );
        assert_eq!(
            install(ArtifactKind::Workflow, ".claude/workflows/audit.js"),
            std::path::Path::new(".claude/workflows/audit.js")
        );
    }
}
