use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static TOTAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^TOTAL\s+\d+\s+\d+\s+([\d.]+%)").unwrap());

static FILE_COV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+\.py)\s+\d+\s+\d+\s+([\d.]+%)").unwrap());

static STMTS_MISSED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^TOTAL\s+(\d+)\s+(\d+)").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut total_coverage = String::new();
    let mut total_stmts = 0;
    let mut total_missed = 0;
    let mut warnings = Vec::new();

    for line in output.lines() {
        if let Some(caps) = TOTAL_RE.captures(line) {
            total_coverage = caps[1].to_string();
        }

        if let Some(caps) = STMTS_MISSED_RE.captures(line) {
            total_stmts = caps[1].parse().unwrap_or(0);
            total_missed = caps[2].parse().unwrap_or(0);
        }

        if let Some(caps) = FILE_COV_RE.captures(line) {
            let file = &caps[1];
            let pct_str = &caps[2];
            let pct: f32 = pct_str.trim_end_matches('%').parse().unwrap_or(100.0);
            if pct < 80.0 {
                warnings.push(Diagnostic {
                    name: file.to_string(),
                    location: None,
                    message: format!("{pct_str} coverage"),
                });
            }
        }
    }

    let summary = if total_coverage.is_empty() {
        "no coverage data".to_string()
    } else {
        format!(
            "{} total coverage ({} stmts, {} missed)",
            total_coverage, total_stmts, total_missed
        )
    };

    ParsedResult {
        summary,
        passed: 0,
        failed: 0,
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
    fn parse_pytest_cov_with_header() {
        let output = "\
---------- coverage: platform linux, python 3.11 ----------
Name                      Stmts   Miss  Cover
----------------------------------------------
src/auth.py                  42     12    71%
src/handler.py              100      5    95%
src/__init__.py               2      0   100%
----------------------------------------------
TOTAL                       144     17    88%
";
        let result = parse(output);
        assert!(result.summary.contains("88%"));
        assert!(result.summary.contains("144 stmts"));
        assert!(result.summary.contains("17 missed"));
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].name, "src/auth.py");
        assert!(result.warnings[0].message.contains("71%"));
    }

    #[test]
    fn parse_coverage_report_format() {
        let output = "\
Name                      Stmts   Miss  Cover
----------------------------------------------
src/auth.py                  42     12    71%
src/handler.py              100      5    95%
----------------------------------------------
TOTAL                       144     17    88%
";
        let result = parse(output);
        assert!(result.summary.contains("88%"));
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn parse_all_high_coverage() {
        let output = "\
Name                      Stmts   Miss  Cover
----------------------------------------------
src/auth.py                  42      2    95%
src/handler.py              100      5    95%
----------------------------------------------
TOTAL                       144      7    95%
";
        let result = parse(output);
        assert!(result.summary.contains("95%"));
        assert!(result.warnings.is_empty());
    }
}
