use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static PACKAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(ok|FAIL)\s+(\S+)\s+([\d.]+)s").unwrap());

static FAIL_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^--- FAIL: (\S+)").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut passed_pkgs = 0;
    let mut failed_pkgs = 0;
    let mut failures = Vec::new();

    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(caps) = PACKAGE_RE.captures(lines[i]) {
            if &caps[1] == "ok" {
                passed_pkgs += 1;
            } else {
                failed_pkgs += 1;
            }
        }

        if let Some(caps) = FAIL_HEADER_RE.captures(lines[i]) {
            let test_name = caps[1].to_string();
            let mut message_lines = Vec::new();

            i += 1;
            while i < lines.len() && !lines[i].starts_with("---") && !PACKAGE_RE.is_match(lines[i])
            {
                let trimmed = lines[i].trim();
                if !trimmed.is_empty() {
                    message_lines.push(trimmed.to_string());
                }
                i += 1;
            }

            let location = message_lines.first().and_then(|l| {
                if l.contains(".go:") {
                    Some(l.split(':').take(2).collect::<Vec<_>>().join(":"))
                } else {
                    None
                }
            });

            failures.push(Diagnostic {
                name: test_name,
                location,
                message: message_lines.join("\n"),
            });
            continue;
        }
        i += 1;
    }

    let summary = format!("{passed_pkgs} ok, {failed_pkgs} failed");

    ParsedResult {
        summary,
        passed: passed_pkgs,
        failed: failed_pkgs,
        skipped: 0,
        failures,
        warnings: vec![],
        tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_passing() {
        let output = "ok      github.com/user/project/auth     0.003s\nok      github.com/user/project/handler  0.005s\n";
        let result = parse(output);
        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn parse_with_failure() {
        let output = "\
--- FAIL: TestLogin (0.00s)
    auth_test.go:42: expected true, got false
FAIL    github.com/user/project/auth    0.005s
ok      github.com/user/project/handler 0.003s
";
        let result = parse(output);
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "TestLogin");
    }
}
