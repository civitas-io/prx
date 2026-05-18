use super::ParsedResult;

pub fn parse(output: &str, exit_code: i32) -> ParsedResult {
    let tail_lines = if exit_code == 0 { 10 } else { 20 };
    let lines: Vec<&str> = output.lines().collect();
    let start = lines.len().saturating_sub(tail_lines);
    let tail = lines[start..].join("\n");

    ParsedResult {
        summary: format!("exited {exit_code}"),
        passed: 0,
        failed: if exit_code != 0 { 1 } else { 0 },
        skipped: 0,
        failures: vec![],
        warnings: vec![],
        tail: Some(tail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_success() {
        let result = parse("hello\nworld\n", 0);
        assert_eq!(result.summary, "exited 0");
        assert_eq!(result.failed, 0);
        assert!(result.tail.unwrap().contains("hello"));
    }

    #[test]
    fn fallback_failure() {
        let result = parse("error: something broke\n", 1);
        assert_eq!(result.summary, "exited 1");
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn fallback_truncates_long_output() {
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let output = lines.join("\n");
        let result = parse(&output, 0);
        let tail = result.tail.unwrap();
        let tail_count = tail.lines().count();
        assert!(tail_count <= 10);
    }

    #[test]
    fn failure_gets_more_tail() {
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let output = lines.join("\n");
        let result = parse(&output, 1);
        let tail = result.tail.unwrap();
        let tail_count = tail.lines().count();
        assert!(tail_count <= 20);
        assert!(tail_count > 10);
    }
}
