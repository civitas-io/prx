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

    let start = std::time::Instant::now();

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
        Commands::Bench(args) => commands::bench::run(args),
        #[cfg(feature = "mcp")]
        Commands::Mcp(args) => commands::mcp::run(args),
    };

    let wall_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(ref data) => {
            log_telemetry(&command_name, data, wall_ms);
            write_envelope(&command_name, data.clone(), plain);
        }
        Err(e) => output::write_error(&command_name, &e, plain),
    }

    Ok(())
}

fn log_telemetry(command: &str, data: &serde_json::Value, wall_ms: u64) {
    let actual_bytes = serde_json::to_string(data).map(|s| s.len()).unwrap_or(0);

    let baseline_bytes = match command {
        "search" => {
            // baseline: grep returns all match lines + agent reads matched files
            // estimate: total_matches * avg snippet size, or file sizes
            data.get("total_matches")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize * 200) // ~200 bytes per grep match line + context
                .unwrap_or(actual_bytes)
        }
        "read" => {
            // baseline: cat entire file
            data.get("meta")
                .and_then(|m| m.get("bytes"))
                .and_then(|v| v.as_u64())
                .map(|b| b as usize)
                .unwrap_or(actual_bytes)
        }
        "run" => {
            // baseline: raw command output
            let saved = data
                .get("output_tokens_saved")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            actual_bytes + saved * 4
        }
        "outline" => {
            // baseline: cat all files to get symbols
            data.get("symbols")
                .and_then(|s| s.as_array())
                .map(|syms| syms.len() * 500) // ~500 bytes per file to cat
                .unwrap_or(actual_bytes)
        }
        _ => actual_bytes,
    };

    let strategy = match command {
        "search" => "grep_rn_all_matches",
        "read" => "cat_full_file",
        "run" => "raw_command_output",
        "outline" => "cat_files_for_symbols",
        "exists" => "grep_rl",
        "find" => "find_ls",
        _ => "parity",
    };

    commands::stats::log_stat_entry(&commands::stats::StatEntry {
        command: command.to_string(),
        actual_bytes,
        baseline_bytes,
        baseline_strategy: strategy.to_string(),
        wall_ms,
    });
}
