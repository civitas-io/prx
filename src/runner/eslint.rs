use serde_json;

use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    ISSUE_RE,
    r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s{2,}(\S+)\s*$"
);
define_regex!(
    SUMMARY_RE,
    r"(\d+) problems? \((\d+) errors?, (\d+) warnings?\)"
);

fn parse_json(json: &serde_json::Value) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    // eslint --format json outputs an array of file results
    if let Some(files) = json.as_array() {
        for file_result in files {
            if let Some(file_path) = file_result.get("filePath").and_then(|f| f.as_str()) {
                if let Some(messages) = file_result.get("messages").and_then(|m| m.as_array()) {
                    for msg in messages {
                        if let (
                            Some(severity),
                            Some(message),
                            Some(line),
                            Some(column),
                            Some(rule_id),
                        ) = (
                            msg.get("severity").and_then(|s| s.as_u64()),
                            msg.get("message").and_then(|m| m.as_str()),
                            msg.get("line").and_then(|l| l.as_u64()),
                            msg.get("column").and_then(|c| c.as_u64()),
                            msg.get("ruleId").and_then(|r| r.as_str()),
                        ) {
                            let location = format!("{}:{}:{}", file_path, line, column);
                            let diag = Diagnostic {
                                name: rule_id.to_string(),
                                location: Some(location),
                                message: message.to_string(),
                            };

                            // severity 2 = error, 1 = warning
                            if severity == 2 {
                                failures.push(diag);
                            } else {
                                warnings.push(diag);
                            }
                        }
                    }
                }
            }
        }
    }

    let summary = if failures.is_empty() && warnings.is_empty() {
        "no issues".to_string()
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

pub fn parse(output: &str) -> ParsedResult {
    if let Some(json) = super::try_parse_json(output) {
        return parse_json(&json);
    }

    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut current_file = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !trimmed.starts_with(char::is_whitespace)
            && !trimmed.starts_with('✖')
            && !trimmed.starts_with('×')
            && !ISSUE_RE.is_match(line)
            && !SUMMARY_RE.is_match(line)
        {
            current_file = trimmed.to_string();
        }

        if let Some(caps) = ISSUE_RE.captures(line) {
            let location = if current_file.is_empty() {
                format!("{}:{}", &caps[1], &caps[2])
            } else {
                format!("{}:{}:{}", current_file, &caps[1], &caps[2])
            };

            let diag = Diagnostic {
                name: caps[5].to_string(),
                location: Some(location),
                message: caps[4].to_string(),
            };

            if &caps[3] == "error" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
        }
    }

    let summary = if failures.is_empty() && warnings.is_empty() {
        "no issues".to_string()
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
    fn parse_eslint_errors() {
        let output = "\
/Users/user/project/src/auth.ts
  42:18  error  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any
  55:1   warning  Missing return type                      @typescript-eslint/explicit-function-return-type

✖ 2 problems (1 error, 1 warning)
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.failures[0].name,
            "@typescript-eslint/no-explicit-any"
        );
    }

    #[test]
    fn parse_eslint_clean() {
        let result = parse("");
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("no issues"));
    }

    #[test]
    fn parse_eslint_json_output() {
        let json_output = r#"[
  {
    "filePath": "/Users/user/project/src/auth.ts",
    "messages": [
      {
        "severity": 2,
        "message": "Unexpected any. Specify a different type",
        "line": 42,
        "column": 18,
        "ruleId": "@typescript-eslint/no-explicit-any"
      },
      {
        "severity": 1,
        "message": "Missing return type",
        "line": 55,
        "column": 1,
        "ruleId": "@typescript-eslint/explicit-function-return-type"
      }
    ]
  }
]
"#;
        let result = parse(json_output);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.failures[0].name,
            "@typescript-eslint/no-explicit-any"
        );
        assert!(
            result.failures[0]
                .location
                .as_ref()
                .unwrap()
                .contains("42:18")
        );
    }
}
