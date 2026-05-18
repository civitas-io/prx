pub mod cargo_build;
pub mod cargo_test;
pub mod eslint;
pub mod fallback;
pub mod go_test;
pub mod jest;
pub mod pytest;
pub mod tsc;

use serde::Serialize;
use std::time::Instant;

use crate::output::AgError;

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct RunOutput {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub tool: String,
    pub summary: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub output_lines: usize,
    pub output_tokens_saved: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<String>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub message: String,
}

pub struct RawOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

pub fn execute(command: &[String], _timeout_secs: u64) -> Result<RawOutput, AgError> {
    if command.is_empty() {
        return Err(AgError::InvalidArgument {
            flag: "command".to_string(),
            message: "no command provided".to_string(),
        });
    }

    let start = Instant::now();
    let result = std::process::Command::new(&command[0])
        .args(&command[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let child = match result {
        Ok(c) => c,
        Err(e) => {
            return Err(AgError::Internal {
                message: format!("failed to spawn `{}`: {e}", command[0]),
            });
        }
    };

    let output = child.wait_with_output().map_err(|e| AgError::Internal {
        message: format!("failed to wait for command: {e}"),
    })?;

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(RawOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms,
    })
}

pub fn detect_tool(command: &[String]) -> &'static str {
    let cmd = command.join(" ").to_lowercase();

    if cmd.starts_with("cargo test") {
        return "cargo_test";
    }
    if cmd.starts_with("cargo clippy") {
        return "cargo_clippy";
    }
    if cmd.starts_with("cargo build") || cmd.starts_with("cargo check") {
        return "cargo_build";
    }
    if cmd.contains("pytest") || cmd.contains("python -m pytest") {
        return "pytest";
    }
    if cmd.starts_with("go test") {
        return "go_test";
    }
    if cmd.contains("vitest") {
        return "jest";
    }
    if cmd.contains("jest") || cmd.starts_with("npm test") || cmd.starts_with("npx jest") {
        return "jest";
    }
    if cmd.starts_with("tsc") || cmd.starts_with("npx tsc") {
        return "tsc";
    }
    if cmd.contains("eslint") {
        return "eslint";
    }

    "unknown"
}

pub fn parse_output(tool: &str, raw: &RawOutput) -> RunOutput {
    let combined = format!("{}\n{}", raw.stdout, raw.stderr);
    let output_lines = combined.lines().count();
    let raw_tokens = combined.len() / 4;

    let parsed = match tool {
        "cargo_test" => cargo_test::parse(&combined),
        "cargo_build" | "cargo_clippy" => cargo_build::parse(&combined),
        "pytest" => pytest::parse(&combined),
        "go_test" => go_test::parse(&combined),
        "jest" => jest::parse(&combined),
        "tsc" => tsc::parse(&combined),
        "eslint" => eslint::parse(&combined),
        _ => fallback::parse(&combined, raw.exit_code),
    };

    let parsed_tokens = estimate_parsed_tokens(&parsed);

    RunOutput {
        exit_code: raw.exit_code,
        duration_ms: raw.duration_ms,
        tool: tool.to_string(),
        summary: parsed.summary,
        passed: parsed.passed,
        failed: parsed.failed,
        skipped: parsed.skipped,
        failures: parsed.failures,
        warnings: parsed.warnings,
        output_lines,
        output_tokens_saved: raw_tokens.saturating_sub(parsed_tokens),
        tail: parsed.tail,
    }
}

fn estimate_parsed_tokens(parsed: &ParsedResult) -> usize {
    let mut total = parsed.summary.len();
    for d in &parsed.failures {
        total += d.name.len() + d.message.len() + d.location.as_ref().map_or(0, |l| l.len());
    }
    for d in &parsed.warnings {
        total += d.name.len() + d.message.len() + d.location.as_ref().map_or(0, |l| l.len());
    }
    if let Some(ref t) = parsed.tail {
        total += t.len();
    }
    total / 4
}

pub struct ParsedResult {
    pub summary: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub tail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_cargo_test() {
        let cmd = vec!["cargo".into(), "test".into()];
        assert_eq!(detect_tool(&cmd), "cargo_test");
    }

    #[test]
    fn detect_cargo_clippy() {
        let cmd = vec!["cargo".into(), "clippy".into()];
        assert_eq!(detect_tool(&cmd), "cargo_clippy");
    }

    #[test]
    fn detect_pytest() {
        let cmd = vec!["pytest".into(), "-v".into()];
        assert_eq!(detect_tool(&cmd), "pytest");
    }

    #[test]
    fn detect_go_test() {
        let cmd = vec!["go".into(), "test".into(), "./...".into()];
        assert_eq!(detect_tool(&cmd), "go_test");
    }

    #[test]
    fn detect_jest() {
        let cmd = vec!["npx".into(), "jest".into()];
        assert_eq!(detect_tool(&cmd), "jest");
    }

    #[test]
    fn detect_vitest() {
        let cmd = vec!["npx".into(), "vitest".into()];
        assert_eq!(detect_tool(&cmd), "jest");
    }

    #[test]
    fn detect_npm_test() {
        let cmd = vec!["npm".into(), "test".into()];
        assert_eq!(detect_tool(&cmd), "jest");
    }

    #[test]
    fn detect_tsc() {
        let cmd = vec!["tsc".into(), "--noEmit".into()];
        assert_eq!(detect_tool(&cmd), "tsc");
    }

    #[test]
    fn detect_eslint() {
        let cmd = vec!["npx".into(), "eslint".into(), "src/".into()];
        assert_eq!(detect_tool(&cmd), "eslint");
    }

    #[test]
    fn detect_unknown() {
        let cmd = vec!["echo".into(), "hello".into()];
        assert_eq!(detect_tool(&cmd), "unknown");
    }

    #[test]
    fn execute_echo() {
        let raw = execute(&["echo".into(), "hello".into()], 10).unwrap();
        assert_eq!(raw.exit_code, 0);
        assert!(raw.stdout.contains("hello"));
        assert!(raw.duration_ms < 5000);
    }

    #[test]
    fn execute_failing() {
        let raw = execute(&["false".into()], 10).unwrap();
        assert_ne!(raw.exit_code, 0);
    }

    #[test]
    fn execute_nonexistent() {
        let result = execute(&["ag_nonexistent_binary_xyz".into()], 10);
        assert!(result.is_err());
    }

    #[test]
    fn execute_empty_command() {
        let result = execute(&[], 10);
        assert!(result.is_err());
    }

    #[test]
    fn parse_output_fallback() {
        let raw = RawOutput {
            exit_code: 0,
            stdout: "hello\nworld\n".to_string(),
            stderr: String::new(),
            duration_ms: 10,
        };
        let output = parse_output("unknown", &raw);
        assert_eq!(output.tool, "unknown");
        assert!(output.tail.is_some());
    }

    #[test]
    fn parse_output_cargo_test() {
        let raw = RawOutput {
            exit_code: 0,
            stdout: "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n".to_string(),
            stderr: String::new(),
            duration_ms: 10,
        };
        let output = parse_output("cargo_test", &raw);
        assert_eq!(output.passed, 5);
        assert_eq!(output.failed, 0);
    }

    #[test]
    fn tokens_saved_positive() {
        let raw = RawOutput {
            exit_code: 0,
            stdout: "test a ... ok\ntest b ... ok\ntest c ... ok\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n".to_string(),
            stderr: String::new(),
            duration_ms: 10,
        };
        let output = parse_output("cargo_test", &raw);
        assert!(output.output_tokens_saved > 0);
    }
}
