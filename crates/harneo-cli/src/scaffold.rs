//! `harneo scaffold`: plan, confirm, apply. Never writes without --yes or a confirmation.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context};
use clap::Args;
use harneo_core::install::{ApplyReport, Plan, WriteAction};
use harneo_core::library::Library;
use harneo_core::profiles::ProfileId;
use harneo_core::Error;
use serde::Serialize;

use crate::profiles::open_library;

#[derive(Args)]
pub struct ScaffoldArgs {
    /// The folder to write into; it must have no harness yet.
    pub path: PathBuf,
    /// The profile, by name (any case) or id.
    #[arg(long)]
    pub profile: String,
    /// A profile version; the current one when left out.
    #[arg(long)]
    pub version: Option<u32>,
    /// Print the plan and stop.
    #[arg(long)]
    pub dry_run: bool,
    /// Write without asking.
    #[arg(long)]
    pub yes: bool,
    /// Print the plan (and the result) as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct Applied<'a> {
    plan: &'a Plan,
    report: &'a ApplyReport,
}

/// The profile named `wanted` (ignoring case), or else the one with that numeric id.
fn resolve_profile(library: &Library, wanted: &str) -> anyhow::Result<ProfileId> {
    let profiles = library.profiles()?;
    let wanted = wanted.trim();
    if let Some(p) = profiles
        .iter()
        .find(|p| p.name.to_lowercase() == wanted.to_lowercase())
    {
        return Ok(p.id);
    }
    if let Some(p) = wanted
        .parse::<ProfileId>()
        .ok()
        .and_then(|id| profiles.iter().find(|p| p.id == id))
    {
        return Ok(p.id);
    }
    Err(anyhow!("no profile named or numbered {wanted}"))
}

fn render(plan: &Plan) -> String {
    let mut out = format!(
        "Scaffold {} with {} v{}:\n",
        plan.target, plan.profile_name, plan.profile_version
    );
    for w in &plan.writes {
        let action = match w.action {
            WriteAction::New => "new",
            WriteAction::Replace => "replace",
            WriteAction::Delete => "delete",
            WriteAction::Keep => "keep",
            WriteAction::Skip => "skip",
        };
        let _ = writeln!(out, "  {action:<8}{}", w.path);
    }
    let _ = writeln!(out, "{} files will be written.", plan.writes.len());
    out
}

/// Scaffolds a project. `interactive` is whether stdin is a terminal; `confirm` reads the answer.
pub fn run(
    args: &ScaffoldArgs,
    out: &mut impl Write,
    interactive: bool,
    confirm: impl FnOnce() -> io::Result<bool>,
) -> anyhow::Result<ExitCode> {
    let (library, _) = open_library()?;
    let id = resolve_profile(&library, &args.profile)?;
    let plan = library
        .plan_scaffold(&args.path, id, args.version)
        .map_err(|e| match e {
            Error::HasHarness { .. } => anyhow!("{e}\nrefit it in the app"),
            other => other.into(),
        })?;
    if args.json && args.dry_run {
        writeln!(out, "{}", serde_json::to_string_pretty(&plan)?)?;
    } else if !args.json {
        write!(out, "{}", render(&plan))?;
    }
    if args.dry_run {
        return Ok(ExitCode::SUCCESS);
    }
    if !args.yes {
        if !interactive {
            eprintln!("harneo: pass --yes to write without asking");
            return Ok(ExitCode::from(2));
        }
        eprint!("Write these files? [y/N] ");
        io::stderr().flush()?;
        if !confirm().context("cannot read the answer")? {
            writeln!(out, "Nothing written.")?;
            return Ok(ExitCode::SUCCESS);
        }
    }
    let report = library.apply(&plan)?;
    if args.json {
        let text = serde_json::to_string_pretty(&Applied {
            plan: &plan,
            report: &report,
        })?;
        writeln!(out, "{text}")?;
    } else {
        writeln!(
            out,
            "Scaffolded {} with {} v{}: {} files, registered.",
            args.path.display(),
            plan.profile_name,
            plan.profile_version,
            plan.writes.len()
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use harneo_core::install::Write as PlanWrite;

    #[test]
    fn render_lists_each_file_with_its_action_and_a_summary() {
        let write = |path: &str, action| PlanWrite {
            path: path.into(),
            action,
            bytes: None,
            diff: None,
            artifact: None,
            executable: false,
        };
        let plan = Plan {
            target: "/p".into(),
            profile_id: 1,
            profile_name: "Web".into(),
            profile_version: 2,
            reason: String::new(),
            writes: vec![
                write("a", WriteAction::New),
                write("b", WriteAction::Replace),
                write("c", WriteAction::Delete),
                write("d", WriteAction::Keep),
                write("e", WriteAction::Skip),
            ],
            backup_count: 0,
            ..Plan::default()
        };
        let text = render(&plan);
        assert!(text.starts_with("Scaffold /p with Web v2:\n  new     a\n  replace b\n"));
        assert!(text.contains("  delete  c\n  keep    d\n  skip    e\n"));
        assert!(text.ends_with("5 files will be written.\n"));
    }
}
