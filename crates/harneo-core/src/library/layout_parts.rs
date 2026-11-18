//! ADR-008: for every plugin and repo imported, Claude lists each artifact (a part) and the files
//! and folders outside its own folder that it needs, plus what is not needed. Harneo checks each
//! path like tier 3 does (exists, inside the root, no symlink); manifest components are always
//! parts. The answer is stored with the source and replayed by updates without Claude; Claude is
//! asked again only when the file tree changes outside the stored answer. Without Claude, a part
//! needs every file of its plugin's folder that no other part claims.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::layout_claude::{tidy, Claimed, IgnoredPath, SHOWN_CHARS};
use super::Candidate;
use crate::artifact::walk::Walker;
use crate::artifact::{check_path, rel_string, Artifact, Diagnostic, ScanOptions};

/// Diagnostic shown when Claude was not asked.
pub const WITHOUT_CLAUDE: &str = "layout.without-claude";
/// Diagnostic shown when the source's file tree changed outside its stored answer.
pub const TREE_CHANGED: &str = "layout.tree-changed";

/// A file or folder a part needs, with Claude's reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct NeededPath {
    pub path: String,
    pub reason: String,
}

/// The supporting paths of one part: files and folders outside its own that it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PartNeeds {
    /// The part's path, as its candidate has it.
    pub path: String,
    pub needs: Vec<NeededPath>,
}

/// A part naming another part of its plugin by `<plugin>:<name>` or `/<plugin>:<name>` in its
/// text (ADR-008): keeping `from` without `to` leaves a dangling name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PartRef {
    /// The candidate that names the other.
    pub from: String,
    /// The candidate it names.
    pub to: String,
}

/// Who decided the parts' supporting paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum NeedsBy {
    /// Claude's answer, given now.
    Claude,
    /// Claude's answer stored with the source, replayed.
    Stored,
    /// No Claude: every file of the plugin's folder that no other part claims.
    Fallback,
}

/// One part as Claude wrote it, before any check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedPart {
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) path: String,
    pub(super) needs: Vec<NeededPath>,
}

/// What is stored with a source: Claude's answer and the file tree it was given.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StoredParts {
    #[serde(default)]
    pub parts: Vec<PartNeeds>,
    #[serde(default)]
    pub not_needed: Vec<IgnoredPath>,
    #[serde(default)]
    pub tree: Vec<String>,
}

fn text_of(entry: &Value, field: &str) -> String {
    entry
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn pairs(value: Option<&Value>, max: usize) -> Vec<NeededPath> {
    value
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(max)
                .map(|e| NeededPath {
                    path: text_of(e, "path"),
                    reason: text_of(e, "reason"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reads the `parts` and `notNeeded` lists of an answer; both are optional.
pub(super) fn parse_parts(answer: &Value, max: usize) -> (Vec<ProposedPart>, Vec<IgnoredPath>) {
    let parts = answer
        .get("parts")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(max)
                .map(|e| ProposedPart {
                    kind: text_of(e, "kind"),
                    name: text_of(e, "name"),
                    path: text_of(e, "path"),
                    needs: pairs(e.get("needs"), max),
                })
                .collect()
        })
        .unwrap_or_default();
    let not_needed = pairs(answer.get("notNeeded"), max)
        .into_iter()
        .map(|n| IgnoredPath {
            path: n.path,
            reason: n.reason,
        })
        .collect();
    (parts, not_needed)
}

/// The directories down to depth 2 and the files at the root: what "the file tree" means for
/// deciding that Claude must be asked again.
pub(super) fn tree_of(root: &Path) -> Vec<String> {
    let options = ScanOptions::default();
    let walker = Walker::new(root, &options);
    let files = walker.list_files(Path::new(""), &mut Vec::new());
    let mut set = BTreeSet::new();
    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        match parts.len() {
            1 => {
                set.insert(file.clone());
            }
            2 => {
                set.insert(format!("{}/", parts[0]));
            }
            _ => {
                set.insert(format!("{}/", parts[0]));
                set.insert(format!("{}/{}/", parts[0], parts[1]));
            }
        }
    }
    set.into_iter().collect()
}

/// The entries of `now` that the stored tree did not have.
pub(super) fn tree_added(stored: &[String], now: &[String]) -> Vec<String> {
    now.iter()
        .filter(|p| !stored.contains(p))
        .cloned()
        .collect()
}

fn plugin_folder(artifact: &Artifact) -> Option<String> {
    let from = artifact.metadata.get("fromPlugin")?;
    from.get("folder")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn in_folder(path: &str, folder: &str) -> bool {
    folder.is_empty() || folder == "." || Path::new(path).starts_with(folder)
}

/// Without Claude: each plugin part needs every file of its plugin's folder that no other part
/// claims. The plugin's manifest folder is written by the scaffold, so it is not listed.
pub(super) fn fallback(root: &Path, candidates: &[Candidate]) -> Vec<PartNeeds> {
    let options = ScanOptions::default();
    let walker = Walker::new(root, &options);
    let files = walker.list_files(Path::new(""), &mut Vec::new());
    let mut out = Vec::new();
    for candidate in candidates {
        let Some(folder) = plugin_folder(&candidate.artifact) else {
            continue;
        };
        let mut others = Claimed::of(&[]);
        for other in candidates.iter().filter(|o| o.id != candidate.id) {
            if plugin_folder(&other.artifact).as_deref() == Some(folder.as_str()) {
                others.add(&other.artifact, &other.copies);
            }
        }
        let mut own = Claimed::of(&[]);
        own.add(&candidate.artifact, &candidate.copies);
        let needs = files
            .iter()
            .filter(|f| in_folder(f, &folder))
            .filter(|f| !f.contains(".claude-plugin/"))
            .filter(|f| !own.holds(f) && !others.holds(f))
            .map(|f| NeededPath {
                path: f.clone(),
                reason: "the plugin's folder holds it and no other part claims it".to_owned(),
            })
            .collect();
        out.push(PartNeeds {
            path: candidate.artifact.path.clone(),
            needs,
        });
    }
    out
}

fn rejected(path: &str, why: &str) -> Diagnostic {
    Diagnostic::warning(
        "layout.rejected",
        Path::new(&tidy(path, SHOWN_CHARS)),
        &format!("Claude listed this path but it was not used: {why}"),
    )
}

/// Checks each needed path; those that fail are dropped with a `layout.rejected` diagnostic.
/// A path inside the part's own folder is the part's already and is left out quietly.
pub(super) fn check_needs(
    root: &Path,
    part: &Candidate,
    needs: &[NeededPath],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<NeededPath> {
    let mut own = Claimed::of(&[]);
    own.add(&part.artifact, &part.copies);
    let mut kept: Vec<NeededPath> = Vec::new();
    for need in needs {
        match check_path(root, &need.path) {
            Ok(rel) => {
                let shown = rel_string(&rel);
                if own.holds(&shown) || kept.iter().any(|k| k.path == shown) {
                    continue;
                }
                kept.push(NeededPath {
                    path: shown,
                    reason: tidy(&need.reason, 300),
                });
            }
            Err(why) => diagnostics.push(rejected(&need.path, &why)),
        }
    }
    kept
}

/// Checks the paths Claude said are not needed.
pub(super) fn check_not_needed(
    root: &Path,
    listed: &[IgnoredPath],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<IgnoredPath> {
    let mut kept = Vec::new();
    for entry in listed {
        match check_path(root, &entry.path) {
            Ok(rel) => kept.push(IgnoredPath {
                path: rel_string(&rel),
                reason: tidy(&entry.reason, 300),
            }),
            Err(why) => diagnostics.push(rejected(&entry.path, &why)),
        }
    }
    kept
}

/// Replays a stored answer: every path is checked again, and a part no longer in the preview is
/// left out.
pub(super) fn replay(
    root: &Path,
    candidates: &[Candidate],
    stored: &StoredParts,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<PartNeeds> {
    let mut out = Vec::new();
    for part in &stored.parts {
        let Some(candidate) = candidates.iter().find(|c| c.artifact.path == part.path) else {
            continue;
        };
        let needs = check_needs(root, candidate, &part.needs, diagnostics);
        out.push(PartNeeds {
            path: part.path.clone(),
            needs,
        });
    }
    out
}

/// The notice of a preview whose parts were decided without Claude.
pub(super) fn without_claude_notice() -> Diagnostic {
    Diagnostic::info(
        WITHOUT_CLAUDE,
        Path::new(""),
        "Claude was not asked what each part needs, so each part takes every file of its \
plugin's folder that no other part claims",
    )
}

/// The notice listing what changed in the tree since Claude was asked.
pub(super) fn tree_changed_notice(added: &[String]) -> Diagnostic {
    let shown: Vec<String> = added.iter().take(8).map(|p| tidy(p, SHOWN_CHARS)).collect();
    Diagnostic::info(
        TREE_CHANGED,
        Path::new(""),
        &format!(
            "the file tree changed since Claude was asked what each part needs: new {}",
            shown.join(", ")
        ),
    )
}

/// Which parts of each plugin name which, by reading each part's files in `root`.
pub(super) fn refs(root: &Path, candidates: &[Candidate]) -> Vec<PartRef> {
    let options = ScanOptions::default();
    let mut walker = Walker::new(root, &options);
    let mut out = Vec::new();
    for from in candidates {
        let Some((plugin, folder)) = super::plugin_record::plugin_of(&from.artifact) else {
            continue;
        };
        let mut text = String::new();
        for file in &from.artifact.files {
            if let Some(read) = walker.read_text(Path::new(file), &mut Vec::new()) {
                text.push_str(&read);
                text.push('\n');
            }
        }
        let names = crate::plugin::namespaced_uses(&text, &plugin);
        for to in candidates {
            let same =
                super::plugin_record::plugin_of(&to.artifact).is_some_and(|(_, f)| f == folder);
            if same && to.id != from.id && names.contains(&to.artifact.name) {
                out.push(PartRef {
                    from: from.id.clone(),
                    to: to.id.clone(),
                });
            }
        }
    }
    out
}
