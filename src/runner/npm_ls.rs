use regex::Regex;
use serde_json;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static TOP_LEVEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:├──|└──|├─┬|└─┬)\s+(.+)$").unwrap());

static NPM_ERR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^npm (ERR!|WARN)\s+(.+)$").unwrap());

fn parse_json(json: &serde_json::Value) -> ParsedResult {
    let mut deps: Vec<String> = Vec::new();
    let mut failures: Vec<Diagnostic> = Vec::new();
    let warnings: Vec<Diagnostic> = Vec::new();

    // Extract top-level dependencies
    if let Some(dependencies) = json.get("dependencies").and_then(|d| d.as_object()) {
        for (name, dep_info) in dependencies {
            if let Some(version) = dep_info.get("version").and_then(|v| v.as_str()) {
                let entry = format!("{}@{}", name, version);
                let mut has_problems = false;

                // Check for problems array
                if let Some(problems) = dep_info.get("problems").and_then(|p| p.as_array()) {
                    for problem in problems {
                        if let Some(problem_str) = problem.as_str() {
                            failures.push(Diagnostic {
                                name: "npm/dep_problem".to_string(),
                                location: None,
                                message: format!("{}: {}", name, problem_str),
                            });
                            has_problems = true;
                        }
                    }
                }

                // Check for invalid/missing flags
                if dep_info
                    .get("invalid")
                    .and_then(|i| i.as_bool())
                    .unwrap_or(false)
                {
                    failures.push(Diagnostic {
                        name: "npm/dep_problem".to_string(),
                        location: None,
                        message: format!("{}: invalid", name),
                    });
                    has_problems = true;
                } else if dep_info
                    .get("missing")
                    .and_then(|m| m.as_bool())
                    .unwrap_or(false)
                {
                    failures.push(Diagnostic {
                        name: "npm/dep_problem".to_string(),
                        location: None,
                        message: format!("{}: missing", name),
                    });
                    has_problems = true;
                }

                if !has_problems {
                    deps.push(entry);
                }
            }
        }
    }

    let summary = format!(
        "{} top-level dep(s), {} problem(s), {} warning(s)",
        deps.len(),
        failures.len(),
        warnings.len()
    );

    let tail = if deps.is_empty() {
        None
    } else {
        Some(deps.join("\n"))
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

pub fn parse(output: &str) -> ParsedResult {
    let trimmed = output.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return parse_json(&json);
        }
    }

    let mut deps: Vec<String> = Vec::new();
    let mut failures: Vec<Diagnostic> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();

    for line in output.lines() {
        if let Some(caps) = TOP_LEVEL_RE.captures(line) {
            let entry = caps[1].trim().to_string();
            if entry.contains("UNMET") || entry.contains("INVALID") {
                failures.push(Diagnostic {
                    name: "npm/dep_problem".to_string(),
                    location: None,
                    message: entry,
                });
            } else {
                deps.push(entry);
            }
            continue;
        }

        if let Some(caps) = NPM_ERR_RE.captures(line) {
            let kind = &caps[1];
            let diag = Diagnostic {
                name: format!("npm/{kind}"),
                location: None,
                message: caps[2].to_string(),
            };
            if kind == "ERR!" {
                failures.push(diag);
            } else {
                warnings.push(diag);
            }
        }
    }

    let summary = format!(
        "{} top-level dep(s), {} problem(s), {} warning(s)",
        deps.len(),
        failures.len(),
        warnings.len()
    );

    let tail = if deps.is_empty() {
        None
    } else {
        Some(deps.join("\n"))
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
    fn parse_npm_ls_mixed() {
        let output = "\
myapp@1.0.0 /path/to/app
├── express@4.18.2
├── UNMET PEER DEPENDENCY react@17.0.0
├─┬ typescript@5.0.0
│ └── nested-dep@1.0.0
└── lodash@4.17.21
npm ERR! peer dep missing: react@^18.0.0, required by react-dom@18.2.0
";
        let result = parse(output);
        let tail = result.tail.expect("tail");
        assert!(tail.contains("express@4.18.2"));
        assert!(tail.contains("typescript@5.0.0"));
        assert!(tail.contains("lodash@4.17.21"));
        assert!(!tail.contains("nested-dep"));
        assert!(result.failures.iter().any(|f| f.message.contains("UNMET")));
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.message.contains("peer dep missing"))
        );
    }

    #[test]
    fn parse_npm_ls_clean() {
        let output = "\
myapp@1.0.0 /path/to/app
├── express@4.18.2
└── lodash@4.17.21
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 0);
        let tail = result.tail.expect("tail");
        assert_eq!(tail.lines().count(), 2);
    }

    #[test]
    fn parse_npm_ls_warnings() {
        let output = "npm WARN deprecated foo@1.0.0: use bar instead\n";
        let result = parse(output);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn parse_npm_ls_json_output() {
        let json_output = r#"{
  "name": "myapp",
  "version": "1.0.0",
  "dependencies": {
    "express": {
      "version": "4.18.2"
    },
    "react": {
      "version": "17.0.0",
      "problems": ["peer dependency mismatch"]
    },
    "missing-pkg": {
      "version": "1.0.0",
      "missing": true
    }
  }
}
"#;
        let result = parse(json_output);
        let tail = result.tail.expect("tail");
        assert!(tail.contains("express@4.18.2"));
        assert!(!tail.contains("react@17.0.0"));
        assert!(!tail.contains("missing-pkg"));
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.message.contains("peer dependency mismatch"))
        );
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.message.contains("missing"))
        );
    }
}
