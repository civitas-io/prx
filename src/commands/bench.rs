use std::path::Path;
use std::time::Instant;

use clap::Args;
use serde::Serialize;

use crate::output::AgError;

#[derive(Args)]
pub struct BenchArgs {
    /// Root path to benchmark against
    #[arg(default_value = ".")]
    pub path: String,

    /// Output raw JSON instead of formatted table
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct BenchOutput {
    tasks: Vec<TaskResult>,
    summary: BenchSummary,
}

#[derive(Serialize)]
struct TaskResult {
    name: String,
    query: String,
    prx_tokens: usize,
    baseline_tokens: usize,
    savings_pct: f64,
    prx_ms: u64,
    baseline_ms: u64,
}

#[derive(Serialize)]
struct BenchSummary {
    total_tasks: usize,
    avg_savings_pct: f64,
    total_prx_tokens: usize,
    total_baseline_tokens: usize,
    prx_total_ms: u64,
    baseline_total_ms: u64,
}

struct Task {
    name: &'static str,
    query: &'static str,
    prx_args: Vec<String>,
    baseline_cmd: Vec<String>,
}

fn tasks(root: &str) -> Vec<Task> {
    let r = root.to_string();
    vec![
        Task {
            name: "literal_search",
            query: "fn main",
            prx_args: vec![
                "search".into(),
                "--literal".into(),
                "fn main".into(),
                r.clone(),
            ],
            baseline_cmd: vec!["grep".into(), "-rn".into(), "fn main".into(), r.clone()],
        },
        Task {
            name: "semantic_search",
            query: "hash content bytes",
            prx_args: vec![
                "search".into(),
                "hash content bytes".into(),
                r.clone(),
                "--top-k".into(),
                "5".into(),
            ],
            baseline_cmd: vec!["grep".into(), "-rn".into(), "hash".into(), r.clone()],
        },
        Task {
            name: "read_skeleton",
            query: "main.rs skeleton",
            prx_args: vec![
                "read".into(),
                format!("{r}/src/main.rs"),
                "--skeleton".into(),
            ],
            baseline_cmd: vec!["cat".into(), format!("{r}/src/main.rs")],
        },
        Task {
            name: "read_full_file",
            query: "search.rs",
            prx_args: vec!["read".into(), format!("{r}/src/commands/search.rs")],
            baseline_cmd: vec!["cat".into(), format!("{r}/src/commands/search.rs")],
        },
        Task {
            name: "find_rust_files",
            query: "*.rs files",
            prx_args: vec!["find".into(), r.clone(), "--pattern".into(), "*.rs".into()],
            baseline_cmd: vec!["find".into(), r.clone(), "-name".into(), "*.rs".into()],
        },
        Task {
            name: "semantic_search",
            query: "hash content bytes",
            prx_args: vec!["search".into(), "--literal".into(), "hash".into(), r.clone(), "--top-k".into(), "5".into()],
            baseline_cmd: vec!["grep".into(), "-rn".into(), "hash".into(), r.clone()],
        },
        Task {
            name: "outline_file",
            query: "hash.rs symbols",
            prx_args: vec!["outline".into(), format!("{r}/src/hash.rs")],
            baseline_cmd: vec!["cat".into(), format!("{r}/src/hash.rs")],
        },
        Task {
            name: "run_echo",
            query: "test output parsing",
            prx_args: vec![
                "run".into(),
                "echo".into(),
                "test result: ok. 50 passed; 0 failed; 0 ignored".into(),
            ],
            baseline_cmd: vec![
                "echo".into(),
                "test result: ok. 50 passed; 0 failed; 0 ignored".into(),
            ],
        },
    ]
}

pub fn run(args: BenchArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let prx_binary = std::env::current_exe().unwrap_or_else(|_| "prx".into());
    let mut results = Vec::new();
    let root_str = root.to_str().unwrap_or(".");

    for task in tasks(root_str) {
        let prx_start = Instant::now();
        let prx_output = std::process::Command::new(&prx_binary)
            .args(&task.prx_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        let prx_ms = prx_start.elapsed().as_millis() as u64;

        let prx_bytes = prx_output.as_ref().map(|o| o.stdout.len()).unwrap_or(0);

        let baseline_start = Instant::now();
        let baseline_output = std::process::Command::new(&task.baseline_cmd[0])
            .args(&task.baseline_cmd[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        let baseline_ms = baseline_start.elapsed().as_millis() as u64;

        let baseline_bytes = baseline_output
            .as_ref()
            .map(|o| o.stdout.len())
            .unwrap_or(0);

        let prx_tokens = prx_bytes / 4;
        let baseline_tokens = baseline_bytes / 4;

        let savings = if baseline_tokens > 0 {
            ((baseline_tokens.saturating_sub(prx_tokens)) as f64 / baseline_tokens as f64) * 100.0
        } else {
            0.0
        };

        results.push(TaskResult {
            name: task.name.to_string(),
            query: task.query.to_string(),
            prx_tokens,
            baseline_tokens,
            savings_pct: (savings * 10.0).round() / 10.0,
            prx_ms,
            baseline_ms,
        });
    }

    let total_prx: usize = results.iter().map(|r| r.prx_tokens).sum();
    let total_baseline: usize = results.iter().map(|r| r.baseline_tokens).sum();
    let total_prx_ms: u64 = results.iter().map(|r| r.prx_ms).sum();
    let total_baseline_ms: u64 = results.iter().map(|r| r.baseline_ms).sum();
    let avg_savings = if !results.is_empty() {
        results.iter().map(|r| r.savings_pct).sum::<f64>() / results.len() as f64
    } else {
        0.0
    };

    let output = BenchOutput {
        summary: BenchSummary {
            total_tasks: results.len(),
            avg_savings_pct: (avg_savings * 10.0).round() / 10.0,
            total_prx_tokens: total_prx,
            total_baseline_tokens: total_baseline,
            prx_total_ms: total_prx_ms,
            baseline_total_ms: total_baseline_ms,
        },
        tasks: results,
    };

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}
