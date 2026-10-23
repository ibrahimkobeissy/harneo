//! `harneo`: the CLI companion. Commands arrive with the modules that own them.

mod audit;
mod scan;

use std::path::PathBuf;

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
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Paths => println!("{}", harneo_core::paths::data_dir()?.display()),
        Command::Scan {
            path,
            json,
            include_managed,
        } => scan::run(&path, json, include_managed)?,
        Command::Audit {
            path,
            json,
            include_managed,
        } => audit::run(&path, json, include_managed)?,
    }
    Ok(())
}
