use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

// 42:18  error  Unexpected any  @typescript-eslint/no-explicit-any
static ISSUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s{2,}(\S+)\s*$").unwrap()
});

static SUMMARY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+) problems? \((\d+) errors?, (\d+) warnings?\)").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut current_file = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !trimmed.starts_with(char::is_whitespace)
            && !trimmed.starts_with('✖')
            && !trimmed.starts_with('×')
            && !ISSUE_RE.is_match(line)
            && !SUMMARY_RE.is_match(line)
        {
            current_file = trimmed.to_string();
        }

        if let Some(caps) = ISSUE_RE.captures(line) {
            let location = if current_file.is_empty() {
                format!("{}:{}", &caps[1], &caps[2])
            } else {
                format!("{}:{}:{}", current_file, &caps[1], &caps[2])
            };

            let diag = Diagnostic {
                name: caps[5].to_string(),
                location: Some(location),
                message: caps[4].to_string(),
            };

            if &caps[3] == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
        }
    }

    let summary = if failures.is_empty() && warnings.is_empty() {
        "no issues".to_string()
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
    fn parse_eslint_errors() {
        let output = "\
/Users/user/project/src/auth.ts
  42:18  error  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any
  55:1   warning  Missing return type                      @typescript-eslint/explicit-function-return-type

✖ 2 problems (1 error, 1 warning)
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.failures[0].name,
            "@typescript-eslint/no-explicit-any"
        );
    }

    #[test]
    fn parse_eslint_clean() {
        let result = parse("");
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("no issues"));
    }
}
