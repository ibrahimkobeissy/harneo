//! Output styles: `.claude/output-styles/*.md` (or `output-styles/` at a pack root). Every
//! frontmatter field is optional; the name defaults to the file name.

use std::path::Path;

use super::rule::{head, scan_markdown, stem};
use crate::artifact::walk::Walker;
use crate::artifact::{rel_string, Artifact, ArtifactKind, ScanReport};

const STYLE_DIRS: [&str; 2] = ["output-styles", ".claude/output-styles"];

/// Pure: reads one output style.
pub(crate) fn parse(_base: &Path, rel: &Path, contents: &str) -> Artifact {
    let h = head(rel, contents, "output-style.invalid-frontmatter");
    Artifact {
        kind: ArtifactKind::OutputStyle,
        name: h.name.unwrap_or_else(|| stem(rel)),
        description: h.description,
        path: rel_string(rel),
        files: vec![rel_string(rel)],
        metadata: h.metadata,
        references: Vec::new(),
        diagnostics: h.diagnostics,
        digest: None,
    }
}

/// Finds the output styles under `output-styles/` and `.claude/output-styles/`.
pub(crate) fn scan(walker: &mut Walker, report: &mut ScanReport) {
    scan_markdown(walker, &STYLE_DIRS, parse, report);
}

/// `CONTEXT.md` §2: Output styles install to `.claude/output-styles/<file name>`.
pub(crate) fn install_path(artifact: &crate::artifact::Artifact) -> std::path::PathBuf {
    use std::path::Path;
    Path::new(".claude/output-styles").join(super::below(&artifact.path, "output-styles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(contents: &str) -> Artifact {
        parse(
            Path::new(".claude/output-styles"),
            Path::new(".claude/output-styles/terse.md"),
            contents,
        )
    }

    #[test]
    fn reads_name_description_and_flags() {
        let a = style(
            "---\nname: Diagrams first\ndescription: d\nkeep-coding-instructions: true\n---\n",
        );
        assert_eq!(a.name, "Diagrams first");
        assert_eq!(a.description.as_deref(), Some("d"));
        assert_eq!(a.metadata["keep-coding-instructions"], true);
        assert!(a.diagnostics.is_empty());
        assert_eq!(style("just text").name, "terse");
    }

    #[test]
    fn invalid_frontmatter_triggers_and_valid_does_not() {
        let bad = style("---\nname: [oops\n---\n");
        assert_eq!(bad.diagnostics[0].code, "output-style.invalid-frontmatter");
        assert!(style("---\nname: a\n---\n").diagnostics.is_empty());
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
            install(ArtifactKind::OutputStyle, "output-styles/terse.md"),
            std::path::Path::new(".claude/output-styles/terse.md")
        );
        assert_eq!(
            install(ArtifactKind::OutputStyle, ".claude/output-styles/terse.md"),
            std::path::Path::new(".claude/output-styles/terse.md")
        );
    }
}
