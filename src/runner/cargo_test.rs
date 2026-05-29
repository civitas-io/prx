use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    SUMMARY_RE,
    r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored"
);
define_regex!(FAILURE_HEADER_RE, r"^---- (.+) stdout ----$");
define_regex!(PANIC_RE, r"panicked at (.+):(\d+)");

pub fn parse(output: &str) -> ParsedResult {
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut summary = String::new();
    let mut failures = Vec::new();

    if let Some(caps) = SUMMARY_RE.captures(output) {
        passed = caps[2].parse().unwrap_or(0);
        failed = caps[3].parse().unwrap_or(0);
        skipped = caps[4].parse().unwrap_or(0);
        summary = format!("{passed} passed, {failed} failed, {skipped} ignored");
    }

    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(caps) = FAILURE_HEADER_RE.captures(lines[i]) {
            let test_name = caps[1].to_string();
            let mut location = None;
            let mut message_lines = Vec::new();

            i += 1;
            while i < lines.len()
                && !lines[i].starts_with("----")
                && !lines[i].starts_with("failures:")
            {
                if let Some(panic_caps) = PANIC_RE.captures(lines[i]) {
                    location = Some(format!("{}:{}", &panic_caps[1], &panic_caps[2]));
                } else if !lines[i].starts_with("note:") && !lines[i].starts_with("thread '") {
                    let trimmed = lines[i].trim();
                    if !trimmed.is_empty() {
                        message_lines.push(trimmed.to_string());
                    }
                }
                i += 1;
            }

            failures.push(Diagnostic {
                name: test_name,
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

    ParsedResult::new(summary, passed, failed, skipped, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_passing() {
        let output = "\
running 3 tests
test hash::tests::deterministic ... ok
test hash::tests::empty_input ... ok
test hash::tests::hex_length ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
        let result = parse(output);
        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 0);
        assert!(result.failures.is_empty());
        assert!(result.summary.contains("3 passed"));
    }

    #[test]
    fn parse_with_failure() {
        let output = "\
running 2 tests
test hash::tests::deterministic ... ok
test hash::tests::bad_test ... FAILED

failures:

---- hash::tests::bad_test stdout ----
thread 'hash::tests::bad_test' panicked at src/hash.rs:42:9:
assertion `left == right` failed
  left: 1
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    hash::tests::bad_test

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
        let result = parse(output);
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "hash::tests::bad_test");
        assert!(
            result.failures[0]
                .location
                .as_ref()
                .unwrap()
                .contains("src/hash.rs:42")
        );
        assert!(result.failures[0].message.contains("left == right"));
    }

    #[test]
    fn parse_with_ignored() {
        let output = "test result: ok. 5 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        let result = parse(output);
        assert_eq!(result.passed, 5);
        assert_eq!(result.skipped, 2);
    }
}
