//! Tier 3's mechanical half: reads one item Claude proposed with the parser of the kind it
//! named. The proposal is only a path and a kind; everything stored about the artifact (its
//! name, files and diagnostics) comes from the parser, never from Claude.

use std::path::{Path, PathBuf};

use super::kinds::claude_md::{self, Loaded};
use super::kinds::{agent, command, output_style, rule, settings, skill, workflow};
use crate::artifact::walk::{resolve_reference, Walker};
use crate::artifact::{digest, Artifact, ArtifactKind, ScanOptions, ScanReport, Severity};

/// Checks `path` (as Claude wrote it) against the scanned folder: relative, inside it, and
/// reached without crossing a symlink. Returns the clean relative path.
pub(crate) fn check_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    if path.contains('\\') || path.contains('#') {
        return Err("the path is not a plain relative path".to_owned());
    }
    let rel = resolve_reference(Path::new(""), path)
        .map_err(|_| "the path is absolute or leaves the scanned folder".to_owned())?;
    if rel.as_os_str().is_empty() {
        return Err("the path names the whole repository".to_owned());
    }
    let mut at = root.to_path_buf();
    for part in rel.components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "the path crosses a symlink (`{}`)",
                    at.strip_prefix(root).unwrap_or(&at).display()
                ));
            }
            Ok(_) => {}
            Err(_) => return Err("the path does not exist".to_owned()),
        }
    }
    Ok(rel)
}

fn has_extension(rel: &Path, wanted: &str) -> bool {
    rel.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(wanted))
}

/// Reads a file the walk allows, or says why it did not.
fn read(walker: &mut Walker, rel: &Path) -> Result<String, String> {
    let mut why = Vec::new();
    walker.read_text(rel, &mut why).ok_or_else(|| {
        why.first().map_or_else(
            || "the file could not be read as text".to_owned(),
            |d| d.message.clone(),
        )
    })
}

/// Parses the item `kind` at `rel`, a path [`check_path`] accepted, with that kind's parser.
///
/// `Err` carries the reason it cannot be used: the path escapes the root, crosses a symlink or
/// does not exist, the file is not the shape the kind needs, or the parser reports an error.
/// MCP servers and plugins are never placed this way: they come from `.mcp.json` and from
/// plugin manifests.
pub(crate) fn parse_proposed(
    root: &Path,
    kind: ArtifactKind,
    rel: &Path,
) -> Result<Artifact, String> {
    let rel = rel.to_path_buf();
    let options = ScanOptions::default();
    let mut walker = Walker::new(root, &options);
    let parent = rel.parent().unwrap_or(Path::new("")).to_path_buf();
    let markdown_file = |walker: &Walker| {
        if !walker.is_file(&rel) {
            Err("the path is not a file".to_owned())
        } else if !has_extension(&rel, "md") {
            Err("a Markdown (.md) file is needed".to_owned())
        } else {
            Ok(())
        }
    };
    let artifact = match kind {
        ArtifactKind::Command | ArtifactKind::Agent | ArtifactKind::Rule => {
            markdown_file(&walker)?;
            let text = read(&mut walker, &rel)?;
            match kind {
                ArtifactKind::Command => command::parse(&parent, &rel, &text),
                ArtifactKind::Agent => agent::parse(&parent, &rel, &text),
                _ => rule::parse(&parent, &rel, &text),
            }
        }
        ArtifactKind::OutputStyle => {
            markdown_file(&walker)?;
            let text = read(&mut walker, &rel)?;
            output_style::parse(&parent, &rel, &text)
        }
        ArtifactKind::Workflow => {
            if !walker.is_file(&rel) || !has_extension(&rel, "js") {
                return Err("a JavaScript (.js) file is needed".to_owned());
            }
            let text = read(&mut walker, &rel)?;
            workflow::parse(&rel, &text)
        }
        ArtifactKind::Skill => {
            let folder = if walker.is_dir(&rel) {
                rel.clone()
            } else if rel.file_name().is_some_and(|n| n == "SKILL.md") {
                parent.clone()
            } else {
                return Err("a skill is a folder holding a SKILL.md".to_owned());
            };
            if folder.as_os_str().is_empty() {
                return Err("the path names the whole repository".to_owned());
            }
            let manifest = folder.join("SKILL.md");
            if !walker.is_file(&manifest) {
                return Err("the folder has no SKILL.md".to_owned());
            }
            let text = read(&mut walker, &manifest)?;
            let files = walker.list_files(&folder, &mut Vec::new());
            skill::parse(&folder, files, &text)
        }
        ArtifactKind::ClaudeMd => {
            if !walker.is_file(&rel) || rel.file_name().is_none_or(|n| n != "CLAUDE.md") {
                return Err("a file named CLAUDE.md is needed".to_owned());
            }
            let text = read(&mut walker, &rel)?;
            claude_md::build(&rel, &text, &mut |import| {
                if !walker.exists(import) {
                    return Loaded::Missing;
                }
                match walker.read_text(import, &mut Vec::new()) {
                    Some(t) => Loaded::Text(t),
                    None => Loaded::Unread,
                }
            })
        }
        ArtifactKind::Settings => {
            if !walker.is_file(&rel) || rel.file_name().is_none_or(|n| n != "settings.json") {
                return Err("a file named settings.json is needed".to_owned());
            }
            let text = read(&mut walker, &rel)?;
            let exists = |script: &str| walker.is_file(Path::new(script));
            settings::parse(&rel, &text, &exists)
        }
        ArtifactKind::Mcp => {
            return Err("MCP servers are read from .mcp.json only".to_owned());
        }
        ArtifactKind::Plugin => {
            return Err("plugins are placed by their manifest, not by a proposal".to_owned());
        }
    };
    if let Some(error) = artifact
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(format!("it does not parse: {}", error.message));
    }
    // The parser's name and path stand; only the digest is added, as the scan does.
    let mut report = ScanReport {
        artifacts: vec![artifact],
        ..ScanReport::default()
    };
    digest::fill(&mut walker, &mut report);
    report
        .artifacts
        .pop()
        .ok_or_else(|| "the item could not be read".to_owned())
}
