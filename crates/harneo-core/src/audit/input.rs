//! What a rule sees: an artifact's readable files, each with a line index and, for Markdown, the
//! spans of fenced code and inline backticks. This is the only audit code that touches the file
//! system, and it does so through the scan's [`Walker`], so symlinks, secret-named and local
//! files are never opened and the size limits apply.
//!
//! An MCP server shares `.mcp.json` with its siblings, so its definition is rebuilt from the
//! artifact's own `metadata` (secrets already redacted by the scan) instead of reading the file;
//! its findings point at `.mcp.json#<name>`.

use std::ops::Range;
use std::path::{Component, Path};
use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::artifact::walk::Walker;
use crate::artifact::{Artifact, ArtifactKind, Diagnostic, ScanOptions};

/// What a file is, so a rule can decide whether it applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// Prose for the model: CLAUDE.md, rules, skills, commands, agents, styles, workflows.
    Markdown,
    /// A Settings file, or a plugin's hooks configuration.
    Settings,
    /// An MCP server definition.
    McpDefinition,
    /// A script a hook runs (kept with Settings or a plugin's hooks).
    HookScript,
    /// Any other script (shell, Python, JavaScript...).
    Script,
    /// Any other JSON file (a plugin manifest, `package.json`).
    Json,
    Other,
}

/// One line of a file, 1-based, without its line ending.
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    pub number: u32,
    pub text: &'a str,
    /// Byte offset of the line's first character in the file's text.
    pub offset: usize,
}

/// One readable file of an artifact.
#[derive(Debug, Clone)]
pub struct InputFile {
    /// Relative to the scan root, forward slashes.
    pub path: String,
    pub role: FileRole,
    pub text: String,
    line_starts: Vec<usize>,
    fenced: Vec<Range<usize>>,
    inline: Vec<Range<usize>>,
}

impl InputFile {
    pub fn new(path: &str, role: FileRole, text: String) -> Self {
        let line_starts = line_starts(&text);
        let (fenced, inline) = if role == FileRole::Markdown {
            code_spans(&text)
        } else {
            (Vec::new(), Vec::new())
        };
        Self {
            path: path.to_owned(),
            role,
            text,
            line_starts,
            fenced,
            inline,
        }
    }

    pub fn is_markdown(&self) -> bool {
        self.role == FileRole::Markdown
    }
    pub fn is_json(&self) -> bool {
        self.role == FileRole::Json
    }
    pub fn is_settings(&self) -> bool {
        self.role == FileRole::Settings
    }
    pub fn is_mcp_definition(&self) -> bool {
        self.role == FileRole::McpDefinition
    }
    pub fn is_hook_script(&self) -> bool {
        self.role == FileRole::HookScript
    }

    /// Every line, in order.
    pub fn lines(&self) -> impl Iterator<Item = Line<'_>> {
        self.line_starts.iter().enumerate().map(|(i, &start)| {
            let end = self
                .line_starts
                .get(i + 1)
                .copied()
                .unwrap_or(self.text.len());
            let raw = &self.text[start..end];
            let text = raw.strip_suffix('\n').unwrap_or(raw);
            Line {
                number: u32::try_from(i + 1).unwrap_or(u32::MAX),
                text: text.strip_suffix('\r').unwrap_or(text),
                offset: start,
            }
        })
    }

    /// The 1-based line holding byte `offset`.
    pub fn line_of(&self, offset: usize) -> u32 {
        let n = self.line_starts.partition_point(|&s| s <= offset);
        u32::try_from(n.max(1)).unwrap_or(u32::MAX)
    }

    /// The text of 1-based line `number`, or `""` past the end.
    pub fn line_text(&self, number: u32) -> &str {
        let i = usize::try_from(number).unwrap_or(0).saturating_sub(1);
        self.lines().nth(i).map_or("", |l| l.text)
    }

    /// Byte ranges of the content of fenced code blocks (not the fence lines). Markdown only.
    pub fn fenced_spans(&self) -> &[Range<usize>] {
        &self.fenced
    }

    /// Byte ranges of inline-backtick content (not the backticks), outside fenced blocks.
    /// Markdown only.
    pub fn inline_spans(&self) -> &[Range<usize>] {
        &self.inline
    }

    pub fn in_fenced(&self, offset: usize) -> bool {
        within(&self.fenced, offset)
    }

    pub fn in_inline_code(&self, offset: usize) -> bool {
        within(&self.inline, offset)
    }
}

/// The stored file an MCP server's definition lives in.
const STORED_SERVER_FILE: &str = "server.json";

/// What the rules read of a stored `server.json`: the server's own definition when the file is
/// the usual `{"<name>": {...}}`, otherwise the whole text, so extra servers are audited too.
fn definition_text(name: &str, text: String) -> String {
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(mut servers)) if servers.len() == 1 => match servers.remove(name) {
            Some(def @ Value::Object(_)) => serde_json::to_string_pretty(&def).unwrap_or(text),
            _ => text,
        },
        _ => text,
    }
}

/// An artifact as the rules see it.
#[derive(Debug, Clone)]
pub struct AuditInput {
    pub kind: ArtifactKind,
    pub name: String,
    /// The artifact's own path (file, folder or `.mcp.json#<name>`).
    pub path: String,
    pub metadata: Map<String, Value>,
    /// The files that could be read, in the artifact's order.
    pub files: Vec<InputFile>,
    /// Why the others were not (symlink, secret name, too large, not text...). Not findings.
    pub unread: Vec<Diagnostic>,
    /// The artifact's regular files over the per-file limit, binary or not: no rule read them.
    pub oversize: Vec<String>,
    /// Reach inference, computed once per input (see [`AuditInput::trace`]).
    trace: OnceLock<super::reach::Trace>,
}

impl AuditInput {
    /// What the artifact can reach and the lines that show it; inferred on first use, then shared
    /// by the reach result and the `permission-combo` rule.
    pub(crate) fn trace(&self) -> &super::reach::Trace {
        self.trace.get_or_init(|| super::reach::trace(self))
    }

    /// Reads `artifact`'s files below `root` within `options`' limits.
    pub fn read(artifact: &Artifact, root: &Path, options: &ScanOptions) -> Self {
        Self::read_inner(artifact, root, options, false)
    }

    /// Like [`Self::read`] for an artifact's library copy in `root`: an MCP server's definition
    /// is read from the stored `server.json`, the file install writes `.mcp.json` from, never
    /// from the scan record, which a profile file could make say something else.
    pub fn read_stored(artifact: &Artifact, root: &Path, options: &ScanOptions) -> Self {
        Self::read_inner(artifact, root, options, true)
    }

    fn read_inner(artifact: &Artifact, root: &Path, options: &ScanOptions, stored: bool) -> Self {
        let mut walker = Walker::new(root, options);
        let mut unread = Vec::new();
        let mut files = Vec::new();
        let definition_file = artifact.path.split('#').next().unwrap_or_default();
        if artifact.kind == ArtifactKind::Mcp {
            let from_file = if stored {
                walker
                    .read_text(Path::new(STORED_SERVER_FILE), &mut unread)
                    .map(|text| definition_text(&artifact.name, text))
            } else {
                None
            };
            let text = from_file.unwrap_or_else(|| {
                serde_json::to_string_pretty(&Value::Object(artifact.metadata.clone()))
                    .unwrap_or_default()
            });
            files.push(InputFile::new(
                &artifact.path,
                FileRole::McpDefinition,
                text,
            ));
        }
        let mut oversize = Vec::new();
        for rel in &artifact.files {
            if !stays_inside(rel) {
                continue;
            }
            // Never follows a symlink: only a regular file's own size counts.
            if std::fs::symlink_metadata(root.join(rel))
                .is_ok_and(|m| m.file_type().is_file() && m.len() > options.max_file_bytes as u64)
            {
                oversize.push(rel.clone());
            }
            if (artifact.kind == ArtifactKind::Mcp && rel == definition_file) || is_binary_name(rel)
            {
                continue;
            }
            if let Some(text) = walker.read_text(Path::new(rel), &mut unread) {
                let role = classify(artifact, rel);
                files.push(InputFile::new(rel, role, text));
            }
        }
        unread.extend(walker.finish());
        Self {
            kind: artifact.kind,
            name: artifact.name.clone(),
            path: artifact.path.clone(),
            metadata: artifact.metadata.clone(),
            files,
            unread,
            oversize,
            trace: OnceLock::new(),
        }
    }

    /// An input over in-memory files, for rule tests. Roles come from the same classifier.
    #[cfg(test)]
    pub(crate) fn with_files(
        kind: ArtifactKind,
        name: &str,
        path: &str,
        files: &[(&str, &str)],
    ) -> Self {
        let artifact = Artifact {
            kind,
            name: name.to_owned(),
            description: None,
            path: path.to_owned(),
            files: files.iter().map(|(p, _)| (*p).to_owned()).collect(),
            metadata: Map::new(),
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        };
        Self {
            kind,
            name: name.to_owned(),
            path: path.to_owned(),
            metadata: Map::new(),
            files: files
                .iter()
                .map(|(p, t)| InputFile::new(p, classify(&artifact, p), (*t).to_owned()))
                .collect(),
            unread: Vec::new(),
            oversize: Vec::new(),
            trace: OnceLock::new(),
        }
    }

    /// The Markdown files, for rules that apply to prose.
    pub fn markdown_files(&self) -> impl Iterator<Item = &InputFile> {
        self.files.iter().filter(|f| f.is_markdown())
    }

    /// The file at `path`, if it was read.
    pub fn file(&self, path: &str) -> Option<&InputFile> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// True for characters that render as nothing (or reorder text): zero-width, bidi controls,
/// tag characters, variation selectors, the soft hyphen and the byte order mark.
pub fn is_invisible(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{180E}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FE00}'..='\u{FE0F}'
        | '\u{FEFF}'
        | '\u{E0000}'..='\u{E007F}'
        | '\u{E0100}'..='\u{E01EF}'
    )
}

/// Decides a file's role from the artifact's kind and the file's name.
fn classify(artifact: &Artifact, rel: &str) -> FileRole {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase();
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    let is_primary = rel == artifact.path;
    match artifact.kind {
        ArtifactKind::Settings if is_primary => return FileRole::Settings,
        ArtifactKind::Settings => return FileRole::HookScript,
        ArtifactKind::Mcp if ext == "json" => return FileRole::McpDefinition,
        ArtifactKind::Mcp => return FileRole::HookScript,
        _ => {}
    }
    let in_hooks_dir = rel.split('/').any(|c| c == "hooks");
    match (ext, name.as_str()) {
        ("md" | "markdown" | "mdx", _) => FileRole::Markdown,
        ("json", ".mcp.json") => FileRole::McpDefinition,
        ("json", "hooks.json" | "settings.json") => FileRole::Settings,
        ("json", _) => FileRole::Json,
        _ if in_hooks_dir && (ext.is_empty() || is_script_ext(ext)) => FileRole::HookScript,
        _ if is_script_ext(ext) => FileRole::Script,
        _ => FileRole::Other,
    }
}

fn is_script_ext(ext: &str) -> bool {
    matches!(
        ext,
        "sh" | "bash"
            | "zsh"
            | "fish"
            | "py"
            | "js"
            | "mjs"
            | "cjs"
            | "ts"
            | "rb"
            | "pl"
            | "ps1"
            | "bat"
            | "cmd"
    )
}

fn is_binary_name(rel: &str) -> bool {
    let ext = rel
        .rsplit_once('.')
        .map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "pdf"
            | "zip"
            | "gz"
            | "tgz"
            | "tar"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "wasm"
            | "so"
            | "dll"
            | "dylib"
            | "exe"
            | "bin"
            | "mp3"
            | "mp4"
            | "mov"
    )
}

/// A relative path that stays under the root: no `..`, no root, no prefix.
fn stays_inside(rel: &str) -> bool {
    Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    if starts.last() == Some(&text.len()) && starts.len() > 1 {
        starts.pop();
    }
    starts
}

fn within(spans: &[Range<usize>], offset: usize) -> bool {
    let i = spans.partition_point(|r| r.end <= offset);
    spans.get(i).is_some_and(|r| r.start <= offset)
}

/// Fenced blocks (``` or ~~~, closed by a fence of the same character and at least the same
/// length; an unclosed fence runs to the end, as CommonMark does) and inline code spans (a
/// backtick run closed by a run of the same length on the same line).
fn code_spans(text: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let mut fenced = Vec::new();
    let mut inline = Vec::new();
    let mut open: Option<(char, usize, usize)> = None; // (fence char, length, content start)
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        let run = fence_run(trimmed);
        match open {
            Some((ch, len, start)) => {
                let closes =
                    run.is_some_and(|(c, n)| c == ch && n >= len && trimmed[n..].trim().is_empty());
                if closes {
                    fenced.push(start..offset);
                    open = None;
                }
            }
            None => match run {
                Some((c, n)) if !(c == '`' && trimmed[n..].contains('`')) => {
                    open = Some((c, n, offset + raw.len()));
                }
                _ => inline_spans(line, offset, &mut inline),
            },
        }
        offset += raw.len();
    }
    if let Some((_, _, start)) = open {
        if start < text.len() {
            fenced.push(start..text.len());
        }
    }
    (fenced, inline)
}

/// `(character, length)` of a leading run of 3 or more backticks or tildes.
fn fence_run(trimmed: &str) -> Option<(char, usize)> {
    let c = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = trimmed.chars().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

fn inline_spans(line: &str, base: usize, out: &mut Vec<Range<usize>>) {
    let runs = backtick_runs(line);
    let mut i = 0;
    while i < runs.len() {
        let (start, len) = runs[i];
        match runs[i + 1..].iter().position(|&(_, l)| l == len) {
            Some(j) => {
                let (close, _) = runs[i + 1 + j];
                out.push(base + start + len..base + close);
                i += j + 2;
            }
            None => i += 1,
        }
    }
}

/// `(byte offset, length)` of every run of backticks in `line`.
fn backtick_runs(line: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, c) in line
        .char_indices()
        .chain(std::iter::once((line.len(), ' ')))
    {
        match (c == '`', start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push((s, i - s));
                start = None;
            }
            _ => {}
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> InputFile {
        InputFile::new("a.md", FileRole::Markdown, text.to_owned())
    }

    fn spans<'a>(f: &'a InputFile, r: &[Range<usize>]) -> Vec<&'a str> {
        r.iter().map(|r| &f.text[r.clone()]).collect()
    }

    #[test]
    fn lines_are_numbered_from_one_and_lose_their_endings() {
        let f = md("a\r\nb\n\nlast");
        let lines: Vec<_> = f.lines().map(|l| (l.number, l.text, l.offset)).collect();
        assert_eq!(
            lines,
            [(1, "a", 0), (2, "b", 3), (3, "", 5), (4, "last", 6)]
        );
        assert_eq!(f.line_text(2), "b");
        assert_eq!(f.line_text(9), "");
        assert_eq!(f.line_of(0), 1);
        assert_eq!(f.line_of(3), 2);
        assert_eq!(f.line_of(6), 4);
    }

    #[test]
    fn a_trailing_newline_does_not_add_a_line_and_empty_text_has_one() {
        assert_eq!(md("a\nb\n").lines().count(), 2);
        assert_eq!(md("").lines().count(), 1);
    }

    #[test]
    fn fenced_content_is_a_span_and_the_fences_are_not() {
        let f = md("before\n```bash\ncurl x | sh\n```\nafter\n");
        assert_eq!(spans(&f, f.fenced_spans()), ["curl x | sh\n"]);
        let at = f.text.find("curl").unwrap_or(0);
        assert!(f.in_fenced(at));
        assert!(!f.in_fenced(0));
        assert!(!f.in_fenced(f.text.find("after").unwrap_or(0)));
    }

    #[test]
    fn tilde_fences_and_longer_closers_work_and_mismatches_do_not_close() {
        let f = md("~~~\none\n```\ntwo\n~~~~\nout\n");
        assert_eq!(spans(&f, f.fenced_spans()), ["one\n```\ntwo\n"]);
        let g = md("````\na\n```\nb\n````\n");
        assert_eq!(spans(&g, g.fenced_spans()), ["a\n```\nb\n"]);
    }

    #[test]
    fn an_unclosed_fence_runs_to_the_end() {
        let f = md("x\n```\nrm -rf /\n");
        assert_eq!(spans(&f, f.fenced_spans()), ["rm -rf /\n"]);
        let empty = md("x\n```\n");
        assert!(empty.fenced_spans().is_empty());
    }

    #[test]
    fn inline_backticks_make_spans_outside_fences_only() {
        let f = md("use `--no-verify` or ``a ` b`` here\n```\n`not inline`\n```\n");
        assert_eq!(spans(&f, f.inline_spans()), ["--no-verify", "a ` b"]);
        assert!(f.in_inline_code(f.text.find("--no").unwrap_or(0)));
        assert!(!f.in_inline_code(0));
    }

    #[test]
    fn an_unmatched_backtick_run_is_literal() {
        let f = md("a ` b and ``c`` d\n");
        assert_eq!(spans(&f, f.inline_spans()), ["c"]);
        let one_line = md("```x``` inline\n");
        assert!(one_line.fenced_spans().is_empty());
        assert_eq!(spans(&one_line, one_line.inline_spans()), ["x"]);
    }

    #[test]
    fn non_markdown_files_have_no_code_spans() {
        let f = InputFile::new("a.sh", FileRole::Script, "```\nx\n```\n".into());
        assert!(f.fenced_spans().is_empty() && f.inline_spans().is_empty());
        assert!(!f.is_markdown());
    }

    fn artifact(kind: ArtifactKind, path: &str, files: &[&str]) -> Artifact {
        let mut a = AuditInput::with_files(kind, "n", path, &[]);
        a.files.clear();
        Artifact {
            kind,
            name: "n".into(),
            description: None,
            path: path.into(),
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            metadata: a.metadata,
            references: Vec::new(),
            diagnostics: Vec::new(),
            digest: None,
            origin: crate::artifact::Origin::default(),
        }
    }

    #[test]
    fn roles_follow_kind_and_file_name() {
        let role = |kind, path, file| classify(&artifact(kind, path, &[file]), file);
        let s = ArtifactKind::Settings;
        assert_eq!(
            role(s, ".claude/settings.json", ".claude/settings.json"),
            FileRole::Settings
        );
        assert_eq!(
            role(s, ".claude/settings.json", ".claude/hooks/x.sh"),
            FileRole::HookScript
        );
        let m = ArtifactKind::Mcp;
        assert_eq!(role(m, ".mcp.json#x", ".mcp.json"), FileRole::McpDefinition);
        assert_eq!(
            role(m, ".mcp.json#x", "tools/helper.sh"),
            FileRole::HookScript
        );
        let p = ArtifactKind::Plugin;
        assert_eq!(role(p, "plug", "plug/README.md"), FileRole::Markdown);
        assert_eq!(role(p, "plug", "plug/hooks/hooks.json"), FileRole::Settings);
        assert_eq!(role(p, "plug", "plug/.mcp.json"), FileRole::McpDefinition);
        assert_eq!(role(p, "plug", "plug/package.json"), FileRole::Json);
        assert_eq!(role(p, "plug", "plug/hooks/run.sh"), FileRole::HookScript);
        assert_eq!(role(p, "plug", "plug/bin/tool.py"), FileRole::Script);
        assert_eq!(role(p, "plug", "plug/data.yaml"), FileRole::Other);
    }

    #[test]
    fn invisible_characters_are_recognised() {
        for c in [
            '\u{200B}',
            '\u{202E}',
            '\u{2066}',
            '\u{FEFF}',
            '\u{E0041}',
            '\u{FE0F}',
            '\u{00AD}',
        ] {
            assert!(is_invisible(c), "{c:?}");
        }
        assert!(!is_invisible('a') && !is_invisible('é') && !is_invisible('\u{1F600}'));
    }

    #[test]
    fn paths_that_climb_or_are_absolute_are_refused() {
        assert!(stays_inside("a/b.md"));
        assert!(!stays_inside("../a.md"));
        assert!(!stays_inside("/etc/passwd"));
        assert!(!stays_inside("a/../../b"));
    }

    #[test]
    fn binary_names_are_skipped() {
        assert!(is_binary_name("a/logo.PNG"));
        assert!(!is_binary_name("a/run.sh"));
    }
}
