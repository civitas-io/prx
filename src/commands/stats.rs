use std::io::BufRead;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::output::AgError;

#[derive(Args)]
pub struct StatsArgs {
    /// Per-command breakdown
    #[arg(long)]
    pub verbose: bool,

    /// Show savings comparison (actual vs baseline)
    #[arg(long)]
    pub compare: bool,

    /// Clear saved statistics
    #[arg(long)]
    pub reset: bool,
}

#[derive(Serialize)]
struct StatsOutput {
    periods: Vec<PeriodStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by_command: Option<Vec<CommandStats>>,
}

#[derive(Serialize)]
struct PeriodStats {
    label: String,
    calls: usize,
    tokens_saved: usize,
}

#[derive(Serialize)]
struct CommandStats {
    command: String,
    calls: usize,
    actual_tokens: usize,
    baseline_tokens: usize,
    savings_pct: f64,
}

fn stats_path() -> PathBuf {
    if let Ok(custom) = std::env::var("PRX_STATS_FILE") {
        return PathBuf::from(custom);
    }
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".prx")
        .join("stats.jsonl")
}

pub fn run(args: StatsArgs) -> Result<serde_json::Value, AgError> {
    let path = stats_path();

    if args.reset {
        if path.exists() {
            std::fs::remove_file(&path).map_err(AgError::Io)?;
        }
        return Ok(serde_json::json!({"reset": true}));
    }

    if !path.exists() {
        return Ok(serde_json::json!({"periods": []}));
    }

    let file = std::fs::File::open(&path).map_err(AgError::Io)?;
    let reader = std::io::BufReader::new(file);

    let mut total_calls = 0usize;
    let mut total_saved = 0usize;
    let mut cmd_map: std::collections::HashMap<String, (usize, usize, usize)> =
        std::collections::HashMap::new();

    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
            total_calls += 1;
            total_saved += entry
                .get("tokens_saved")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            if args.compare || args.verbose {
                let cmd = entry
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let actual = entry
                    .get("actual_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let baseline = entry
                    .get("baseline_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let e = cmd_map.entry(cmd).or_insert((0, 0, 0));
                e.0 += 1;
                e.1 += actual;
                e.2 += baseline;
            }
        }
    }

    let by_command = if args.compare || args.verbose {
        let mut cmds: Vec<CommandStats> = cmd_map
            .into_iter()
            .map(|(cmd, (calls, actual, baseline))| {
                let savings = if baseline > 0 {
                    ((baseline - actual.min(baseline)) as f64 / baseline as f64) * 100.0
                } else {
                    0.0
                };
                CommandStats {
                    command: cmd,
                    calls,
                    actual_tokens: actual / 4,
                    baseline_tokens: baseline / 4,
                    savings_pct: (savings * 10.0).round() / 10.0,
                }
            })
            .collect();
        cmds.sort_by_key(|c| std::cmp::Reverse(c.calls));
        Some(cmds)
    } else {
        None
    };

    let output = StatsOutput {
        periods: vec![PeriodStats {
            label: "all time".to_string(),
            calls: total_calls,
            tokens_saved: total_saved,
        }],
        by_command,
    };

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

pub struct StatEntry {
    pub command: String,
    pub actual_bytes: usize,
    pub baseline_bytes: usize,
    pub baseline_strategy: String,
    pub wall_ms: u64,
}

pub fn log_stat(command: &str, tokens_saved: usize) {
    log_stat_entry(&StatEntry {
        command: command.to_string(),
        actual_bytes: 0,
        baseline_bytes: tokens_saved * 4,
        baseline_strategy: "legacy".to_string(),
        wall_ms: 0,
    });
}

pub fn log_stat_entry(entry: &StatEntry) {
    log_stat_entry_to(entry, &stats_path());
}

fn log_stat_entry_to(entry: &StatEntry, path: &PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let saved = entry.baseline_bytes.saturating_sub(entry.actual_bytes);
    let json_entry = serde_json::json!({
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "command": entry.command,
        "actual_bytes": entry.actual_bytes,
        "baseline_bytes": entry.baseline_bytes,
        "baseline_strategy": entry.baseline_strategy,
        "wall_ms": entry.wall_ms,
        "tokens_saved": saved / 4,
    });

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{}", json_entry)
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(cmd: &str, actual: usize, baseline: usize) -> StatEntry {
        StatEntry {
            command: cmd.to_string(),
            actual_bytes: actual,
            baseline_bytes: baseline,
            baseline_strategy: "test".to_string(),
            wall_ms: 10,
        }
    }

    #[test]
    fn log_and_read() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("stats.jsonl");

        log_stat_entry_to(&test_entry("search", 100, 500), &path);
        log_stat_entry_to(&test_entry("read", 200, 1000), &path);
        log_stat_entry_to(&test_entry("search", 50, 300), &path);

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 3);

        let mut total_saved = 0usize;
        for line in content.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            total_saved += v["tokens_saved"].as_u64().unwrap() as usize;
        }
        assert!(total_saved > 0);
    }

    #[test]
    fn log_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("stats.jsonl");

        log_stat_entry_to(&test_entry("search", 100, 500), &path);
        assert!(path.exists());
    }

    #[test]
    fn log_stat_entry_has_required_fields() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("stats.jsonl");

        log_stat_entry_to(&test_entry("search", 100, 400), &path);
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

        assert_eq!(v["command"], "search");
        assert!(v["actual_bytes"].as_u64().unwrap() > 0);
        assert!(v["baseline_bytes"].as_u64().unwrap() > 0);
        assert!(v["tokens_saved"].as_u64().unwrap() > 0);
        assert!(v["ts"].as_u64().unwrap() > 0);
        assert_eq!(v["baseline_strategy"], "test");
    }
}
