use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static ERROR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(error|warning)(\[E\d+\])?: (.+)$").unwrap());

static LOCATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+--> (.+):(\d+):(\d+)$").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(caps) = ERROR_RE.captures(lines[i]) {
            let severity = caps[1].to_string();
            let code = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let message = caps[3].to_string();

            let mut location = None;
            if i + 1 < lines.len() {
                if let Some(loc_caps) = LOCATION_RE.captures(lines[i + 1]) {
                    location = Some(format!(
                        "{}:{}:{}",
                        &loc_caps[1], &loc_caps[2], &loc_caps[3]
                    ));
                }
            }

            let diag = Diagnostic {
                name: format!("{severity}{code}"),
                location,
                message,
            };

            if severity == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
        }
        i += 1;
    }

    let summary = if failures.is_empty() && warnings.is_empty() {
        "build succeeded".to_string()
    } else {
        format!("{} error(s), {} warning(s)", failures.len(), warnings.len())
    };

    ParsedResult {
        summary,
        passed: 0,
        failed: failures.len(),
        skipped: 0,
        failures,
        warnings,
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_build_success() {
        let output = "   Compiling prx v0.2.0\n    Finished `dev` profile in 1.5s\n";
        let result = parse(output);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("succeeded"));
    }

    #[test]
    fn parse_build_error() {
        let output = "\
error[E0382]: borrow of partially moved value
  --> src/main.rs:30:37
   |
30 |         Ok(data) => write_envelope(&cli.command.name(), data, cli.plain),
   |                                     ^^^^^^^^^^^ value borrowed here after partial move
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].name.contains("E0382"));
        assert!(
            result.failures[0]
                .location
                .as_ref()
                .unwrap()
                .contains("src/main.rs:30")
        );
    }

    #[test]
    fn parse_clippy_warning() {
        let output = "warning: unused variable: `x`\n  --> src/lib.rs:10:9\n";
        let result = parse(output);
        assert_eq!(result.warnings.len(), 1);
        assert!(
            result.warnings[0]
                .location
                .as_ref()
                .unwrap()
                .contains("src/lib.rs:10")
        );
    }
}
