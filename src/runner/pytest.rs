use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(SUMMARY_RE, r"=+ (.+) =+\s*$");
define_regex!(PASSED_RE, r"(\d+) passed");
define_regex!(FAILED_RE, r"(\d+) failed");
define_regex!(SKIPPED_RE, r"(\d+) skipped");
define_regex!(FAILURE_LINE_RE, r"^FAILED (.+?) - (.+)$");

pub fn parse(output: &str) -> ParsedResult {
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut summary = String::new();
    let mut failures = Vec::new();

    for line in output.lines() {
        if let Some(caps) = SUMMARY_RE.captures(line) {
            let summary_text = &caps[1];
            if let Some(p) = PASSED_RE.captures(summary_text) {
                passed = p[1].parse().unwrap_or(0);
            }
            if let Some(f) = FAILED_RE.captures(summary_text) {
                failed = f[1].parse().unwrap_or(0);
            }
            if let Some(s) = SKIPPED_RE.captures(summary_text) {
                skipped = s[1].parse().unwrap_or(0);
            }
            summary = summary_text.trim().to_string();
        }

        if let Some(caps) = FAILURE_LINE_RE.captures(line) {
            failures.push(Diagnostic {
                name: caps[1].to_string(),
                location: None,
                message: caps[2].to_string(),
            });
        }
    }

    if summary.is_empty() {
        summary = format!("{passed} passed, {failed} failed");
    }

    ParsedResult::new(summary, passed, failed, skipped, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_passing() {
        let output =
            "============================= 5 passed in 0.12s =============================\n";
        let result = parse(output);
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn parse_with_failures() {
        let output = "\
FAILED tests/test_auth.py::test_login - AssertionError: assert False == True
FAILED tests/test_auth.py::test_signup - AssertionError: expected 200
======= 3 passed, 2 failed in 0.45s =======
";
        let result = parse(output);
        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 2);
        assert_eq!(result.failures.len(), 2);
        assert!(result.failures[0].name.contains("test_login"));
    }

    #[test]
    fn parse_with_skipped() {
        let output = "============================= 10 passed, 2 skipped in 0.5s =============================\n";
        let result = parse(output);
        assert_eq!(result.passed, 10);
        assert_eq!(result.skipped, 2);
    }
}
