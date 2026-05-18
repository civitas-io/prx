#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;

mod chunking;
mod commands;
mod hash;
mod index;
mod output;
mod parsing;
mod ranking;
mod runner;
mod search;
mod tokens;
mod walk;

use commands::{Cli, Commands};
use output::write_envelope;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let command_name = cli.command.name();
    let plain = cli.plain;

    let result = match cli.command {
        Commands::Search(args) => commands::search::run(args),
        Commands::Read(args) => commands::read::run(args),
        Commands::Find(args) => commands::find::run(args),
        Commands::Edit(args) => commands::edit::run(args),
        Commands::Diff(args) => commands::diff::run(args),
        Commands::Index(args) => commands::index::run(args),
        Commands::Outline(args) => commands::outline::run(args),
        Commands::Exists(args) => commands::exists::run(args),
        Commands::Batch(args) => commands::batch::run(args),
        Commands::Stats(args) => commands::stats::run(args),
        Commands::Init(args) => commands::init::run(args),
        Commands::Run(args) => commands::run::run(args),
        #[cfg(feature = "mcp")]
        Commands::Mcp(args) => commands::mcp::run(args),
    };

    match result {
        Ok(data) => write_envelope(&command_name, data, plain),
        Err(e) => output::write_error(&command_name, &e, plain),
    }

    Ok(())
}
