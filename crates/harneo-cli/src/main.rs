//! `harneo`: the CLI companion. Commands arrive with the modules that own them.

mod audit;
mod profiles;
mod scaffold;
mod scan;
mod status;
mod update;

use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "harneo", version = harneo_core::version())]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the folder where Harneo keeps its data.
    Paths,
    /// Read a folder and list the artifacts and problems found in it.
    Scan {
        /// The folder to scan.
        path: PathBuf,
        /// Print the full report as JSON.
        #[arg(long)]
        json: bool,
        /// Also scan Claude-managed folders (`skills/synced/`, `dev-mods/`), skipped by default.
        #[arg(long)]
        include_managed: bool,
    },
    /// Audit every artifact in a folder: trust score, findings and what each can reach.
    Audit {
        /// The folder to audit.
        path: PathBuf,
        /// Print the audits as JSON.
        #[arg(long)]
        json: bool,
        /// Also audit Claude-managed folders (`skills/synced/`, `dev-mods/`), skipped by default.
        #[arg(long)]
        include_managed: bool,
        /// Skip phase 2 for this run: do not ask Claude to confirm the findings.
        #[arg(long)]
        no_claude: bool,
    },
    /// List the profiles in the library.
    Profiles {
        /// Print the profiles as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Write a profile into a folder that has no harness yet, and register the project.
    Scaffold(scaffold::ScaffoldArgs),
    /// Check the library's sources and update outdated artifacts that have no local edits.
    Update(update::UpdateArgs),
    /// Show every registered project with its drift, or one project's drifted files.
    Status {
        /// A registered project folder; every project when left out.
        path: Option<PathBuf>,
        /// Print the reports as JSON.
        #[arg(long)]
        json: bool,
        /// Exit with 3 when a listed project has drifted.
        #[arg(long)]
        exit_code: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("harneo: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        Command::Paths => println!("{}", harneo_core::paths::data_dir()?.display()),
        Command::Profiles { json } => {
            profiles::run(json, &mut std::io::stdout())?;
        }
        Command::Scaffold(args) => {
            let interactive = std::io::stdin().is_terminal();
            return scaffold::run(&args, &mut std::io::stdout(), interactive, || {
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line)?;
                Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
            });
        }
        Command::Update(args) => return update::run(&args, &mut std::io::stdout()),
        Command::Status {
            path,
            json,
            exit_code,
        } => return status::run(path.as_deref(), json, exit_code, &mut std::io::stdout()),
        Command::Scan {
            path,
            json,
            include_managed,
        } => scan::run(&path, json, include_managed)?,
        Command::Audit {
            path,
            json,
            include_managed,
            no_claude,
        } => audit::run(&path, json, include_managed, !no_claude)?,
    }
    Ok(ExitCode::SUCCESS)
}
