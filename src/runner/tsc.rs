use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    ERROR_RE,
    r"^(.+)\((\d+),(\d+)\): (error|warning) (TS\d+): (.+)$"
);

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    for line in output.lines() {
        if let Some(caps) = ERROR_RE.captures(line) {
            let diag = Diagnostic {
                name: caps[5].to_string(),
                location: Some(format!("{}:{}:{}", &caps[1], &caps[2], &caps[3])),
                message: caps[6].to_string(),
            };
            if &caps[4] == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
        }
    }

    let summary = if failures.is_empty() && warnings.is_empty() {
        "no errors".to_string()
    } else {
        ParsedResult::diagnostic_summary(failures.len(), warnings.len())
    };

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
    fn parse_tsc_error() {
        let output = "src/auth.ts(42,18): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.\n";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "TS2345");
        assert!(
            result.failures[0]
                .location
                .as_ref()
                .unwrap()
                .contains("src/auth.ts:42:18")
        );
    }

    #[test]
    fn parse_tsc_clean() {
        let output = "";
        let result = parse(output);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("no errors"));
    }
}
