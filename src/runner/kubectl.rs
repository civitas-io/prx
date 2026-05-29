use serde_json;

use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(STATUS_RE, r"^Status:\s+(\S+)");
define_regex!(
    CONDITION_RE,
    r"^\s+(Initialized|Ready|PodScheduled|ContainersReady|Available|Progressing)\s+(\S+)"
);
define_regex!(
    EVENT_RE,
    r"^\s+(Normal|Warning)\s+(\S+)\s+\S+\s+\S+\s+(.+?)\s*$"
);
define_regex!(RESTART_RE, r"(?i)restart\s*count:\s*(\d+)");

fn parse_json(json: &serde_json::Value) -> ParsedResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut status: Option<String> = None;

    // Handle kubectl get -o json format: {"items": [...]}
    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
        for item in items {
            // Extract status phase
            if let Some(phase) = item
                .get("status")
                .and_then(|s| s.get("phase"))
                .and_then(|p| p.as_str())
            {
                status = Some(phase.to_string());
                if matches!(
                    phase.to_ascii_lowercase().as_str(),
                    "failed" | "crashloopbackoff" | "error" | "pending"
                ) {
                    failures.push(Diagnostic {
                        name: "kubectl/phase".to_string(),
                        location: None,
                        message: format!("status: {}", phase),
                    });
                }
            }

            // Extract conditions
            if let Some(conditions) = item
                .get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array())
            {
                for cond in conditions {
                    if let (Some(cond_type), Some(cond_status)) = (
                        cond.get("type").and_then(|t| t.as_str()),
                        cond.get("status").and_then(|s| s.as_str()),
                    ) {
                        if cond_status != "True" {
                            failures.push(Diagnostic {
                                name: format!("kubectl/condition/{}", cond_type),
                                location: None,
                                message: format!("{}={}", cond_type, cond_status),
                            });
                        }
                    }
                }
            }

            // Extract warning events
            if let Some(events) = item
                .get("status")
                .and_then(|s| s.get("events"))
                .and_then(|e| e.as_array())
            {
                for event in events {
                    if let (Some(event_type), Some(reason), Some(message)) = (
                        event.get("type").and_then(|t| t.as_str()),
                        event.get("reason").and_then(|r| r.as_str()),
                        event.get("message").and_then(|m| m.as_str()),
                    ) {
                        if event_type == "Warning" {
                            warnings.push(Diagnostic {
                                name: format!("kubectl/event/{}", reason),
                                location: None,
                                message: message.to_string(),
                            });
                        }
                    }
                }
            }

            // Extract restart count
            if let Some(containers) = item
                .get("status")
                .and_then(|s| s.get("containerStatuses"))
                .and_then(|c| c.as_array())
            {
                for container in containers {
                    if let Some(restart_count) =
                        container.get("restartCount").and_then(|r| r.as_u64())
                    {
                        if restart_count > 0 {
                            warnings.push(Diagnostic {
                                name: "kubectl/restarts".to_string(),
                                location: None,
                                message: format!("restart count: {}", restart_count),
                            });
                        }
                    }
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

pub fn parse(output: &str) -> ParsedResult {
    let trimmed = output.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return parse_json(&json);
        }
    }

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

    #[test]
    fn parse_kubectl_json_output() {
        let json_output = r#"{
  "items": [
    {
      "metadata": {"name": "myapp-xyz"},
      "status": {
        "phase": "Running",
        "conditions": [
          {"type": "Ready", "status": "False"},
          {"type": "Initialized", "status": "True"}
        ],
        "containerStatuses": [
          {"restartCount": 3}
        ]
      }
    }
  ]
}"#;
        let result = parse(json_output);
        assert!(
            result
                .failures
                .iter()
                .any(|f| f.message.contains("Ready=False"))
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.message.contains("restart count: 3"))
        );
        assert!(result.summary.contains("Running"));
    }
}
