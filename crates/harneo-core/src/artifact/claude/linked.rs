//! Linked files (ADR-007): files of the source, outside an artifact's own folder, that the
//! artifact's text reaches with a `../` path. They travel with the artifact, one level deep.

use std::path::{Component, Path, PathBuf};

use crate::artifact::walk::Walker;
use crate::artifact::{
    rel_string, AgentAdapter, Artifact, ArtifactKind, ClaudeAdapter, Diagnostic,
};
use crate::artifact::{ScanOptions, ScanReport};
use crate::profiles::layout::in_harness;

/// Text files whose links are followed.
const TEXT_EXTENSIONS: [&str; 4] = ["md", "markdown", "mdx", "txt"];

/// Every path in `text` that starts with `../`, as written (path characters only, trailing dots
/// of a sentence removed).
fn link_tokens(text: &str) -> Vec<&str> {
    let is_path = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '@');
    text.split(|c: char| !is_path(c))
        .filter(|t| t.starts_with("../"))
        .map(|t| t.trim_end_matches('.'))
        .collect()
}

/// `link` resolved from the folder `from_dir` (both relative to the scan root), with `..` and
/// `.` applied. `None` when it climbs out of the root.
fn resolve(from_dir: &Path, link: &str) -> Option<String> {
    let mut parts: Vec<String> = from_dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for component in Path::new(link).components() {
        match component {
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The folder a file artifact's own files live in, or the skill's folder: what a link's position
/// is measured from.
fn source_dir(artifact: &Artifact) -> String {
    let own = artifact.path.split('#').next().unwrap_or_default();
    if artifact.kind == ArtifactKind::Skill {
        own.to_owned()
    } else {
        own.rsplit_once('/').map_or("", |(p, _)| p).to_owned()
    }
}

/// The project path `linked` (a root-relative source file) is installed at for `artifact`: its
/// position relative to the artifact's source folder, applied to the install folder. `None`
/// when that leaves the project or lands outside the harness.
pub(crate) fn project_path(artifact: &Artifact, linked: &str) -> Option<String> {
    let install = ClaudeAdapter.install_path(artifact);
    let install_dir: PathBuf = if artifact.kind == ArtifactKind::Skill {
        install
    } else {
        install.parent()?.to_path_buf()
    };
    let src = source_dir(artifact);
    let src_parts: Vec<&str> = src.split('/').filter(|p| !p.is_empty()).collect();
    let file_parts: Vec<&str> = linked.split('/').collect();
    let dirs = &file_parts[..file_parts.len().saturating_sub(1)];
    let common = src_parts
        .iter()
        .zip(dirs)
        .take_while(|(a, b)| a == b)
        .count();
    let ups = src_parts.len() - common;
    let mut target: Vec<String> = install_dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for _ in 0..ups {
        target.pop()?;
    }
    target.extend(file_parts[common..].iter().map(|s| (*s).to_owned()));
    let path = rel_string(Path::new(&target.join("/")));
    in_harness(&path).then_some(path)
}

fn code_prefix(kind: ArtifactKind) -> Option<&'static str> {
    match kind {
        ArtifactKind::Skill => Some("skill"),
        ArtifactKind::Agent => Some("agent"),
        ArtifactKind::Command => Some("command"),
        ArtifactKind::OutputStyle => Some("output-style"),
        _ => None,
    }
}

/// Fills `linked_files` of every skill, agent, command and output style in the report, and
/// warns about links that would land outside the harness.
pub(crate) fn fill(walker: &mut Walker<'_>, options: &ScanOptions, report: &mut ScanReport) {
    for artifact in &mut report.artifacts {
        let Some(prefix) = code_prefix(artifact.kind) else {
            continue;
        };
        // A plugin's part gets what it needs from Claude's answer, stored with the plugin
        // (ADR-008), not from links in its text.
        if artifact.metadata.contains_key("fromPlugin") {
            continue;
        }
        let own = artifact
            .path
            .split('#')
            .next()
            .unwrap_or_default()
            .to_owned();
        let mut found: Vec<String> = Vec::new();
        for file in artifact.files.clone() {
            let ext = Path::new(&file)
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if !TEXT_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let Some(text) = walker.read_text(Path::new(&file), &mut Vec::new()) else {
                continue;
            };
            let dir = Path::new(&file).parent().unwrap_or(Path::new(""));
            for link in link_tokens(&text) {
                let Some(target) = resolve(dir, link) else {
                    continue;
                };
                let inside_own =
                    artifact.kind == ArtifactKind::Skill && target.starts_with(&format!("{own}/"));
                let skipped = target
                    .split('/')
                    .any(|c| options.skip_dirs.iter().any(|s| s == c));
                if inside_own
                    || skipped
                    || artifact.files.contains(&target)
                    || !walker.is_file(Path::new(&target))
                    || found.contains(&target)
                {
                    continue;
                }
                found.push(target);
            }
        }
        found.sort();
        let mut kept = Vec::new();
        for target in found {
            if project_path(artifact, &target).is_some() {
                kept.push(target);
            } else {
                artifact.diagnostics.push(Diagnostic::warning(
                    &format!("{prefix}.link-outside-harness"),
                    Path::new(&own),
                    &format!(
                        "links to `{target}`, which would land outside the harness, so it is not carried"
                    ),
                ));
            }
        }
        artifact.linked_files = kept;
    }
}
