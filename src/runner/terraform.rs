use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static PLAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^Plan: (\d+) to add, (\d+) to change, (\d+) to destroy\.").unwrap()
});

static APPLY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^Apply complete!\s+Resources:\s+(\d+) added,\s+(\d+) changed,\s+(\d+) destroyed")
        .unwrap()
});

static RESOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*#\s+(\S+)\s+will be (\w+)").unwrap());

static NO_CHANGES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^No changes\.").unwrap());

static ERROR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?i)error:?\s*(.+)$").unwrap());

pub fn parse(output: &str) -> ParsedResult {
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
}
