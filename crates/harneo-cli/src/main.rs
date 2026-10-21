//! `harneo`: the CLI companion. Commands arrive with the modules that own them.

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
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Paths => println!("{}", harneo_core::paths::data_dir()?.display()),
        Command::Scan { path, json } => scan::run(&path, json)?,
    }
    Ok(())
}
