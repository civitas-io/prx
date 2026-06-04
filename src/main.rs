use anyhow::Result;
use clap::Parser;

mod budget;
mod chunking;
mod commands;
mod fallback;
mod git;
mod hash;
mod index;
mod models;
mod output;
mod parsing;
mod ranking;
mod runner;
mod search;
mod tokens;
mod walk;
mod workspace;

use commands::{Cli, Commands};
use output::write_envelope;

fn main() -> Result<()> {
    // Pin BLAS thread pools to 1 — prevents N×N oversubscription when rayon
    // parallelizes embedding calls (each of which may invoke BLAS internally).
    unsafe {
        std::env::set_var("VECLIB_MAXIMUM_THREADS", "1");
        std::env::set_var("OPENBLAS_NUM_THREADS", "1");
        std::env::set_var("MKL_NUM_THREADS", "1");
    }

    let cli = Cli::parse();
    let command_name = cli.command.name();
    let plain = cli.plain;
    let no_fallback = cli.no_fallback;
    let can_fb = !no_fallback && fallback::can_fallback(&command_name);
    let fb_spec = if can_fb {
        fallback::fallback_spec(&command_name, &cli.command)
    } else {
        None
    };

    let start = std::time::Instant::now();

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_command(cli.command)));

    let wall_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(ref data)) => {
            log_telemetry(&command_name, data, wall_ms);
            if plain && command_name == "bench-ndcg" {
                commands::bench_ndcg::render_plain(data);
            } else if plain && command_name == "bench" {
                commands::bench::render_plain(data);
            } else {
                write_envelope(&command_name, data.clone(), plain);
            }
        }
        Ok(Err(ref e)) => {
            if should_fallback(e) {
                handle_error(&command_name, &e.to_string(), fb_spec, plain);
            } else {
                output::write_error(&command_name, e, plain);
            }
        }
        Err(panic_info) => {
            let msg = panic_info
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_info.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            handle_error(&command_name, msg, fb_spec, plain);
        }
    }

    Ok(())
}

fn should_fallback(error: &output::AgError) -> bool {
    matches!(
        error,
        output::AgError::Internal { .. }
            | output::AgError::ParseError { .. }
            | output::AgError::IndexCorrupted { .. }
    )
}

fn handle_error(command: &str, error: &str, fb_spec: Option<(String, Vec<String>)>, plain: bool) {
    if let Some((cmd, args)) = fb_spec {
        if let Some(data) = fallback::execute_fallback(&cmd, &args) {
            let raw_bytes = serde_json::to_string(&data).map(|s| s.len()).unwrap_or(0);
            let fallback_cmd = format!("{cmd} {}", args.join(" "));
            fallback::log_error(command, error, &fallback_cmd, raw_bytes);
            output::write_fallback_envelope(command, data, plain);
            return;
        }
    }

    output::write_error(
        command,
        &output::AgError::Internal {
            message: error.to_string(),
        },
        plain,
    );
}

fn run_command(command: Commands) -> Result<serde_json::Value, output::AgError> {
    match command {
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
        Commands::BenchNdcg(args) => commands::bench_ndcg::run(args),
        Commands::Context(args) => commands::context::run(args),
        Commands::Impact(args) => commands::impact::run(args),
        Commands::Explain(args) => commands::explain::run(args),
        Commands::Rename(args) => commands::rename::run(args),
        #[cfg(feature = "mcp")]
        Commands::Mcp(args) => commands::mcp::run(args),
    }
}

fn log_telemetry(command: &str, data: &serde_json::Value, wall_ms: u64) {
    // Baselines are modeled estimates of what grep/cat/find would emit for
    // the same query. For measured numbers, run `prx bench .`.
    let actual_bytes = serde_json::to_string(data).map(|s| s.len()).unwrap_or(0);

    let baseline_bytes = match command {
        "search" => {
            let matches = data
                .get("total_matches")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let unique_files = data
                .get("matches")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    let mut files: Vec<&str> = arr
                        .iter()
                        .filter_map(|m| m.get("file").and_then(|f| f.as_str()))
                        .collect();
                    files.sort();
                    files.dedup();
                    files.len()
                })
                .unwrap_or(1);
            // grep output (~120B per match) + cat of unique matched files (~3000B avg)
            matches * 120 + unique_files * 3000
        }
        "read" => data
            .get("meta")
            .and_then(|m| m.get("bytes"))
            .and_then(|v| v.as_u64())
            .map(|b| b as usize)
            .unwrap_or(actual_bytes),
        "run" => {
            let saved = data
                .get("output_tokens_saved")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            actual_bytes + saved * 4
        }
        "find" => {
            let total = data
                .get("stats")
                .and_then(|s| s.get("total_files"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            // find output (~80B per file) + follow-up wc -l/file for each (~40B)
            total * 120
        }
        "exists" => {
            // grep -rl scans entire codebase, returns matching file list
            // estimate: ~500B for a typical grep -rl output
            500usize.max(actual_bytes)
        }
        "diff" => {
            let additions = data
                .get("stats")
                .and_then(|s| s.get("additions"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let deletions = data
                .get("stats")
                .and_then(|s| s.get("deletions"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            // git diff: ~100B per changed line (with context)
            (additions + deletions) * 100 + 200
        }
        "outline" => data
            .get("symbols")
            .and_then(|s| s.as_array())
            .map(|syms| syms.len() * 500)
            .unwrap_or(actual_bytes),
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
