//! The only code that touches the file system during a scan. A scanned folder is untrusted, so
//! every read goes through a [`Walker`], which:
//! - uses `symlink_metadata` and never follows a symlink (a symlink is listed as a file with a
//!   warning and its target is never opened or stat'ed);
//! - never opens `settings.local.json`, `CLAUDE.local.md` or a file whose name matches a secret
//!   pattern (it is listed by name with a warning);
//! - reads at most `max_file_bytes` per file and `max_total_bytes` per scan;
//! - skips `ScanOptions::skip_dirs` and stops descending past [`MAX_DEPTH`] levels.
//!
//! Diagnostic codes: `walk.symlink`, `walk.secret-file`, `walk.local-file`, `walk.file-too-large`,
//! `walk.invalid-utf8`, `walk.unreadable`, `walk.too-deep`, `walk.scan-limit-reached`,
//! `walk.absolute-path`, `walk.path-escapes-root`.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use super::{rel_string, Diagnostic, ScanOptions};

/// Hard cap on folder nesting while listing, so a crafted tree cannot recurse without end.
const MAX_DEPTH: usize = 32;

/// Files a project keeps for itself; never read, never part of what is shared.
const LOCAL_FILES: [&str; 2] = ["settings.local.json", "CLAUDE.local.md"];

/// True when the file name matches one of the secret patterns of `CLAUDE.md`'s guardrails.
pub(crate) fn is_secret_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let ends = |suffixes: &[&str]| suffixes.iter().any(|s| n.ends_with(s));
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| n.starts_with(p));
    n == ".env"
        || n.starts_with(".env.")
        || ends(&[
            ".pem",
            ".key",
            ".p12",
            ".pfx",
            ".jks",
            ".keystore",
            ".secret",
        ])
        || starts(&["id_rsa", "id_ed25519", "id_ecdsa", "secrets."])
        || (n.ends_with(".json") && (n.contains("service-account") || n.contains("credentials")))
        || n.contains(".tfstate")
        || matches!(n.as_str(), ".pgpass" | ".my.cnf" | ".vault-token")
}

fn is_local_name(name: &str) -> bool {
    LOCAL_FILES.iter().any(|l| name.eq_ignore_ascii_case(l))
}

/// Adds a diagnostic unless the same one is already there (a file can be met twice: listed, then read).
fn push_unique(out: &mut Vec<Diagnostic>, d: Diagnostic) {
    if !out.contains(&d) {
        out.push(d);
    }
}

/// Reads one scan's files within its limits. Paths in and out are relative to the scan root.
#[derive(Debug)]
pub(crate) struct Walker<'a> {
    root: &'a Path,
    options: &'a ScanOptions,
    total_read: usize,
    exhausted: bool,
    root_diagnostics: Vec<Diagnostic>,
}

impl<'a> Walker<'a> {
    pub(crate) fn new(root: &'a Path, options: &'a ScanOptions) -> Self {
        Self {
            root,
            options,
            total_read: 0,
            exhausted: false,
            root_diagnostics: Vec::new(),
        }
    }

    /// The findings that belong to no single artifact (the scan read limit). Call once at the end.
    pub(crate) fn finish(self) -> Vec<Diagnostic> {
        self.root_diagnostics
    }

    /// True when `rel` exists as anything (a symlink counts, and is not followed).
    pub(crate) fn exists(&self, rel: &Path) -> bool {
        fs::symlink_metadata(self.root.join(rel)).is_ok()
    }

    /// The real sub-folders of `rel`, sorted, as paths relative to the root. Symlinks and
    /// skipped folders are left out.
    pub(crate) fn list_dirs(&self, rel: &Path) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.root.join(rel)) else {
            return Vec::new();
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| !self.is_skipped(&e.file_name().to_string_lossy()))
            .map(|e| rel.join(e.file_name()))
            .collect();
        dirs.sort();
        dirs
    }

    /// Every file below `folder` (relative to the root, forward slashes, sorted). Symlinks,
    /// secret-named and local files are listed by name and reported in `diagnostics`; none of
    /// them is opened. Files over the size limit are listed and reported too.
    pub(crate) fn list_files(
        &self,
        folder: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let mut found = Vec::new();
        self.collect(folder, 0, &mut out, &mut found);
        out.sort();
        // Directory order is arbitrary; reports must be stable.
        found.sort_by(|a, b| (&a.file, &a.code).cmp(&(&b.file, &b.code)));
        for d in found {
            push_unique(diagnostics, d);
        }
        out
    }

    fn collect(
        &self,
        dir: &Path,
        depth: usize,
        out: &mut Vec<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        if depth >= MAX_DEPTH {
            push_unique(
                diags,
                Diagnostic::warning(
                    "walk.too-deep",
                    dir,
                    "folders nested too deeply; the rest of this tree was not listed",
                ),
            );
            return;
        }
        let Ok(entries) = fs::read_dir(self.root.join(dir)) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let rel = dir.join(&name);
            if kind.is_dir() {
                if !self.is_skipped(&name.to_string_lossy()) {
                    self.collect(&rel, depth + 1, out, diags);
                }
                continue;
            }
            out.push(rel_string(&rel));
            if let Some(d) = self.problem_with(&rel, kind.is_symlink()) {
                push_unique(diags, d);
            }
        }
    }

    /// Why a listed file must not be (fully) read, if there is a reason. Never opens the file.
    fn problem_with(&self, rel: &Path, symlink: bool) -> Option<Diagnostic> {
        if symlink {
            return Some(Diagnostic::warning(
                "walk.symlink",
                rel,
                "symlinks are never followed; the target was not read",
            ));
        }
        let name = rel.file_name()?.to_string_lossy();
        if is_secret_name(&name) {
            return Some(Diagnostic::warning(
                "walk.secret-file",
                rel,
                "the name looks like a secret or credential file; it was listed but never read",
            ));
        }
        if is_local_name(&name) {
            return Some(Diagnostic::warning(
                "walk.local-file",
                rel,
                "a local, per-machine file; it was listed but never read",
            ));
        }
        let size = fs::symlink_metadata(self.root.join(rel)).ok()?.len();
        (size > self.options.max_file_bytes as u64).then(|| too_large(rel))
    }

    /// Reads a file as text, within the limits. `None` (with a diagnostic in `diagnostics`) when
    /// it is a symlink, secret or local file, too large, over the scan budget, unreadable or not
    /// valid UTF-8. After the scan budget is spent, nothing more is read.
    pub(crate) fn read_text(
        &mut self,
        rel: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<String> {
        if self.exhausted {
            return None;
        }
        let meta = match fs::symlink_metadata(self.root.join(rel)) {
            Ok(m) => m,
            Err(_) => {
                push_unique(diagnostics, unreadable(rel));
                return None;
            }
        };
        if let Some(d) = self.problem_with(rel, meta.file_type().is_symlink()) {
            push_unique(diagnostics, d);
            return None;
        }
        if !meta.is_file() {
            push_unique(diagnostics, unreadable(rel));
            return None;
        }
        let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        if self.total_read.saturating_add(size) > self.options.max_total_bytes {
            self.exhausted = true;
            push_unique(
                &mut self.root_diagnostics,
                Diagnostic::warning(
                    "walk.scan-limit-reached",
                    Path::new(""),
                    "the read limit for one scan was reached; the remaining files were not read",
                ),
            );
            return None;
        }
        let mut bytes = Vec::new();
        let read = File::open(self.root.join(rel)).and_then(|f| {
            f.take(self.options.max_file_bytes as u64 + 1)
                .read_to_end(&mut bytes)
        });
        if read.is_err() {
            push_unique(diagnostics, unreadable(rel));
            return None;
        }
        if bytes.len() > self.options.max_file_bytes {
            push_unique(diagnostics, too_large(rel));
            return None;
        }
        self.total_read += bytes.len();
        match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(_) => {
                push_unique(
                    diagnostics,
                    Diagnostic::warning(
                        "walk.invalid-utf8",
                        rel,
                        "the file is not valid UTF-8, so it was not parsed",
                    ),
                );
                None
            }
        }
    }

    fn is_skipped(&self, name: &str) -> bool {
        self.options.skip_dirs.iter().any(|s| s == name)
    }
}

fn too_large(rel: &Path) -> Diagnostic {
    Diagnostic::warning(
        "walk.file-too-large",
        rel,
        "the file is over the size limit; it is counted but was not read",
    )
}

fn unreadable(rel: &Path) -> Diagnostic {
    Diagnostic::warning("walk.unreadable", rel, "the file could not be read")
}

/// Resolves a reference written inside an artifact (an `@import`, a hook script path...) against
/// the artifact's `folder`, both relative to the scan root, and returns the path relative to the
/// root. Purely lexical: it never touches the file system and never resolves symlinks.
///
/// Returns an error diagnostic, attached to `folder`, when the reference is absolute
/// (`walk.absolute-path`) or its `..` segments climb out of the scan root
/// (`walk.path-escapes-root`). `..` that stays inside the root is fine. Callers must then not
/// read anything for that reference.
pub fn resolve_reference(folder: &Path, reference: &str) -> Result<PathBuf, Diagnostic> {
    let given = Path::new(reference);
    let absolute = given.is_absolute()
        || reference.starts_with(['/', '\\'])
        || given
            .components()
            .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        || reference.as_bytes().get(1) == Some(&b':');
    if absolute {
        return Err(Diagnostic::error(
            "walk.absolute-path",
            folder,
            &format!("the reference `{reference}` is an absolute path and was not resolved"),
        ));
    }
    let mut parts: Vec<std::ffi::OsString> = folder
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_owned()),
            _ => None,
        })
        .collect();
    for c in given.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_owned()),
            Component::ParentDir if parts.pop().is_none() => {
                return Err(Diagnostic::error(
                    "walk.path-escapes-root",
                    folder,
                    &format!(
                        "the reference `{reference}` leaves the scanned folder and was not resolved"
                    ),
                ));
            }
            _ => {}
        }
    }
    Ok(parts.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_names_match_the_guardrail_patterns() {
        for n in [
            ".env",
            ".env.local",
            "a.pem",
            "x.key",
            "c.p12",
            "c.pfx",
            "s.jks",
            "s.keystore",
            "id_rsa",
            "id_rsa.pub",
            "id_ed25519",
            "id_ecdsa",
            "my-service-account-1.json",
            "aws-credentials.json",
            "secrets.yaml",
            "token.secret",
            "a.tfstate",
            "a.tfstate.backup",
            ".pgpass",
            ".my.cnf",
            ".vault-token",
        ] {
            assert!(is_secret_name(n), "{n}");
        }
        for n in ["SKILL.md", "environment.md", "keys.md", "notes.json", "env"] {
            assert!(!is_secret_name(n), "{n}");
        }
    }

    #[test]
    fn resolve_stays_inside_the_root() {
        let ok = resolve_reference(Path::new("skills/a"), "../b/x.md");
        assert_eq!(ok.ok(), Some(PathBuf::from("skills/b/x.md")));
        let ok = resolve_reference(Path::new("skills/a"), "./sub/x.md");
        assert_eq!(ok.ok(), Some(PathBuf::from("skills/a/sub/x.md")));
    }

    #[test]
    fn resolve_rejects_escapes_and_absolute_paths() {
        let e = resolve_reference(Path::new("skills/a"), "../../../etc/passwd");
        assert_eq!(
            e.err().map(|d| d.code),
            Some("walk.path-escapes-root".into())
        );
        let e = resolve_reference(Path::new(""), "../x");
        assert_eq!(
            e.err().map(|d| d.code),
            Some("walk.path-escapes-root".into())
        );
        for abs in ["/etc/passwd", "\\\\host\\share", "C:\\x"] {
            let e = resolve_reference(Path::new("skills/a"), abs);
            assert_eq!(
                e.err().map(|d| d.code),
                Some("walk.absolute-path".into()),
                "{abs}"
            );
        }
    }
}
