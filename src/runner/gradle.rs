use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(FAILED_TASK_RE, r"^>\s+Task\s+(\S+)\s+FAILED");
define_regex!(JAVA_ERROR_RE, r"^(.+?):(\d+):\s+error:\s+(.+)$");
define_regex!(BUILD_RE, r"^BUILD (SUCCESSFUL|FAILED)");
define_regex!(TEST_FAIL_RE, r"^(\d+) tests? completed, (\d+) failed");

fn is_noise(line: &str) -> bool {
    line.starts_with("Download ")
        || line.starts_with("Starting a Gradle Daemon")
        || line.starts_with("Daemon will be stopped")
        || line.starts_with("Welcome to Gradle")
}

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut build_result: Option<&str> = None;
    let mut failed_tasks: Vec<String> = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for line in output.lines() {
        if is_noise(line) {
            continue;
        }

        if let Some(caps) = FAILED_TASK_RE.captures(line) {
            failed_tasks.push(caps[1].to_string());
            failures.push(Diagnostic {
                name: "gradle/task_failed".to_string(),
                location: None,
                message: format!("task {} FAILED", &caps[1]),
            });
            continue;
        }

        if let Some(caps) = JAVA_ERROR_RE.captures(line) {
            failures.push(Diagnostic {
                name: "gradle/compile_error".to_string(),
                location: Some(format!("{}:{}", &caps[1], &caps[2])),
                message: caps[3].to_string(),
            });
            continue;
        }

        if let Some(caps) = TEST_FAIL_RE.captures(line) {
            let total: usize = caps[1].parse().unwrap_or(0);
            failed = caps[2].parse().unwrap_or(0);
            passed = total.saturating_sub(failed);
            continue;
        }

        if let Some(caps) = BUILD_RE.captures(line) {
            build_result = Some(if &caps[1] == "SUCCESSFUL" {
                "SUCCESSFUL"
            } else {
                "FAILED"
            });
        }
    }

    let summary = match build_result {
        Some("SUCCESSFUL") => "BUILD SUCCESSFUL".to_string(),
        Some("FAILED") => {
            if failed_tasks.is_empty() {
                format!("BUILD FAILED, {} error(s)", failures.len())
            } else {
                format!("BUILD FAILED, tasks: {}", failed_tasks.join(", "))
            }
        }
        _ if failures.is_empty() => "complete".to_string(),
        _ => format!("{} failure(s)", failures.len()),
    };

    ParsedResult::new(
        summary,
        passed,
        if failed > 0 { failed } else { failures.len() },
        0,
        failures,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gradle_build_failure() {
        let output = "\
> Task :compileJava
> Task :compileJava FAILED

/src/main/java/App.java:42: error: ';' expected
    int x = 5
            ^
1 error

BUILD FAILED in 3s
2 actionable tasks: 1 executed, 1 up-to-date
";
        let result = parse(output);
        assert!(result.summary.contains("FAILED"));
        assert!(result.summary.contains(":compileJava"));
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.location.as_deref() == Some("/src/main/java/App.java:42"))
        );
    }

    #[test]
    fn parse_gradle_build_success() {
        let output = "\
BUILD SUCCESSFUL in 5s
7 actionable tasks: 7 executed
";
        let result = parse(output);
        assert!(result.summary.contains("SUCCESSFUL"));
        assert_eq!(result.failures.len(), 0);
    }

    #[test]
    fn parse_gradle_test_failure_summary() {
        let output = "\
10 tests completed, 2 failed
BUILD FAILED in 4s
";
        let result = parse(output);
        assert_eq!(result.failed, 2);
        assert_eq!(result.passed, 8);
    }
}
