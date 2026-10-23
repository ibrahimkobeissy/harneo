//! `harneo status`: every registered project with its drift, or one project's drifted files.

use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{anyhow, Context};
use harneo_core::drift::{DriftChange, DriftReport, DriftStatus, DriftSummary};
use harneo_core::install::Project;
use serde::Serialize;

use crate::profiles::open_library;

/// The exit code `--exit-code` returns when a listed project drifted.
const EXIT_DRIFT: u8 = 3;

#[derive(Serialize)]
struct ListEntry<'a> {
    project: &'a Project,
    summary: &'a DriftSummary,
}

#[derive(Serialize)]
struct Detail<'a> {
    project: &'a Project,
    report: &'a DriftReport,
}

/// The wording of the app's pills; several can apply to one project.
fn state(summary: &DriftSummary) -> String {
    if summary.status == DriftStatus::Missing {
        return "Missing".to_owned();
    }
    let mut parts = Vec::new();
    if summary.status == DriftStatus::Drift {
        let word = if summary.drifted == 1 {
            "file"
        } else {
            "files"
        };
        parts.push(format!("Drift · {} {word}", summary.drifted));
    }
    if let Some(version) = summary.profile_newer {
        parts.push(format!("Profile v{version} available"));
    }
    if summary.library_newer > 0 {
        parts.push("Library has newer copies".to_owned());
    }
    if parts.is_empty() {
        "In sync".to_owned()
    } else {
        parts.join(", ")
    }
}

fn render_list(rows: &[(Project, DriftSummary)]) -> String {
    if rows.is_empty() {
        return "No projects yet.\n".to_owned();
    }
    let mut out = String::new();
    for (p, summary) in rows {
        let _ = writeln!(
            out,
            "{}  {}  {} v{}  {}",
            p.name,
            p.path,
            p.profile_name,
            p.profile_version,
            state(summary)
        );
    }
    out
}

fn render_detail(project: &Project, report: &DriftReport) -> String {
    let mut out = format!(
        "{}  {}  {} v{}\n",
        project.name, project.path, project.profile_name, project.profile_version
    );
    if report.status == DriftStatus::Missing {
        out.push_str("Missing: the project folder is gone.\n");
        return out;
    }
    for f in &report.files {
        let _ = match &f.change {
            DriftChange::Edited { plus, minus } => {
                writeln!(out, "edited  +{plus} −{minus}  {}", f.path)
            }
            DriftChange::Deleted => writeln!(out, "deleted  {}", f.path),
            DriftChange::Added => writeln!(out, "added  {}", f.path),
        };
    }
    for path in &report.library_newer {
        let _ = writeln!(out, "library newer  {path}");
    }
    if let Some(version) = report.profile_newer {
        let _ = writeln!(out, "Profile v{version} available");
    }
    if report.files.is_empty() && report.library_newer.is_empty() && report.profile_newer.is_none()
    {
        out.push_str("In sync\n");
    }
    out
}

fn print_json(out: &mut impl Write, value: &impl Serialize) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(value).context("cannot serialize the status")?;
    writeln!(out, "{text}")?;
    Ok(())
}

pub fn run(
    path: Option<&Path>,
    json: bool,
    exit_code: bool,
    out: &mut impl Write,
) -> anyhow::Result<ExitCode> {
    let (library, _) = open_library()?;
    let projects = library.projects()?;
    let drifted = if let Some(path) = path {
        let wanted = std::fs::canonicalize(path)
            .or_else(|_| std::path::absolute(path))
            .with_context(|| format!("cannot resolve {}", path.display()))?;
        let project = projects
            .iter()
            .find(|p| Path::new(&p.path) == wanted)
            .ok_or_else(|| anyhow!("{} is not a registered project", path.display()))?;
        let report = library.check_project(project.id)?;
        if json {
            print_json(
                out,
                &Detail {
                    project,
                    report: &report,
                },
            )?;
        } else {
            write!(out, "{}", render_detail(project, &report))?;
        }
        report.status == DriftStatus::Drift
    } else {
        let mut by_id = std::collections::HashMap::new();
        for d in library.check_all()? {
            by_id.insert(d.project_id, d.summary);
        }
        let rows: Vec<(Project, DriftSummary)> = projects
            .into_iter()
            .filter_map(|p| by_id.remove(&p.id).map(|s| (p, s)))
            .collect();
        if json {
            let entries: Vec<ListEntry> = rows
                .iter()
                .map(|(project, summary)| ListEntry { project, summary })
                .collect();
            print_json(out, &entries)?;
        } else {
            write!(out, "{}", render_list(&rows))?;
        }
        rows.iter().any(|(_, s)| s.status == DriftStatus::Drift)
    };
    Ok(if exit_code && drifted {
        ExitCode::from(EXIT_DRIFT)
    } else {
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(status: DriftStatus, drifted: u32, lib: u32, profile: Option<u32>) -> DriftSummary {
        DriftSummary {
            status,
            drifted,
            library_newer: lib,
            profile_newer: profile,
        }
    }

    #[test]
    fn state_words_match_the_pills() {
        assert_eq!(state(&summary(DriftStatus::InSync, 0, 0, None)), "In sync");
        assert_eq!(
            state(&summary(DriftStatus::Missing, 0, 0, Some(2))),
            "Missing"
        );
        assert_eq!(
            state(&summary(DriftStatus::Drift, 1, 0, None)),
            "Drift · 1 file"
        );
        assert_eq!(
            state(&summary(DriftStatus::Drift, 3, 2, Some(4))),
            "Drift · 3 files, Profile v4 available, Library has newer copies"
        );
        assert_eq!(
            state(&summary(DriftStatus::InSync, 0, 1, None)),
            "Library has newer copies"
        );
    }

    #[test]
    fn an_empty_list_says_so() {
        assert_eq!(render_list(&[]), "No projects yet.\n");
    }
}
