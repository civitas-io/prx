use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static TEST_SUMMARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored").unwrap()
});

static TOTAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^TOTAL\s+\d+\s+\d+\s+([\d.]+%)").unwrap());

static FILE_COV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+\.rs)\s+\d+\s+\d+\s+([\d.]+%)").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut total_coverage = String::new();
    let mut low_coverage = Vec::new();

    for line in output.lines() {
        if let Some(caps) = TEST_SUMMARY_RE.captures(line) {
            passed = caps[2].parse().unwrap_or(0);
            failed = caps[3].parse().unwrap_or(0);
            skipped = caps[4].parse().unwrap_or(0);
        }

        if let Some(caps) = TOTAL_RE.captures(line) {
            total_coverage = caps[1].to_string();
        }

        if let Some(caps) = FILE_COV_RE.captures(line) {
            let file = &caps[1];
            let pct_str = &caps[2];
            let pct: f32 = pct_str.trim_end_matches('%').parse().unwrap_or(100.0);
            if pct < 80.0 {
                low_coverage.push(Diagnostic {
                    name: file.to_string(),
                    location: None,
                    message: format!("{pct_str} line coverage"),
                });
            }
        }
    }

    let summary = if total_coverage.is_empty() {
        format!("{passed} passed, {failed} failed")
    } else {
        format!("{total_coverage} coverage, {passed} passed, {failed} failed")
    };

    ParsedResult {
        summary,
        passed,
        failed,
        skipped,
        failures: low_coverage,
        warnings: vec![],
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coverage_summary() {
        let output = "\
running 10 tests
test foo ... ok
test bar ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Filename                   Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover    Branches   Missed Branches     Cover
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
src/auth.rs                     50                 5    90.00%          10                 0   100.00%         200                20    90.00%           0                 0         -
src/handler.rs                  30                15    50.00%           5                 2    60.00%         100                50    50.00%           0                 0         -
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
TOTAL                           80                20    75.00%          15                 2    86.67%         300                70    76.67%           0                 0         -
";
        let result = parse(output);
        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("75.00%"));
        assert!(result.summary.contains("coverage"));
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "src/handler.rs");
        assert!(result.failures[0].message.contains("50.00%"));
    }

    #[test]
    fn parse_all_high_coverage() {
        let output = "\
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

TOTAL                          100                 5    95.00%          20                 0   100.00%         500                25    95.00%           0                 0         -
";
        let result = parse(output);
        assert_eq!(result.passed, 5);
        assert!(result.summary.contains("95.00%"));
        assert!(result.failures.is_empty());
    }

    #[test]
    fn parse_no_coverage_table() {
        let output = "test result: ok. 3 passed; 0 failed; 0 ignored\n";
        let result = parse(output);
        assert_eq!(result.passed, 3);
        assert!(!result.summary.contains("coverage"));
    }
}
