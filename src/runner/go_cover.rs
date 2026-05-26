use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static COVERAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"coverage:\s+([\d.]+)%\s+of\s+statements").unwrap());

static PACKAGE_RESULT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(ok|FAIL)\s+(\S+)\s+[\d.]+s\s+coverage:\s+([\d.]+)%").unwrap());

static SINGLE_PKG_COVERAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(PASS|FAIL)\s*$").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut coverages = Vec::new();
    let mut failed_packages = Vec::new();
    let mut single_coverage = String::new();

    let lines: Vec<&str> = output.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        // Multi-package format: ok/FAIL package coverage: X% of statements
        if let Some(caps) = PACKAGE_RESULT_RE.captures(line) {
            let status = &caps[1];
            let package = &caps[2];
            let cov_str = &caps[3];
            let cov: f32 = cov_str.parse().unwrap_or(0.0);

            coverages.push(cov);

            if status == "FAIL" || cov < 80.0 {
                failed_packages.push(Diagnostic {
                    name: package.to_string(),
                    location: None,
                    message: format!("{}% coverage", cov_str),
                });
            }
        }

        // Single package format: PASS/FAIL on one line, coverage: X% on next
        if SINGLE_PKG_COVERAGE_RE.captures(line).is_some() && i + 1 < lines.len() {
            if let Some(caps) = COVERAGE_RE.captures(lines[i + 1]) {
                single_coverage = caps[1].to_string();
                coverages.push(caps[1].parse().unwrap_or(0.0));
            }
        }
    }

    let summary = if !coverages.is_empty() {
        let avg_coverage: f32 = coverages.iter().sum::<f32>() / coverages.len() as f32;
        let pkg_count = coverages.len();
        format!(
            "{:.1}% coverage across {} packages",
            avg_coverage, pkg_count
        )
    } else if !single_coverage.is_empty() {
        format!("{} coverage", single_coverage)
    } else {
        "no coverage data".to_string()
    };

    ParsedResult {
        summary,
        passed: if failed_packages.is_empty() { 1 } else { 0 },
        failed: failed_packages.len(),
        skipped: 0,
        failures: failed_packages,
        warnings: vec![],
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_test_multi_package() {
        let output = "\
ok      github.com/user/pkg/auth    0.045s  coverage: 85.2% of statements
ok      github.com/user/pkg/handler 0.023s  coverage: 92.1% of statements
FAIL    github.com/user/pkg/broken  0.001s  coverage: 0.0% of statements
";
        let result = parse(output);
        assert!(result.summary.contains("coverage"));
        assert!(result.summary.contains("3 packages"));
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn parse_go_test_single_package() {
        let output = "\
PASS
coverage: 85.2% of statements
ok      github.com/user/pkg 0.045s
";
        let result = parse(output);
        assert!(result.summary.contains("85.2%"));
    }

    #[test]
    fn parse_go_test_all_high_coverage() {
        let output = "\
ok      github.com/user/pkg/auth    0.045s  coverage: 95.0% of statements
ok      github.com/user/pkg/handler 0.023s  coverage: 92.1% of statements
";
        let result = parse(output);
        assert!(result.summary.contains("coverage"));
        assert_eq!(result.failed, 0);
    }
}
