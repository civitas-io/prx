use std::path::PathBuf;

use crate::commands::Commands;

pub fn can_fallback(command: &str) -> bool {
    matches!(
        command,
        "search" | "read" | "find" | "exists" | "outline" | "diff" | "run"
    )
}

pub fn fallback_spec(command: &str, cli: &Commands) -> Option<(String, Vec<String>)> {
    match (command, cli) {
        ("search", Commands::Search(args)) => Some((
            "grep".into(),
            vec!["-rn".into(), args.query.clone(), args.path.clone()],
        )),
        ("read", Commands::Read(args)) => {
            if let Some(ref lines) = args.lines {
                let parts: Vec<&str> = lines.split('-').collect();
                if parts.len() == 2 {
                    return Some((
                        "sed".into(),
                        vec![
                            "-n".into(),
                            format!("{},{}p", parts[0], parts[1]),
                            args.file.clone(),
                        ],
                    ));
                }
            }
            Some(("cat".into(), vec![args.file.clone()]))
        }
        ("find", Commands::Find(args)) => {
            let mut cmd_args = vec![args.path.clone(), "-type".into(), "f".into()];
            if let Some(ref pattern) = args.pattern {
                cmd_args.extend(["-name".into(), pattern.clone()]);
            }
            Some(("find".into(), cmd_args))
        }
        ("exists", Commands::Exists(args)) => Some((
            "grep".into(),
            vec!["-rl".into(), args.pattern.clone(), args.path.clone()],
        )),
        ("outline", Commands::Outline(args)) => Some((
            "grep".into(),
            vec![
                "-n".into(),
                r"fn \|struct \|impl \|enum \|trait \|class \|def \|function ".into(),
                args.path.clone(),
            ],
        )),
        ("diff", Commands::Diff(args)) => {
            let mut cmd_args = vec!["diff".into()];
            if args.staged {
                cmd_args.push("--staged".into());
            } else {
                cmd_args.push(args.since.clone());
            }
            Some(("git".into(), cmd_args))
        }
        ("run", Commands::Run(args)) => {
            if args.command.is_empty() {
                return None;
            }
            Some((args.command[0].clone(), args.command[1..].to_vec()))
        }
        _ => None,
    }
}

pub fn execute_fallback(cmd: &str, args: &[String]) -> Option<serde_json::Value> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .ok()?;

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let fallback_cmd = format!("{} {}", cmd, args.join(" "));

    Some(serde_json::json!({
        "raw": raw,
        "source": fallback_cmd,
    }))
}

fn errors_path() -> PathBuf {
    if let Ok(custom) = std::env::var("PRX_ERRORS_FILE") {
        return PathBuf::from(custom);
    }
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".prx")
        .join("errors.jsonl")
}

pub fn log_error(command: &str, error: &str, fallback_cmd: &str, fallback_bytes: usize) {
    let path = errors_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let entry = serde_json::json!({
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "command": command,
        "error": error,
        "fallback_cmd": fallback_cmd,
        "fallback_bytes": fallback_bytes,
    });

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{}", entry)
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_fallback_eligible() {
        assert!(can_fallback("search"));
        assert!(can_fallback("read"));
        assert!(can_fallback("find"));
        assert!(can_fallback("exists"));
        assert!(can_fallback("outline"));
        assert!(can_fallback("diff"));
        assert!(can_fallback("run"));
    }

    #[test]
    fn can_fallback_ineligible() {
        assert!(!can_fallback("edit"));
        assert!(!can_fallback("mcp"));
        assert!(!can_fallback("init"));
        assert!(!can_fallback("stats"));
        assert!(!can_fallback("bench"));
        assert!(!can_fallback("batch"));
        assert!(!can_fallback("index"));
    }

    #[test]
    fn log_error_writes_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("errors.jsonl");

        let entry = serde_json::json!({
            "ts": 123,
            "command": "search",
            "error": "test error",
            "fallback_cmd": "grep -rn test .",
            "fallback_bytes": 500,
        });

        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "{}", entry)
            });

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(v["command"], "search");
        assert_eq!(v["error"], "test error");
    }

    #[test]
    fn fallback_search_command() {
        let args = crate::commands::search::SearchArgs {
            query: "test_pattern".into(),
            path: "src/".into(),
            literal: false,
            semantic: false,
            structural: false,
            top_k: 5,
            budget: None,
            continue_token: None,
            alpha: None,
        };
        let cli = Commands::Search(args);
        let (cmd, cmd_args) = fallback_spec("search", &cli).unwrap();
        assert_eq!(cmd, "grep");
        assert!(cmd_args.contains(&"test_pattern".to_string()));
    }

    #[test]
    fn fallback_read_command() {
        let args = crate::commands::read::ReadArgs {
            file: "src/main.rs".into(),
            lines: None,
            snap: None,
            skeleton: false,
            outline: false,
            hash: false,
            budget: None,
            if_changed: None,
            mode: None,
        };
        let cli = Commands::Read(args);
        let (cmd, cmd_args) = fallback_spec("read", &cli).unwrap();
        assert_eq!(cmd, "cat");
        assert_eq!(cmd_args, vec!["src/main.rs"]);
    }

    #[test]
    fn fallback_find_with_pattern() {
        let args = crate::commands::find::FindArgs {
            path: "src/".into(),
            pattern: Some("*.rs".into()),
            depth: None,
            related_to: None,
            changed_since: None,
            outline: false,
            tree: false,
            flat: false,
            budget: None,
        };
        let cli = Commands::Find(args);
        let (cmd, cmd_args) = fallback_spec("find", &cli).unwrap();
        assert_eq!(cmd, "find");
        assert!(cmd_args.contains(&"*.rs".to_string()));
    }

    #[test]
    fn fallback_run_passes_through() {
        let args = crate::commands::run::RunArgs {
            command: vec!["cargo".into(), "test".into()],
            raw: false,
            full: false,
            auto_json: false,
            timeout: 300,
        };
        let cli = Commands::Run(args);
        let (cmd, cmd_args) = fallback_spec("run", &cli).unwrap();
        assert_eq!(cmd, "cargo");
        assert_eq!(cmd_args, vec!["test"]);
    }
}
