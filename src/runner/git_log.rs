use super::{ParsedResult, define_regex};

define_regex!(COMMIT_RE, r"^commit ([0-9a-f]{7,40})");
define_regex!(AUTHOR_RE, r"^Author:\s+(.+?)\s+<");

pub fn parse(output: &str) -> ParsedResult {
    let mut commits: Vec<String> = Vec::new();
    let mut current_hash: Option<String> = None;
    let mut current_author: Option<String> = None;
    let mut current_subject: Option<String> = None;

    let flush = |hash: &mut Option<String>,
                 author: &mut Option<String>,
                 subject: &mut Option<String>,
                 commits: &mut Vec<String>| {
        if let (Some(h), Some(s)) = (hash.as_ref(), subject.as_ref()) {
            let a = author.as_deref().unwrap_or("?");
            commits.push(format!("{h} [{a}] {s}"));
        }
        *hash = None;
        *author = None;
        *subject = None;
    };

    for line in output.lines() {
        if let Some(caps) = COMMIT_RE.captures(line) {
            flush(
                &mut current_hash,
                &mut current_author,
                &mut current_subject,
                &mut commits,
            );
            let full = &caps[1];
            current_hash = Some(full[..full.len().min(7)].to_string());
            continue;
        }

        if let Some(caps) = AUTHOR_RE.captures(line) {
            current_author = Some(caps[1].to_string());
            continue;
        }

        if current_hash.is_some() && current_subject.is_none() {
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && !line.starts_with("Date:")
                && !line.starts_with("Merge:")
                && !line.starts_with("Author:")
            {
                current_subject = Some(trimmed.to_string());
            }
        }
    }

    flush(
        &mut current_hash,
        &mut current_author,
        &mut current_subject,
        &mut commits,
    );

    let summary = format!("{} commit(s)", commits.len());
    let tail = if commits.is_empty() {
        None
    } else {
        Some(commits.join("\n"))
    };

    ParsedResult {
        summary,
        passed: 0,
        failed: 0,
        skipped: 0,
        failures: vec![],
        warnings: vec![],
        tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_log_multiple_commits() {
        let output = "\
commit abc1234567890def
Author: Alice <alice@example.com>
Date:   Mon May 26 10:00:00 2026 +0000

    Fix authentication bug

commit def4567890123abc
Author: Bob <bob@example.com>
Date:   Sun May 25 09:00:00 2026 +0000

    Add login endpoint
";
        let result = parse(output);
        let tail = result.tail.expect("tail present");
        assert_eq!(tail.lines().count(), 2);
        assert!(tail.contains("abc1234"));
        assert!(tail.contains("[Alice]"));
        assert!(tail.contains("Fix authentication bug"));
        assert!(tail.contains("def4567"));
        assert!(tail.contains("[Bob]"));
        assert!(result.summary.contains("2 commit"));
    }

    #[test]
    fn parse_git_log_empty() {
        let result = parse("");
        assert!(result.tail.is_none());
        assert!(result.summary.contains("0 commit"));
    }

    #[test]
    fn parse_git_log_truncates_long_hash() {
        let output = "\
commit 0123456789abcdef0123456789abcdef01234567
Author: Carol <carol@example.com>
Date:   Mon May 26 10:00:00 2026 +0000

    A subject
";
        let result = parse(output);
        let tail = result.tail.expect("tail");
        assert!(tail.starts_with("0123456 "));
    }
}
