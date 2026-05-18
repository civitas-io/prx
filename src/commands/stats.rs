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

    /// Clear saved statistics
    #[arg(long)]
    pub reset: bool,
}

#[derive(Serialize)]
struct StatsOutput {
    periods: Vec<PeriodStats>,
}

#[derive(Serialize)]
struct PeriodStats {
    label: String,
    calls: usize,
    tokens_saved: usize,
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

    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
            total_calls += 1;
            total_saved += entry
                .get("tokens_saved")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
        }
    }

    let output = StatsOutput {
        periods: vec![PeriodStats {
            label: "all time".to_string(),
            calls: total_calls,
            tokens_saved: total_saved,
        }],
    };

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

pub fn log_stat(command: &str, tokens_saved: usize) {
    log_stat_to(command, tokens_saved, &stats_path());
}

fn log_stat_to(command: &str, tokens_saved: usize, path: &PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let entry = serde_json::json!({
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "command": command,
        "tokens_saved": tokens_saved,
    });

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{}", entry)
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_and_read() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("stats.jsonl");

        log_stat_to("search", 100, &path);
        log_stat_to("read", 50, &path);
        log_stat_to("search", 200, &path);

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 3);

        let mut total_saved = 0usize;
        for line in content.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            total_saved += v["tokens_saved"].as_u64().unwrap() as usize;
        }
        assert_eq!(total_saved, 350);
    }

    #[test]
    fn log_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("stats.jsonl");

        log_stat_to("search", 42, &path);
        assert!(path.exists());
    }

    #[test]
    fn log_stat_entry_has_required_fields() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("stats.jsonl");

        log_stat_to("search", 123, &path);
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

        assert_eq!(v["command"], "search");
        assert_eq!(v["tokens_saved"], 123);
        assert!(v["ts"].as_u64().unwrap() > 0);
    }
}
