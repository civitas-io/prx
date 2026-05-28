use clap::Args;

use crate::output::AgError;
use crate::runner;

#[derive(Args)]
pub struct RunArgs {
    /// Command to run
    #[arg(trailing_var_arg = true, required = true)]
    pub command: Vec<String>,

    /// Bypass parsing, return full output
    #[arg(long)]
    pub raw: bool,

    /// Return parsed summary AND full output
    #[arg(long)]
    pub full: bool,

    /// Auto-inject --json/-o json for tools that support structured output
    #[arg(long)]
    pub auto_json: bool,

    /// Command timeout in seconds
    #[arg(long, default_value = "300")]
    pub timeout: u64,
}

pub fn run(args: RunArgs) -> Result<serde_json::Value, AgError> {
    let command = if args.auto_json {
        runner::inject_json_flag(&args.command)
    } else {
        args.command.clone()
    };
    let raw = runner::execute(&command, args.timeout)?;

    if args.raw {
        let combined = format!("{}\n{}", raw.stdout, raw.stderr);
        let output = runner::RunOutput {
            exit_code: raw.exit_code,
            duration_ms: raw.duration_ms,
            tool: "raw".to_string(),
            summary: format!("exited {}", raw.exit_code),
            passed: 0,
            failed: if raw.exit_code != 0 { 1 } else { 0 },
            skipped: 0,
            failures: vec![],
            warnings: vec![],
            output_lines: combined.lines().count(),
            output_tokens_saved: 0,
            tail: Some(combined),
        };
        return serde_json::to_value(output).map_err(|e| AgError::Internal {
            message: e.to_string(),
        });
    }

    let tool = runner::detect_tool(&command);
    let mut output = runner::parse_output(tool, &raw);

    if args.full {
        let combined = format!("{}\n{}", raw.stdout, raw.stderr);
        output.tail = Some(combined);
    }

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo() {
        let args = RunArgs {
            command: vec!["echo".into(), "hello".into()],
            raw: false,
            full: false,
            auto_json: false,
            timeout: 10,
        };
        let result = run(args).unwrap();
        let out: runner::RunOutput = serde_json::from_value(result).unwrap();
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.tool, "unknown");
    }

    #[test]
    fn run_raw_mode() {
        let args = RunArgs {
            command: vec!["echo".into(), "hello".into()],
            raw: true,
            full: false,
            auto_json: false,
            timeout: 10,
        };
        let result = run(args).unwrap();
        let out: runner::RunOutput = serde_json::from_value(result).unwrap();
        assert_eq!(out.tool, "raw");
        assert!(out.tail.unwrap().contains("hello"));
    }

    #[test]
    fn run_failing_command() {
        let args = RunArgs {
            command: vec!["false".into()],
            raw: false,
            full: false,
            auto_json: false,
            timeout: 10,
        };
        let result = run(args).unwrap();
        let out: runner::RunOutput = serde_json::from_value(result).unwrap();
        assert_ne!(out.exit_code, 0);
    }

    #[test]
    fn run_nonexistent_command() {
        let args = RunArgs {
            command: vec!["ag_nonexistent_cmd_xyz".into()],
            raw: false,
            full: false,
            auto_json: false,
            timeout: 10,
        };
        assert!(run(args).is_err());
    }
}
