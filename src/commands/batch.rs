use std::io::BufRead;

use clap::Args;
use serde::Serialize;

use crate::output::AgError;

#[derive(Args)]
pub struct BatchArgs {}

#[derive(Serialize)]
struct BatchResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn run(_args: BatchArgs) -> Result<serde_json::Value, AgError> {
    let stdin = std::io::stdin();
    let mut results = Vec::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                results.push(BatchResult {
                    id: None,
                    status: "error".to_string(),
                    data: None,
                    error: Some(format!("read error: {e}")),
                });
                continue;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cmd: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                results.push(BatchResult {
                    id: None,
                    status: "error".to_string(),
                    data: None,
                    error: Some(format!("invalid JSON: {e}")),
                });
                continue;
            }
        };

        let id = cmd.get("id").and_then(|v| v.as_str()).map(String::from);
        let result = dispatch_command(&cmd);

        match result {
            Ok(data) => results.push(BatchResult {
                id,
                status: "ok".to_string(),
                data: Some(data),
                error: None,
            }),
            Err(e) => results.push(BatchResult {
                id,
                status: "error".to_string(),
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    serde_json::to_value(serde_json::json!({"results": results})).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn dispatch_command(cmd: &serde_json::Value) -> Result<serde_json::Value, AgError> {
    let command =
        cmd.get("cmd")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgError::InvalidArgument {
                flag: "cmd".to_string(),
                message: "missing 'cmd' field".to_string(),
            })?;

    match command {
        "search" => {
            let query = cmd.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let path = cmd.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let top_k = cmd.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let budget = cmd
                .get("budget")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            super::search::run(super::search::SearchArgs {
                query: query.to_string(),
                path: path.to_string(),
                literal: false,
                semantic: false,
                structural: false,
                top_k,
                budget,
                context: None,
                exists: false,
                continue_token: None,
                alpha: None,
            })
        }
        "read" => {
            let file = cmd
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let skeleton = cmd
                .get("skeleton")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let hash = cmd.get("hash").and_then(|v| v.as_bool()).unwrap_or(false);
            let outline = cmd
                .get("outline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            super::read::run(super::read::ReadArgs {
                file,
                lines: None,
                snap: None,
                skeleton,
                outline,
                hash,
                budget: cmd
                    .get("budget")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                meta: false,
                if_changed: cmd
                    .get("if_changed")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
        }
        "exists" => {
            let pattern = cmd
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path = cmd
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string();
            super::exists::run(super::exists::ExistsArgs { pattern, path })
        }
        "find" => {
            let path = cmd
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string();
            super::find::run(super::find::FindArgs {
                path,
                pattern: cmd
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                depth: cmd
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                related_to: None,
                changed_since: None,
                outline: false,
                tree: false,
                flat: false,
                budget: cmd
                    .get("budget")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
            })
        }
        "outline" => {
            let path = cmd
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            super::outline::run(super::outline::OutlineArgs {
                path,
                depth: None,
                kind: None,
            })
        }
        _ => Err(AgError::InvalidArgument {
            flag: "cmd".to_string(),
            message: format!("unknown command: {command}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_unknown_command() {
        let cmd = serde_json::json!({"cmd": "unknown_xyz"});
        assert!(dispatch_command(&cmd).is_err());
    }

    #[test]
    fn dispatch_missing_cmd() {
        let cmd = serde_json::json!({"query": "test"});
        assert!(dispatch_command(&cmd).is_err());
    }

    #[test]
    fn dispatch_find() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        let cmd = serde_json::json!({"cmd": "find", "path": dir.path().to_str().unwrap()});
        let result = dispatch_command(&cmd).unwrap();
        assert!(result["stats"]["total_files"].as_u64().unwrap() >= 1);
    }
}
