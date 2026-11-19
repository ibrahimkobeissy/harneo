//! `harneo update`: check the library's sources, and update what changed without local edits.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{bail, Context};
use clap::Args;
use harneo_core::library::{Library, LibraryItem, Query};
use harneo_core::updates::{
    CheckReport, CheckScope, SourceResult, UpdateChoice, UpdateOutcome, UpdateReport, UpstreamState,
};
use serde::Serialize;

use crate::profiles::open_library;

#[derive(Args)]
pub struct UpdateArgs {
    /// Only check: list what is outdated and change nothing.
    #[arg(long)]
    check: bool,
    /// Print the result as JSON.
    #[arg(long)]
    json: bool,
    /// Also delete the artifacts removed at their source (they are only listed otherwise).
    /// Only for a run over everything, not for named artifacts.
    #[arg(long, conflicts_with_all = ["check", "artifacts"])]
    remove_deleted: bool,
    /// Update only these artifacts (id or name); all outdated ones when none is given.
    artifacts: Vec<String>,
}

/// What the check found for one artifact.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    name: String,
    kind: String,
    /// `outdated`, `removed`, `layout_changed` or `renamed`.
    state: String,
    from_version: Option<String>,
    to_version: Option<String>,
    local_edits: bool,
    /// For `renamed`: the new name in the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    renamed_to: Option<String>,
}

/// A source the check could not read.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceError {
    id: String,
    label: String,
    problem: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckOutput {
    outdated: Vec<Entry>,
    removed: Vec<Entry>,
    /// Renamed at their source; `harneo update` moves everything over to the new name.
    renamed: Vec<Entry>,
    /// How many artifacts the sources hold that the library does not (and did not delete);
    /// `harneo update` imports them.
    new_at_source: u32,
    errors: Vec<SourceError>,
}

/// What happened to one artifact during an update run.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemResult {
    id: String,
    name: String,
    kind: String,
    /// `updated`, `skippedLocalEdits`, `failed`, `notOutdated`, `imported`, `notPlaced`,
    /// `removed` (deleted, as it is gone at its source), `removedUpstream` (gone, kept) or
    /// `renamed` (moved over to its new name, see `newName`).
    outcome: String,
    from_version: Option<String>,
    to_version: Option<String>,
    message: Option<String>,
    /// For `renamed`: the new name.
    #[serde(skip_serializing_if = "Option::is_none")]
    new_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    updated: u32,
    skipped: u32,
    failed: u32,
    /// Artifacts new at their source, now in the library.
    imported: u32,
    /// New at their source, but the stored layout cannot place them.
    not_placed: u32,
    /// Removed at their source and deleted (`--remove-deleted`).
    removed: u32,
    /// Renamed at their source, now under the new name.
    renamed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateOutput {
    results: Vec<ItemResult>,
    errors: Vec<SourceError>,
    summary: Summary,
}

fn label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn all_items(library: &Library) -> anyhow::Result<Vec<LibraryItem>> {
    let query = Query {
        limit: harneo_core::library::MAX_LIMIT,
        ..Query::default()
    };
    Ok(library.list(&query)?.items)
}

/// Resolves what the user typed: an exact id, else a name (case-insensitive).
fn resolve<'a>(items: &'a [LibraryItem], input: &str) -> anyhow::Result<&'a LibraryItem> {
    if let Some(item) = items.iter().find(|i| i.id == input) {
        return Ok(item);
    }
    let wanted = input.to_lowercase();
    let matches: Vec<&LibraryItem> = items
        .iter()
        .filter(|i| i.name.to_lowercase() == wanted)
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => bail!("no artifact named or with id \"{input}\" in the library"),
        many => {
            let list: Vec<String> = many
                .iter()
                .map(|i| format!("{} ({}, id {})", i.name, label(&i.kind), i.id))
                .collect();
            bail!(
                "\"{input}\" is ambiguous, use the id of one of: {}",
                list.join("; ")
            )
        }
    }
}

fn source_errors(report: &CheckReport, items: &[LibraryItem]) -> Vec<SourceError> {
    let labels: HashMap<&str, &str> = items
        .iter()
        .map(|i| (i.source.id.as_str(), i.source.label.as_str()))
        .collect();
    report
        .sources
        .iter()
        .filter_map(|s| {
            let problem = match &s.result {
                SourceResult::Unreachable { reason } => format!("unreachable: {reason}"),
                SourceResult::Missing => "the source folder is gone".to_owned(),
                SourceResult::Current | SourceResult::Changed { .. } => return None,
            };
            Some(SourceError {
                id: s.id.clone(),
                label: labels
                    .get(s.id.as_str())
                    .copied()
                    .unwrap_or(&s.id)
                    .to_owned(),
                problem,
            })
        })
        .collect()
}

/// New artifacts the check found, in every source it could read.
fn new_at_source(report: &CheckReport) -> u32 {
    report
        .sources
        .iter()
        .map(|s| match s.result {
            SourceResult::Changed { new_upstream, .. } => new_upstream,
            _ => 0,
        })
        .sum()
}

type Entries = (Vec<Entry>, Vec<Entry>, Vec<Entry>);

/// The flagged artifacts: outdated, gone at the source (removed or layout changed), renamed.
fn entries(library: &Library, items: &[LibraryItem]) -> anyhow::Result<Entries> {
    let (mut outdated, mut removed, mut renamed) = (Vec::new(), Vec::new(), Vec::new());
    for item in items {
        let Some(status) = library.update_status(&item.id)? else {
            continue;
        };
        let state = match status.state {
            UpstreamState::Outdated => "outdated",
            UpstreamState::Removed => "removed",
            UpstreamState::LayoutChanged => "layout_changed",
            UpstreamState::Renamed => "renamed",
            UpstreamState::Skipped => continue,
        };
        let entry = Entry {
            id: item.id.clone(),
            name: item.name.clone(),
            kind: label(&item.kind),
            state: state.to_owned(),
            from_version: status.from_version,
            to_version: status.to_version,
            local_edits: status.local_edits,
            renamed_to: status.renamed_to,
        };
        match status.state {
            UpstreamState::Outdated => outdated.push(entry),
            UpstreamState::Renamed => renamed.push(entry),
            _ => removed.push(entry),
        }
    }
    Ok((outdated, removed, renamed))
}

fn versions(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (None, None) => "changed".to_owned(),
        (f, t) => format!("{} → {}", f.unwrap_or("?"), t.unwrap_or("?")),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn render_errors(out: &mut String, errors: &[SourceError]) {
    for e in errors {
        let _ = writeln!(out, "source {} ({}): {}", e.label, e.id, e.problem);
    }
}

fn render_check(check: &CheckOutput) -> String {
    let mut out = String::new();
    for e in &check.outdated {
        let edits = if e.local_edits { "  [local edits]" } else { "" };
        let _ = writeln!(
            out,
            "{} ({})  {}{edits}",
            e.name,
            e.kind,
            versions(e.from_version.as_deref(), e.to_version.as_deref())
        );
    }
    for e in &check.removed {
        let what = if e.state == "layout_changed" {
            "layout changed upstream"
        } else {
            "removed upstream"
        };
        let _ = writeln!(out, "{} ({})  {what}", e.name, e.kind);
    }
    for e in &check.renamed {
        let edits = if e.local_edits { "  [local edits]" } else { "" };
        let _ = writeln!(
            out,
            "{} ({})  renamed at source: {} → {}{edits}",
            e.name,
            e.kind,
            e.name,
            e.renamed_to.as_deref().unwrap_or("?")
        );
    }
    render_errors(&mut out, &check.errors);
    if check.new_at_source > 0 {
        let _ = writeln!(out, "{} new at source", check.new_at_source);
    }
    let _ = writeln!(
        out,
        "{} outdated, {} removed upstream, {} renamed, {}",
        check.outdated.len(),
        check.removed.len(),
        check.renamed.len(),
        plural(check.errors.len(), "source error", "source errors"),
    );
    out
}

fn render_update(output: &UpdateOutput) -> String {
    let mut out = String::new();
    for r in &output.results {
        let range = versions(r.from_version.as_deref(), r.to_version.as_deref());
        match r.outcome.as_str() {
            "updated" => {
                let _ = writeln!(out, "updated {} ({})  {range}", r.name, r.kind);
            }
            "skippedLocalEdits" => {
                let _ = writeln!(
                    out,
                    "skipped {} ({})  {range}: has local edits, update it in the app to choose",
                    r.name, r.kind
                );
            }
            "failed" => {
                let _ = writeln!(
                    out,
                    "failed {} ({}): {}",
                    r.name,
                    r.kind,
                    r.message.as_deref().unwrap_or("unknown error")
                );
            }
            "imported" => {
                let _ = writeln!(out, "imported {} ({})  new at source", r.name, r.kind);
            }
            "removed" => {
                let _ = writeln!(out, "deleted {} ({})  removed at source", r.name, r.kind);
            }
            "removedUpstream" => {
                let _ = writeln!(
                    out,
                    "kept {} ({})  removed at source: run with --remove-deleted to delete it",
                    r.name, r.kind
                );
            }
            "renamed" => {
                let _ = writeln!(
                    out,
                    "renamed {} ({})  renamed at source: {} → {}, with its tags, profiles and usage",
                    r.name,
                    r.kind,
                    r.name,
                    r.new_name.as_deref().unwrap_or("?")
                );
            }
            "notPlaced" => {
                let _ = writeln!(
                    out,
                    "not placed {}: {}",
                    r.name,
                    r.message
                        .as_deref()
                        .unwrap_or("the stored layout cannot place it")
                );
            }
            _ => {
                let _ = writeln!(out, "{} ({}): nothing to update", r.name, r.kind);
            }
        }
    }
    render_errors(&mut out, &output.errors);
    let s = &output.summary;
    if s.imported + s.not_placed > 0 {
        let _ = writeln!(
            out,
            "Imported {} new at source, {} not placed",
            s.imported, s.not_placed
        );
    }
    if s.removed > 0 {
        let _ = writeln!(out, "Deleted {} removed at source", s.removed);
    }
    if s.renamed > 0 {
        let _ = writeln!(out, "Renamed {} at source", s.renamed);
    }
    let _ = writeln!(
        out,
        "Updated {}, skipped {} (local edits), failed {}",
        s.updated, s.skipped, s.failed
    );
    out
}

fn result_of(item: &LibraryItem, report: UpdateReport) -> ItemResult {
    let outcome = match report.outcome {
        UpdateOutcome::Imported => "imported",
        UpdateOutcome::NotPlaced => "notPlaced",
        UpdateOutcome::Removed => "removed",
        UpdateOutcome::Renamed => "renamed",
        UpdateOutcome::Updated => "updated",
        UpdateOutcome::SkippedLocalEdits => "skippedLocalEdits",
        UpdateOutcome::Failed => "failed",
        UpdateOutcome::Kept => "notOutdated",
    };
    ItemResult {
        id: item.id.clone(),
        name: item.name.clone(),
        kind: label(&item.kind),
        outcome: outcome.to_owned(),
        from_version: report.from_version,
        to_version: report.to_version,
        message: report.message,
        new_name: (report.outcome == UpdateOutcome::Renamed)
            .then_some(report.name)
            .flatten(),
    }
}

/// A result for an artifact the library does not hold (a new one that could not be placed).
fn not_in_library(report: UpdateReport) -> ItemResult {
    ItemResult {
        id: report.id,
        name: report.name.or(report.source_path).unwrap_or_default(),
        kind: String::new(),
        outcome: "notPlaced".to_owned(),
        from_version: None,
        to_version: None,
        message: report.message,
        new_name: None,
    }
}

fn print_json<T: Serialize>(out: &mut impl Write, value: &T) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(value).context("cannot serialize the result")?;
    writeln!(out, "{text}")?;
    Ok(())
}

/// Updates the named artifacts (or every outdated one) and reports each.
fn apply(
    library: &mut Library,
    items: &[LibraryItem],
    named: &[&LibraryItem],
    remove_deleted: bool,
) -> anyhow::Result<Vec<ItemResult>> {
    let by_id: HashMap<&str, &LibraryItem> = items.iter().map(|i| (i.id.as_str(), i)).collect();
    if named.is_empty() {
        let (_, gone, _) = entries(library, items)?;
        // No prompt in the CLI: `--remove-deleted` is the confirmation for what is removed now.
        let confirmed: Vec<String> = if remove_deleted {
            library
                .removed_at_source_plan()?
                .artifacts
                .into_iter()
                .map(|item| item.id)
                .collect()
        } else {
            Vec::new()
        };
        let reports = library.update_all_with(&confirmed)?;
        // Imported artifacts were not in the library when `items` was read.
        let after = all_items(library)?;
        let found: HashMap<&str, &LibraryItem> = after.iter().map(|i| (i.id.as_str(), i)).collect();
        let mut results: Vec<ItemResult> = reports
            .into_iter()
            .filter_map(|r| match r.outcome {
                UpdateOutcome::NotPlaced => Some(not_in_library(r)),
                UpdateOutcome::Imported => found.get(r.id.as_str()).map(|i| result_of(i, r)),
                _ => by_id.get(r.id.as_str()).map(|i| result_of(i, r)),
            })
            .collect();
        if !remove_deleted {
            // Listed, never deleted without the flag (a non-interactive run has no prompt).
            results.extend(
                gone.into_iter()
                    .filter(|e| e.state == "removed")
                    .map(|e| ItemResult {
                        id: e.id,
                        name: e.name,
                        kind: e.kind,
                        outcome: "removedUpstream".to_owned(),
                        from_version: None,
                        to_version: None,
                        message: None,
                        new_name: None,
                    }),
            );
        }
        return Ok(results);
    }
    let mut results = Vec::new();
    for item in named {
        let status = library.update_status(&item.id)?;
        let report = match status {
            Some(s) if s.state.is_applicable() && s.local_edits => UpdateReport {
                id: item.id.clone(),
                outcome: UpdateOutcome::SkippedLocalEdits,
                backup: None,
                from_version: s.from_version,
                to_version: s.to_version,
                message: None,
                name: None,
                source_path: None,
            },
            Some(s) if s.state.is_applicable() => {
                match library.update(&item.id, UpdateChoice::TakeUpstream) {
                    Ok(report) => report,
                    Err(error) => UpdateReport {
                        id: item.id.clone(),
                        outcome: UpdateOutcome::Failed,
                        backup: None,
                        from_version: s.from_version,
                        to_version: s.to_version,
                        message: Some(error.to_string()),
                        name: None,
                        source_path: None,
                    },
                }
            }
            _ => UpdateReport {
                id: item.id.clone(),
                outcome: UpdateOutcome::Kept,
                backup: None,
                from_version: None,
                to_version: None,
                message: None,
                name: None,
                source_path: None,
            },
        };
        results.push(result_of(item, report));
    }
    Ok(results)
}

/// Runs `harneo update`. Exit 1 only when something could not be run (an error, an unknown
/// name, or an update that failed); skipped artifacts are reported but are not an error.
pub fn run(args: &UpdateArgs, out: &mut impl Write) -> anyhow::Result<ExitCode> {
    let (mut library, _) = open_library()?;
    let items = all_items(&library)?;
    let named: Vec<&LibraryItem> = args
        .artifacts
        .iter()
        .map(|input| resolve(&items, input))
        .collect::<anyhow::Result<_>>()?;
    let report = library.check_updates(&CheckScope::All)?;
    let errors = source_errors(&report, &items);
    if args.check {
        let (outdated, removed, renamed) = entries(&library, &items)?;
        let check = CheckOutput {
            outdated,
            removed,
            renamed,
            new_at_source: new_at_source(&report),
            errors,
        };
        if args.json {
            print_json(out, &check)?;
        } else {
            write!(out, "{}", render_check(&check))?;
        }
        return Ok(ExitCode::SUCCESS);
    }
    let results = apply(&mut library, &items, &named, args.remove_deleted)?;
    let count = |outcome: &str| {
        u32::try_from(results.iter().filter(|r| r.outcome == outcome).count()).unwrap_or(u32::MAX)
    };
    let summary = Summary {
        updated: count("updated"),
        skipped: count("skippedLocalEdits"),
        failed: count("failed"),
        imported: count("imported"),
        not_placed: count("notPlaced"),
        removed: count("removed"),
        renamed: count("renamed"),
    };
    let failed = summary.failed > 0;
    let output = UpdateOutput {
        results,
        errors,
        summary,
    };
    if args.json {
        print_json(out, &output)?;
    } else {
        write!(out, "{}", render_update(&output))?;
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(local_edits: bool, from: Option<&str>, to: Option<&str>) -> Entry {
        Entry {
            id: "x".into(),
            name: "plan".into(),
            kind: "skill".into(),
            state: "outdated".into(),
            from_version: from.map(str::to_owned),
            to_version: to.map(str::to_owned),
            local_edits,
            renamed_to: None,
        }
    }

    #[test]
    fn check_render_lists_versions_edits_removed_and_errors() {
        let check = CheckOutput {
            outdated: vec![
                entry(true, Some("1.0"), Some("1.1")),
                entry(false, None, None),
            ],
            removed: vec![entry(false, None, None), {
                let mut moved = entry(false, None, None);
                moved.name = "moved".into();
                moved.state = "layout_changed".into();
                moved
            }],
            renamed: vec![{
                let mut moved = entry(false, None, None);
                moved.name = "old".into();
                moved.state = "renamed".into();
                moved.renamed_to = Some("new".into());
                moved
            }],
            new_at_source: 0,
            errors: vec![SourceError {
                id: "s1".into(),
                label: "me/repo".into(),
                problem: "unreachable: offline".into(),
            }],
        };
        let text = render_check(&check);
        assert!(text.contains("plan (skill)  1.0 → 1.1  [local edits]\n"));
        assert!(text.contains("plan (skill)  changed\n"));
        assert!(text.contains("plan (skill)  removed upstream\n"));
        assert!(text.contains("moved (skill)  layout changed upstream\n"));
        assert!(text.contains("source me/repo (s1): unreachable: offline\n"));
        assert!(text.contains("old (skill)  renamed at source: old → new\n"));
        assert!(text.ends_with("2 outdated, 2 removed upstream, 1 renamed, 1 source error\n"));
    }

    fn item_result(outcome: &str, message: Option<&str>) -> ItemResult {
        ItemResult {
            id: "x".into(),
            name: "plan".into(),
            kind: "skill".into(),
            outcome: outcome.into(),
            from_version: Some("a".into()),
            to_version: None,
            message: message.map(str::to_owned),
            new_name: (outcome == "renamed").then(|| "new".to_owned()),
        }
    }

    #[test]
    fn update_render_has_a_line_per_outcome_and_a_summary() {
        let output = UpdateOutput {
            results: vec![
                item_result("updated", None),
                item_result("skippedLocalEdits", None),
                item_result("failed", Some("boom")),
                item_result("notOutdated", None),
                item_result("renamed", None),
            ],
            errors: Vec::new(),
            summary: Summary {
                updated: 1,
                skipped: 1,
                failed: 1,
                imported: 0,
                not_placed: 0,
                removed: 0,
                renamed: 1,
            },
        };
        let text = render_update(&output);
        assert!(text.contains("updated plan (skill)  a → ?\n"));
        assert!(text.contains("update it in the app to choose"));
        assert!(text.contains("failed plan (skill): boom\n"));
        assert!(text.contains("plan (skill): nothing to update\n"));
        assert!(text.contains("renamed plan (skill)  renamed at source: plan → new, with"));
        assert!(text.contains("Renamed 1 at source\n"));
        assert!(text.ends_with("Updated 1, skipped 1 (local edits), failed 1\n"));
    }
}
