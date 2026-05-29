use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    JEST_SUMMARY_RE,
    r"Tests:\s+(?:(\d+) failed, )?(\d+) passed, (\d+) total"
);
define_regex!(
    VITEST_SUMMARY_RE,
    r"Tests\s+(?:(\d+) failed \| )?(\d+) passed \((\d+)\)"
);
define_regex!(FAILURE_RE, r"^\s*●\s+(.+)$");
define_regex!(AT_RE, r"at .+ \((.+):(\d+):(\d+)\)");

pub fn parse(output: &str) -> ParsedResult {
    let mut passed = 0;
    let mut failed = 0;
    let mut summary = String::new();
    let mut failures = Vec::new();

    for line in output.lines() {
        if let Some(caps) = JEST_SUMMARY_RE.captures(line) {
            failed = caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            passed = caps[2].parse().unwrap_or(0);
            summary = format!("{passed} passed, {failed} failed");
        } else if let Some(caps) = VITEST_SUMMARY_RE.captures(line) {
            failed = caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            passed = caps[2].parse().unwrap_or(0);
            summary = format!("{passed} passed, {failed} failed");
        }
    }

    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(caps) = FAILURE_RE.captures(lines[i]) {
            let name = caps[1].trim().to_string();
            let mut message_lines = Vec::new();
            let mut location = None;

            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with('●') {
                let trimmed = lines[i].trim();
                if let Some(at_caps) = AT_RE.captures(trimmed) {
                    if location.is_none() {
                        location = Some(format!("{}:{}:{}", &at_caps[1], &at_caps[2], &at_caps[3]));
                    }
                } else if !trimmed.is_empty() {
                    message_lines.push(trimmed.to_string());
                }
                i += 1;
            }

            failures.push(Diagnostic {
                name,
                location,
                message: message_lines.join("\n"),
            });
            continue;
        }
        i += 1;
    }

    if summary.is_empty() {
        summary = format!("{passed} passed, {failed} failed");
    }

    ParsedResult::new(summary, passed, failed, 0, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_jest_all_pass() {
        let output = "Tests:  50 passed, 50 total\nTime:   3.45 s\n";
        let result = parse(output);
        assert_eq!(result.passed, 50);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn parse_jest_with_failures() {
        let output = "\
  ● Auth > should authenticate user

    expect(received).toBe(expected)

    Expected: true
    Received: false

      at Object.<anonymous> (tests/auth.test.ts:43:18)

Tests:  1 failed, 49 passed, 50 total
";
        let result = parse(output);
        assert_eq!(result.passed, 49);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].name.contains("authenticate"));
        assert!(
            result.failures[0]
                .location
                .as_ref()
                .unwrap()
                .contains("auth.test.ts:43")
        );
    }

    #[test]
    fn parse_vitest_summary() {
        let output = " Tests  2 failed | 48 passed (50)\n";
        let result = parse(output);
        assert_eq!(result.passed, 48);
        assert_eq!(result.failed, 2);
    }
}
