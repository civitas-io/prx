use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    ISSUE_RE,
    r"^(.+?\.pyi?):(\d+)(?::(\d+))?: (error|warning|note): (.+)$"
);
define_regex!(SUMMARY_RE, r"^Found (\d+) errors? in (\d+) files?");
define_regex!(SUCCESS_RE, r"^Success: no issues");

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut summary = String::new();

    for line in output.lines() {
        if let Some(caps) = ISSUE_RE.captures(line) {
            let severity = &caps[4];
            if severity == "note" {
                continue;
            }
            let location = if let Some(col) = caps.get(3) {
                format!("{}:{}:{}", &caps[1], &caps[2], col.as_str())
            } else {
                format!("{}:{}", &caps[1], &caps[2])
            };

            let diag = Diagnostic {
                name: format!("mypy/{severity}"),
                location: Some(location),
                message: caps[5].to_string(),
            };

            if severity == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
            continue;
        }

        if let Some(caps) = SUMMARY_RE.captures(line) {
            summary = format!(
                "{} error(s) in {} file(s)",
                &caps[1].to_string(),
                &caps[2].to_string()
            );
        } else if SUCCESS_RE.is_match(line) {
            summary = "no issues found".to_string();
        }
    }

    if summary.is_empty() {
        summary = format!("{} error(s), {} warning(s)", failures.len(), warnings.len());
    }

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
    fn parse_mypy_errors() {
        let output = "\
src/auth.py:42: error: Incompatible return value type (got \"str\", expected \"int\")
src/auth.py:55: error: Name \"foo\" is not defined
Found 2 errors in 1 file (checked 5 source files)
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 2);
        assert_eq!(result.failed, 2);
        assert_eq!(
            result.failures[0].location.as_deref(),
            Some("src/auth.py:42")
        );
        assert!(result.summary.contains("2 error"));
    }

    #[test]
    fn parse_mypy_success() {
        let output = "Success: no issues found in 5 source files\n";
        let result = parse(output);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("no issues"));
    }

    #[test]
    fn parse_mypy_ignores_notes() {
        let output = "\
src/auth.py:42: error: Incompatible types
src/auth.py:42: note: Expected: int
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 0);
    }
}
