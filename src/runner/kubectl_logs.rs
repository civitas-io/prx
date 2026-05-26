use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static LEVEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ERROR|WARN(?:ING)?|FATAL|DEBUG|INFO|TRACE)\b").unwrap());

fn classify(line: &str) -> Option<&'static str> {
    let caps = LEVEL_RE.captures(line)?;
    match caps[1].to_ascii_uppercase().as_str() {
        "ERROR" => Some("ERROR"),
        "FATAL" => Some("FATAL"),
        "WARN" | "WARNING" => Some("WARN"),
        _ => None,
    }
}

pub fn parse(output: &str) -> ParsedResult {
    let lines: Vec<&str> = output.lines().collect();
    let mut failures: Vec<Diagnostic> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();

    let mut i = 0;
    let mut first_interesting = true;
    while i < lines.len() {
        let line = lines[i];
        let Some(level) = classify(line) else {
            i += 1;
            continue;
        };

        if first_interesting && i > 0 {
            let prev = lines[i - 1];
            if !prev.trim().is_empty() && classify(prev).is_none() {
                emitted.push(format!("context: {prev}"));
            }
            first_interesting = false;
        }

        let mut count = 1;
        while i + count < lines.len() && lines[i + count] == line {
            count += 1;
        }

        let display = if count > 1 {
            format!("{line} [repeated {count} times]")
        } else {
            line.to_string()
        };
        emitted.push(display.clone());

        let diag = Diagnostic {
            name: format!("log/{level}"),
            location: None,
            message: display,
        };
        if level == "WARN" {
            warnings.push(diag);
        } else {
            failures.push(diag);
        }

        i += count;
    }

    let summary = format!(
        "{} error(s)/fatal(s), {} warning(s)",
        failures.len(),
        warnings.len()
    );

    let tail = if emitted.is_empty() {
        None
    } else {
        Some(emitted.join("\n"))
    };

    ParsedResult {
        summary,
        passed: 0,
        failed: failures.len(),
        skipped: 0,
        failures,
        warnings,
        tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dedupe_and_context() {
        let output = "\
2026-05-26 10:00:00 INFO Starting server on port 8080
2026-05-26 10:00:01 INFO Connected to database
2026-05-26 10:00:02 ERROR Connection refused: redis:6379
2026-05-26 10:00:02 ERROR Connection refused: redis:6379
2026-05-26 10:00:02 ERROR Connection refused: redis:6379
2026-05-26 10:00:05 WARN Retrying in 5 seconds
2026-05-26 10:00:10 INFO Reconnected to redis
";
        let result = parse(output);
        let tail = result.tail.expect("tail");
        assert!(tail.contains("context: 2026-05-26 10:00:01 INFO Connected to database"));
        assert!(tail.contains("Connection refused: redis:6379 [repeated 3 times]"));
        assert!(tail.contains("Retrying in 5 seconds"));
        assert!(!tail.contains("Reconnected to redis"));
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn parse_no_problems() {
        let output = "\
2026-05-26 10:00:00 INFO Starting
2026-05-26 10:00:01 INFO Connected
2026-05-26 10:00:02 DEBUG initialization done
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 0);
        assert_eq!(result.warnings.len(), 0);
        assert!(result.tail.is_none());
    }

    #[test]
    fn parse_fatal_treated_as_failure() {
        let output = "FATAL out of memory\n";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].name.contains("FATAL"));
    }
}
