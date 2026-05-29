use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    ISSUE_RE,
    r"^(.+?)\((\d+),(\d+)\): (error|warning) (CS\d+): (.+)$"
);
define_regex!(BUILD_RE, r"^Build (succeeded|FAILED)\.");
define_regex!(
    TEST_RE,
    r"(?:Failed|Passed)!\s+-\s+Failed:\s+(\d+),\s+Passed:\s+(\d+),\s+Skipped:\s+(\d+)"
);

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut build_result: Option<&str> = None;
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for line in output.lines() {
        if let Some(caps) = ISSUE_RE.captures(line) {
            let severity = &caps[4];
            let diag = Diagnostic {
                name: format!("dotnet/{}", &caps[5]),
                location: Some(format!("{}:{}:{}", &caps[1], &caps[2], &caps[3])),
                message: caps[6].to_string(),
            };
            if severity == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
            continue;
        }

        if let Some(caps) = BUILD_RE.captures(line) {
            build_result = Some(if &caps[1] == "succeeded" {
                "succeeded"
            } else {
                "FAILED"
            });
        }

        if let Some(caps) = TEST_RE.captures(line) {
            failed = caps[1].parse().unwrap_or(0);
            passed = caps[2].parse().unwrap_or(0);
            skipped = caps[3].parse().unwrap_or(0);
        }
    }

    let summary = match build_result {
        Some("succeeded") => format!("build succeeded, {} warning(s)", warnings.len()),
        Some("FAILED") => format!("build FAILED, {} error(s)", failures.len()),
        _ if passed + failed + skipped > 0 => {
            format!("{passed} passed, {failed} failed, {skipped} skipped")
        }
        _ => format!("{} error(s), {} warning(s)", failures.len(), warnings.len()),
    };

    ParsedResult {
        summary,
        passed,
        failed: if failed > 0 { failed } else { failures.len() },
        skipped,
        failures,
        warnings,
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dotnet_build_failure() {
        let output = "\
Program.cs(42,10): error CS1002: ; expected
Program.cs(15,5): warning CS0168: The variable 'x' is declared but never used
Build FAILED.
    1 Error(s)
    1 Warning(s)
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.failures[0].location.as_deref(),
            Some("Program.cs:42:10")
        );
        assert!(result.summary.contains("FAILED"));
    }

    #[test]
    fn parse_dotnet_build_success() {
        let output = "\
Build succeeded.
    0 Error(s)
    0 Warning(s)
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 0);
        assert!(result.summary.contains("succeeded"));
    }

    #[test]
    fn parse_dotnet_test_summary() {
        let output = "Passed!  - Failed:     0, Passed:    10, Skipped:     0, Total:    10\n";
        let result = parse(output);
        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 0);
    }
}
