use serde_json;

use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(
    PLAN_RE,
    r"^Plan: (\d+) to add, (\d+) to change, (\d+) to destroy\."
);
define_regex!(
    APPLY_RE,
    r"^Apply complete!\s+Resources:\s+(\d+) added,\s+(\d+) changed,\s+(\d+) destroyed"
);
define_regex!(RESOURCE_RE, r"^\s*#\s+(\S+)\s+will be (\w+)");
define_regex!(NO_CHANGES_RE, r"^No changes\.");
define_regex!(ERROR_RE, r"^(?i)error:?\s*(.+)$");

fn parse_json(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let warnings = Vec::new();
    let mut summary = String::new();
    let mut resources: Vec<String> = Vec::new();

    // terraform plan -json outputs line-delimited JSON
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Extract change_summary for plan summary
            if let Some(msg) = json.get("@message").and_then(|m| m.as_str()) {
                if let Some(json_type) = json.get("type").and_then(|t| t.as_str()) {
                    if json_type == "change_summary" {
                        summary = msg.to_string();
                    }
                }
            }

            // Extract planned_change resources
            if let Some(json_type) = json.get("type").and_then(|t| t.as_str()) {
                if json_type == "planned_change" {
                    if let Some(resource) = json.get("resource").and_then(|r| r.as_str()) {
                        if let Some(change) = json.get("change").and_then(|c| c.get("actions")) {
                            if let Some(actions) = change.as_array() {
                                for action in actions {
                                    if let Some(action_str) = action.as_str() {
                                        resources.push(format!("{} ({})", resource, action_str));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Extract errors
            if let Some(level) = json.get("@level").and_then(|l| l.as_str()) {
                if level == "error" {
                    if let Some(msg) = json.get("@message").and_then(|m| m.as_str()) {
                        failures.push(Diagnostic {
                            name: "terraform/error".to_string(),
                            location: None,
                            message: msg.to_string(),
                        });
                    }
                }
            }
        }
    }

    if summary.is_empty() {
        summary = if !failures.is_empty() {
            format!("{} error(s)", failures.len())
        } else {
            "terraform complete".to_string()
        };
    }

    let tail = if resources.is_empty() {
        None
    } else {
        Some(resources.join("\n"))
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
    if trimmed.starts_with('{') {
        // Likely JSON output, try parsing as line-delimited JSON
        if trimmed
            .lines()
            .next()
            .is_some_and(|l| serde_json::from_str::<serde_json::Value>(l.trim()).is_ok())
        {
            return parse_json(output);
        }
    }

    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut summary = String::new();
    let mut resources: Vec<String> = Vec::new();
    let mut no_changes = false;

    for line in output.lines() {
        if let Some(caps) = PLAN_RE.captures(line) {
            summary = format!(
                "plan: {} add, {} change, {} destroy",
                &caps[1], &caps[2], &caps[3]
            );
            continue;
        }
        if let Some(caps) = APPLY_RE.captures(line) {
            summary = format!(
                "apply: {} added, {} changed, {} destroyed",
                &caps[1], &caps[2], &caps[3]
            );
            continue;
        }
        if NO_CHANGES_RE.is_match(line) {
            no_changes = true;
            continue;
        }
        if let Some(caps) = RESOURCE_RE.captures(line) {
            resources.push(format!("{} ({})", &caps[1], &caps[2]));
            continue;
        }
        if let Some(caps) = ERROR_RE.captures(line) {
            failures.push(Diagnostic {
                name: "terraform/error".to_string(),
                location: None,
                message: caps[1].to_string(),
            });
        } else if line.starts_with("Warning:") {
            warnings.push(Diagnostic {
                name: "terraform/warning".to_string(),
                location: None,
                message: line.trim_start_matches("Warning:").trim().to_string(),
            });
        }
    }

    if summary.is_empty() {
        summary = if no_changes {
            "no changes".to_string()
        } else if !failures.is_empty() {
            format!("{} error(s)", failures.len())
        } else {
            "terraform complete".to_string()
        };
    }

    let tail = if resources.is_empty() {
        None
    } else {
        Some(resources.join("\n"))
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
    fn parse_terraform_plan() {
        let output = "\
Terraform will perform the following actions:

  # aws_instance.web will be created
  + resource \"aws_instance\" \"web\" {
      + ami           = \"ami-12345\"
      + instance_type = \"t2.micro\"
    }

  # aws_s3_bucket.data will be destroyed
  - resource \"aws_s3_bucket\" \"data\" {
    }

Plan: 1 to add, 0 to change, 1 to destroy.
";
        let result = parse(output);
        assert!(result.summary.contains("1 add"));
        assert!(result.summary.contains("1 destroy"));
        let tail = result.tail.expect("tail");
        assert!(tail.contains("aws_instance.web (created)"));
        assert!(tail.contains("aws_s3_bucket.data (destroyed)"));
    }

    #[test]
    fn parse_terraform_apply() {
        let output = "Apply complete! Resources: 2 added, 0 changed, 0 destroyed.\n";
        let result = parse(output);
        assert!(result.summary.contains("2 added"));
    }

    #[test]
    fn parse_terraform_no_changes() {
        let output = "No changes. Your infrastructure matches the configuration.\n";
        let result = parse(output);
        assert!(result.summary.contains("no changes"));
    }

    #[test]
    fn parse_terraform_json_output() {
        let json_output = r#"{"type":"planned_change","resource":"aws_instance.web","change":{"actions":["create"]}}
{"type":"planned_change","resource":"aws_s3_bucket.data","change":{"actions":["delete"]}}
{"type":"change_summary","@level":"info","@message":"Plan: 1 to add, 0 to change, 1 to destroy"}
"#;
        let result = parse(json_output);
        assert!(result.summary.contains("Plan: 1 to add"));
        let tail = result.tail.expect("tail");
        assert!(tail.contains("aws_instance.web (create)"));
        assert!(tail.contains("aws_s3_bucket.data (delete)"));
    }
}
