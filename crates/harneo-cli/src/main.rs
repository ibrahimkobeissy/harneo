//! `harneo`: the CLI companion. Commands arrive with the modules that own them.

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
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Paths => println!("{}", harneo_core::paths::data_dir()?.display()),
    }
    Ok(())
}
