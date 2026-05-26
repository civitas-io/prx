use regex::Regex;
use std::sync::LazyLock;

use super::{Diagnostic, ParsedResult};

static STATUS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Status:\s+(\S+)").unwrap());

static CONDITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s+(Initialized|Ready|PodScheduled|ContainersReady|Available|Progressing)\s+(\S+)",
    )
    .unwrap()
});

static EVENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+(Normal|Warning)\s+(\S+)\s+\S+\s+\S+\s+(.+?)\s*$").unwrap());

static RESTART_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)restart\s*count:\s*(\d+)").unwrap());

pub fn parse(output: &str) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut status: Option<String> = None;
    let mut in_events = false;

    for line in output.lines() {
        if let Some(caps) = STATUS_RE.captures(line) {
            status = Some(caps[1].to_string());
            let s = caps[1].to_ascii_lowercase();
            if matches!(
                s.as_str(),
                "failed" | "crashloopbackoff" | "error" | "pending"
            ) {
                failures.push(Diagnostic {
                    name: "kubectl/phase".to_string(),
                    location: None,
                    message: format!("status: {}", &caps[1]),
                });
            }
            continue;
        }

        if line.trim_start().starts_with("Events:") {
            in_events = true;
            continue;
        }

        if !in_events {
            if let Some(caps) = CONDITION_RE.captures(line) {
                if &caps[2] != "True" {
                    failures.push(Diagnostic {
                        name: format!("kubectl/condition/{}", &caps[1]),
                        location: None,
                        message: format!("{}={}", &caps[1], &caps[2]),
                    });
                }
                continue;
            }
        }

        if let Some(caps) = EVENT_RE.captures(line) {
            if &caps[1] == "Warning" {
                warnings.push(Diagnostic {
                    name: format!("kubectl/event/{}", &caps[2]),
                    location: None,
                    message: caps[3].trim().to_string(),
                });
            }
            continue;
        }

        if let Some(caps) = RESTART_RE.captures(line) {
            if let Ok(n) = caps[1].parse::<u32>() {
                if n > 0 {
                    warnings.push(Diagnostic {
                        name: "kubectl/restarts".to_string(),
                        location: None,
                        message: format!("restart count: {n}"),
                    });
                }
            }
        }
    }

    let summary = match status {
        Some(s) => format!(
            "status: {}, {} problem(s), {} warning(s)",
            s,
            failures.len(),
            warnings.len()
        ),
        None => format!(
            "{} problem(s), {} warning(s)",
            failures.len(),
            warnings.len()
        ),
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
    fn parse_kubectl_describe_unhealthy() {
        let output = "\
Name:         myapp-xyz-abc
Namespace:    default
Status:       Running
Conditions:
  Type           Status
  Initialized    True
  Ready          False
  PodScheduled   True
Events:
  Type     Reason     Age   From     Message
  Normal   Scheduled  5m    default  Successfully assigned
  Normal   Pulled     5m    kubelet  Container image pulled
  Warning  BackOff    2m    kubelet  Back-off restarting failed container
  Warning  Unhealthy  1m    kubelet  Readiness probe failed
";
        let result = parse(output);
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.message.contains("Ready=False"))
        );
        assert_eq!(result.warnings.len(), 2);
        assert!(result.warnings.iter().any(|w| w.name.contains("BackOff")));
    }

    #[test]
    fn parse_kubectl_healthy_drops_normal_events() {
        let output = "\
Status:       Running
Conditions:
  Type           Status
  Initialized    True
  Ready          True
Events:
  Type     Reason     Age   From     Message
  Normal   Scheduled  5m    default  Successfully assigned
";
        let result = parse(output);
        assert_eq!(result.failures.len(), 0);
        assert_eq!(result.warnings.len(), 0);
    }

    #[test]
    fn parse_kubectl_restart_count() {
        let output = "\
Status:       Running
    Restart Count: 7
";
        let result = parse(output);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.message.contains("restart count: 7"))
        );
    }
}
