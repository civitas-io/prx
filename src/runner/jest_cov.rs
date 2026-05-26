use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static COVERAGE_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^All files\s+\|\s+([\d.]+)\s+\|").unwrap());

static FILE_COV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+(\S+\.(?:ts|tsx|js|jsx))\s+\|\s+([\d.]+)\s+\|").unwrap());

static TEST_SUMMARY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Tests:\s+(\d+) passed, (\d+) total").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut overall_coverage = String::new();
    let mut warnings = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for line in output.lines() {
        if let Some(caps) = COVERAGE_TABLE_RE.captures(line) {
            overall_coverage = caps[1].to_string();
        }

        if let Some(caps) = FILE_COV_RE.captures(line) {
            let file = &caps[1];
            let pct_str = &caps[2];
            let pct: f32 = pct_str.parse().unwrap_or(100.0);
            if pct < 80.0 {
                warnings.push(Diagnostic {
                    name: file.to_string(),
                    location: None,
                    message: format!("{}% statement coverage", pct_str),
                });
            }
        }

        if let Some(caps) = TEST_SUMMARY_RE.captures(line) {
            passed = caps[1].parse().unwrap_or(0);
            let total: usize = caps[2].parse().unwrap_or(0);
            failed = total.saturating_sub(passed);
        }
    }

    let summary = if overall_coverage.is_empty() {
        format!("{passed} passed, {failed} failed")
    } else {
        format!("{}% statement coverage", overall_coverage)
    };

    ParsedResult {
        summary,
        passed,
        failed,
        skipped: 0,
        failures: vec![],
        warnings,
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_jest_coverage_table() {
        let output = "\
----------|---------|----------|---------|---------|
File      | % Stmts | % Branch | % Funcs | % Lines |
----------|---------|----------|---------|---------|
All files |   85.71 |      100 |   83.33 |   85.71 |
 auth.ts  |   71.42 |      100 |   66.66 |   71.42 |
 index.ts |  100    |      100 |  100    |  100    |
----------|---------|----------|---------|---------|

Test Suites: 2 passed, 2 total
Tests:       5 passed, 5 total
";
        let result = parse(output);
        assert!(result.summary.contains("85.71%"));
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].name, "auth.ts");
        assert!(result.warnings[0].message.contains("71.42%"));
        assert_eq!(result.passed, 5);
    }

    #[test]
    fn parse_jest_all_high_coverage() {
        let output = "\
----------|---------|----------|---------|---------|
File      | % Stmts | % Branch | % Funcs | % Lines |
----------|---------|----------|---------|---------|
All files |   95.00 |      100 |   95.00 |   95.00 |
 auth.ts  |   95.00 |      100 |   95.00 |   95.00 |
 index.ts |  100    |      100 |  100    |  100    |
----------|---------|----------|---------|---------|

Tests:       10 passed, 10 total
";
        let result = parse(output);
        assert!(result.summary.contains("95.00%"));
        assert!(result.warnings.is_empty());
        assert_eq!(result.passed, 10);
    }

    #[test]
    fn parse_jest_no_coverage_table() {
        let output = "Tests:       3 passed, 3 total\n";
        let result = parse(output);
        assert_eq!(result.passed, 3);
        assert!(!result.summary.contains("%"));
    }
}
