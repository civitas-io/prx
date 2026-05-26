use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static ERROR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[ERROR\]\s+(.+)$").unwrap());

static LOCATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?):\[(\d+),(\d+)\]\s+(.+)$").unwrap());

static TESTS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Tests run: (\d+), Failures: (\d+), Errors: (\d+), Skipped: (\d+)").unwrap()
});

static BUILD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[INFO\] BUILD (SUCCESS|FAILURE)").unwrap());

fn is_noise(line: &str) -> bool {
    line.starts_with("[INFO] Downloading from")
        || line.starts_with("[INFO] Downloaded from")
        || line.starts_with("[INFO] Progress")
}

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut build_result: Option<&str> = None;

    for line in output.lines() {
        if is_noise(line) {
            continue;
        }

        if let Some(caps) = ERROR_RE.captures(line) {
            let msg = caps[1].to_string();
            let (location, message) = if let Some(loc_caps) = LOCATION_RE.captures(&msg) {
                (
                    Some(format!(
                        "{}:{}:{}",
                        &loc_caps[1], &loc_caps[2], &loc_caps[3]
                    )),
                    loc_caps[4].to_string(),
                )
            } else {
                (None, msg)
            };
            failures.push(Diagnostic {
                name: "mvn/error".to_string(),
                location,
                message,
            });
            continue;
        }

        if let Some(caps) = TESTS_RE.captures(line) {
            let total: usize = caps[1].parse().unwrap_or(0);
            let f: usize = caps[2].parse().unwrap_or(0);
            let e: usize = caps[3].parse().unwrap_or(0);
            let s: usize = caps[4].parse().unwrap_or(0);
            failed = f + e;
            skipped = s;
            passed = total.saturating_sub(failed + skipped);
            continue;
        }

        if let Some(caps) = BUILD_RE.captures(line) {
            build_result = Some(if &caps[1] == "SUCCESS" {
                "SUCCESS"
            } else {
                "FAILURE"
            });
        }
    }

    let summary = match build_result {
        Some("SUCCESS") => {
            if passed + failed + skipped > 0 {
                format!("BUILD SUCCESS, {passed} passed, {failed} failed, {skipped} skipped")
            } else {
                "BUILD SUCCESS".to_string()
            }
        }
        Some("FAILURE") => format!("BUILD FAILURE, {} error(s)", failures.len()),
        _ if failures.is_empty() => "complete".to_string(),
        _ => format!("{} error(s)", failures.len()),
    };

    ParsedResult {
        summary,
        passed,
        failed: if failed > 0 { failed } else { failures.len() },
        skipped,
        failures,
        warnings: vec![],
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mvn_compile_error() {
        let output = "\
[INFO] Scanning for projects...
[INFO] Downloading from central: https://repo.maven.apache.org/foo
[INFO] Downloaded from central: https://repo.maven.apache.org/foo
[ERROR] /src/main/java/App.java:[42,10] error: ';' expected
[INFO] BUILD FAILURE
[INFO] Total time: 5.123 s
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(
            result.failures[0].location.as_deref(),
            Some("/src/main/java/App.java:42:10")
        );
        assert!(result.summary.contains("FAILURE"));
    }

    #[test]
    fn parse_mvn_test_success() {
        let output = "\
[INFO] Tests run: 10, Failures: 0, Errors: 0, Skipped: 0
[INFO] BUILD SUCCESS
";
        let result = parse(output);
        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("SUCCESS"));
    }

    #[test]
    fn parse_mvn_test_failure() {
        let output = "\
[INFO] Tests run: 10, Failures: 1, Errors: 0, Skipped: 2
[INFO] BUILD FAILURE
";
        let result = parse(output);
        assert_eq!(result.passed, 7);
        assert_eq!(result.failed, 1);
        assert_eq!(result.skipped, 2);
    }
}
