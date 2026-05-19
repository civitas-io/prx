pub mod batch;
pub mod bench;
pub mod diff;
pub mod edit;
pub mod exists;
pub mod find;
pub mod index;
pub mod init;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod outline;
pub mod read;
pub mod run;
pub mod search;
pub mod stats;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "prx", version, about = "Praxis — agent-native Unix tools")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Human-readable output instead of JSON
    #[arg(long, global = true)]
    pub plain: bool,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Search the codebase by query
    Search(search::SearchArgs),
    /// Read file content with structural awareness
    Read(read::ReadArgs),
    /// List and filter files in the workspace
    Find(find::FindArgs),
    /// Find and replace content in a file
    Edit(edit::EditArgs),
    /// Show git diffs with semantic summaries
    Diff(diff::DiffArgs),
    /// Build or update the search index
    Index(index::IndexArgs),
    /// Print the symbol table for a file or directory
    Outline(outline::OutlineArgs),
    /// Probabilistic existence check for a pattern
    Exists(exists::ExistsArgs),
    /// Execute multiple commands in parallel from stdin
    Batch(batch::BatchArgs),
    /// Print token savings dashboard
    Stats(stats::StatsArgs),
    /// Generate integration files for agent frameworks
    Init(init::InitArgs),
    /// Run a command and return structured output
    Run(run::RunArgs),
    /// Run synthetic benchmarks comparing prx vs grep+cat
    Bench(bench::BenchArgs),
    /// Start MCP server on stdio
    #[cfg(feature = "mcp")]
    Mcp(mcp::McpArgs),
}

impl Commands {
    pub fn name(&self) -> String {
        match self {
            Self::Search(_) => "search",
            Self::Read(_) => "read",
            Self::Find(_) => "find",
            Self::Edit(_) => "edit",
            Self::Diff(_) => "diff",
            Self::Index(_) => "index",
            Self::Outline(_) => "outline",
            Self::Exists(_) => "exists",
            Self::Batch(_) => "batch",
            Self::Stats(_) => "stats",
            Self::Init(_) => "init",
            Self::Run(_) => "run",
            Self::Bench(_) => "bench",
            #[cfg(feature = "mcp")]
            Self::Mcp(_) => "mcp",
        }
        .to_string()
    }
}
