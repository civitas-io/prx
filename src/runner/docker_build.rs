use super::{Diagnostic, ParsedResult, define_regex};

define_regex!(STEP_RE, r"^Step (\d+/\d+) : (.+)$");
define_regex!(SUCCESS_BUILT_RE, r"^Successfully built (\S+)");
define_regex!(SUCCESS_TAGGED_RE, r"^Successfully tagged (\S+)");
define_regex!(ERROR_RE, r"^(ERROR|error):\s*(.+)$");
define_regex!(NONZERO_RE, r"returned a non-zero code:\s*(\d+)");

fn is_noise(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.starts_with("Sending build context")
        || trimmed.starts_with("---> ")
        || trimmed == "--->"
        || line.contains(": Pulling from")
        || line.contains(": Already exists")
        || line.contains(": Pull complete")
        || line.contains(": Downloading")
        || line.contains(": Extracting")
        || line.contains(": Waiting")
        || line.contains(": Verifying Checksum")
        || line.contains(": Download complete")
}

pub fn parse(output: &str) -> ParsedResult {
    let lines: Vec<&str> = output.lines().collect();
    let mut failures = Vec::new();
    let mut last_step: Option<String> = None;
    let mut built_image: Option<String> = None;
    let mut tagged_image: Option<String> = None;
    let mut had_error = false;

    for line in &lines {
        if let Some(caps) = STEP_RE.captures(line) {
            last_step = Some(format!("Step {}: {}", &caps[1], &caps[2]));
        }
        if let Some(caps) = SUCCESS_BUILT_RE.captures(line) {
            built_image = Some(caps[1].to_string());
        }
        if let Some(caps) = SUCCESS_TAGGED_RE.captures(line) {
            tagged_image = Some(caps[1].to_string());
        }
        if let Some(caps) = ERROR_RE.captures(line) {
            had_error = true;
            failures.push(Diagnostic {
                name: "docker/error".to_string(),
                location: last_step.clone(),
                message: caps[2].to_string(),
            });
        }
        if let Some(caps) = NONZERO_RE.captures(line) {
            had_error = true;
            failures.push(Diagnostic {
                name: format!("docker/exit_{}", &caps[1]),
                location: last_step.clone(),
                message: line.trim().to_string(),
            });
        }
    }

    let (summary, tail) = if had_error {
        let filtered: Vec<&&str> = lines.iter().filter(|l| !is_noise(l)).collect();
        let start = filtered.len().saturating_sub(20);
        let tail_text: String = filtered[start..]
            .iter()
            .map(|s| **s)
            .collect::<Vec<&str>>()
            .join("\n");
        let summary = match &last_step {
            Some(s) => format!("build FAILED at {s}"),
            None => "build FAILED".to_string(),
        };
        (summary, Some(tail_text))
    } else if built_image.is_some() || tagged_image.is_some() {
        let parts: Vec<String> = [
            built_image.as_ref().map(|i| format!("built {i}")),
            tagged_image.as_ref().map(|i| format!("tagged {i}")),
        ]
        .into_iter()
        .flatten()
        .collect();
        (format!("build succeeded ({})", parts.join(", ")), None)
    } else {
        ("build complete".to_string(), None)
    };

    ParsedResult {
        summary,
        passed: 0,
        failed: failures.len(),
        skipped: 0,
        failures,
        warnings: vec![],
        tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_docker_build_failure() {
        let output = "\
Sending build context to Docker daemon  2.048kB
Step 1/5 : FROM python:3.11
 ---> abc123
Step 2/5 : COPY requirements.txt .
 ---> Using cache
Step 3/5 : RUN pip install -r requirements.txt
 ---> Running in def456
ERROR: Could not find a version that satisfies the requirement nonexistent-pkg
The command '/bin/sh -c pip install -r requirements.txt' returned a non-zero code: 1
";
        let result = parse(output);
        assert!(result.failed >= 1);
        assert!(result.summary.contains("FAILED"));
        let tail = result.tail.expect("tail on failure");
        assert!(!tail.contains("Sending build context"));
        assert!(tail.contains("ERROR"));
    }

    #[test]
    fn parse_docker_build_success() {
        let output = "\
Successfully built abc123def456
Successfully tagged myapp:latest
";
        let result = parse(output);
        assert_eq!(result.failed, 0);
        assert!(result.summary.contains("succeeded"));
        assert!(result.summary.contains("myapp:latest"));
        assert!(result.tail.is_none());
    }

    #[test]
    fn parse_docker_build_strips_noise() {
        let output = "\
Sending build context to Docker daemon  10kB
Step 1/2 : FROM alpine
 ---> aaa
Step 2/2 : RUN false
ERROR: build failed
";
        let result = parse(output);
        let tail = result.tail.unwrap();
        assert!(!tail.contains("Sending build context"));
        assert!(!tail.contains(" ---> aaa"));
    }
}
