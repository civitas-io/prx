use std::path::Path;

use clap::Args;
use serde::Serialize;
use similar::ChangeTag;

use crate::output::{AgError, to_json};
use crate::parsing::outline;

#[derive(Args)]
pub struct DiffArgs {
    /// File path (optional, default: all changed files)
    pub file: Option<String>,

    /// Compare against git ref
    #[arg(long, default_value = "HEAD")]
    pub since: String,

    /// Compare staged changes
    #[arg(long)]
    pub staged: bool,

    /// Summary and stats only
    #[arg(long)]
    pub stat_only: bool,

    /// Token budget for hunks
    #[arg(long)]
    pub budget: Option<usize>,

    /// Group hunks by function
    #[arg(long)]
    pub functions: bool,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct DiffOutput {
    pub summary: String,
    pub stats: DiffStats,
    pub semantic_notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunks: Option<Vec<Hunk>>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct DiffStats {
    pub additions: usize,
    pub deletions: usize,
    pub files_changed: usize,
    pub functions_changed: Vec<String>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct Hunk {
    pub file: String,
    pub function: Option<String>,
    pub changes: Vec<DiffChange>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct DiffChange {
    #[serde(rename = "type")]
    pub change_type: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

pub fn run(args: DiffArgs) -> Result<serde_json::Value, AgError> {
    let changed_files = get_git_diff(&args)?;

    if changed_files.is_empty() {
        let output = DiffOutput {
            summary: "no changes".to_string(),
            stats: DiffStats {
                additions: 0,
                deletions: 0,
                files_changed: 0,
                functions_changed: vec![],
            },
            semantic_notes: vec![],
            hunks: None,
        };
        return to_json(output);
    }

    let mut total_additions = 0;
    let mut total_deletions = 0;
    let mut all_hunks = Vec::new();
    let mut all_functions_changed = Vec::new();
    let mut semantic_notes = Vec::new();

    for file_diff in &changed_files {
        let diff = similar::TextDiff::from_lines(&file_diff.old_content, &file_diff.new_content);
        let mut file_additions = 0;
        let mut file_deletions = 0;
        let mut file_changes = Vec::new();

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => {
                    file_additions += 1;
                    file_changes.push(DiffChange {
                        change_type: "addition".to_string(),
                        old: None,
                        new: Some(change.to_string().trim_end().to_string()),
                    });
                }
                ChangeTag::Delete => {
                    file_deletions += 1;
                    file_changes.push(DiffChange {
                        change_type: "deletion".to_string(),
                        old: Some(change.to_string().trim_end().to_string()),
                        new: None,
                    });
                }
                ChangeTag::Equal => {}
            }
        }

        total_additions += file_additions;
        total_deletions += file_deletions;

        let ext = Path::new(&file_diff.path)
            .extension()
            .and_then(|e| e.to_str());

        let functions_in_diff = if let Some(ext_str) = ext {
            find_changed_functions(&file_diff.old_content, &file_diff.new_content, ext_str)
        } else {
            vec![]
        };

        all_functions_changed.extend(
            functions_in_diff
                .iter()
                .map(|f| format!("{}:{}", file_diff.path, f)),
        );

        detect_semantic_changes(
            &file_diff.old_content,
            &file_diff.new_content,
            ext,
            &file_diff.path,
            &mut semantic_notes,
        );

        if !file_changes.is_empty() {
            all_hunks.push(Hunk {
                file: file_diff.path.clone(),
                function: functions_in_diff.first().cloned(),
                changes: file_changes,
            });
        }
    }

    let summary = build_summary(
        changed_files.len(),
        total_additions,
        total_deletions,
        &all_functions_changed,
    );

    let hunks = if args.stat_only {
        None
    } else {
        let mut h = all_hunks;
        if let Some(budget) = args.budget {
            let mut used = 0;
            h.retain(|hunk| {
                let cost = hunk
                    .changes
                    .iter()
                    .map(|c| {
                        c.old.as_ref().map_or(0, |s| s.len())
                            + c.new.as_ref().map_or(0, |s| s.len())
                    })
                    .sum::<usize>()
                    / 4;
                if used + cost <= budget {
                    used += cost;
                    true
                } else {
                    false
                }
            });
        }
        Some(h)
    };

    let output = DiffOutput {
        summary,
        stats: DiffStats {
            additions: total_additions,
            deletions: total_deletions,
            files_changed: changed_files.len(),
            functions_changed: all_functions_changed,
        },
        semantic_notes,
        hunks,
    };

    to_json(output)
}

struct FileDiff {
    path: String,
    old_content: String,
    new_content: String,
}

fn get_git_diff(args: &DiffArgs) -> Result<Vec<FileDiff>, AgError> {
    let diff_args = if args.staged {
        vec!["diff", "--staged", "--name-only"]
    } else {
        vec!["diff", &args.since, "--name-only"]
    };

    let output = std::process::Command::new("git")
        .args(&diff_args)
        .output()
        .map_err(|e| AgError::GitError {
            message: format!("failed to run git: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AgError::GitError {
            message: format!("git diff failed: {stderr}"),
        });
    }

    let names = String::from_utf8_lossy(&output.stdout);
    let mut diffs = Vec::new();

    for name in names.lines().filter(|l| !l.is_empty()) {
        if let Some(ref file_filter) = args.file {
            if !name.contains(file_filter) {
                continue;
            }
        }

        let old_content = get_git_file_content(name, &args.since).unwrap_or_default();
        let new_content = std::fs::read_to_string(name).unwrap_or_default();

        diffs.push(FileDiff {
            path: name.to_string(),
            old_content,
            new_content,
        });
    }

    Ok(diffs)
}

fn get_git_file_content(path: &str, git_ref: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{git_ref}:{path}")])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

fn find_changed_functions(old: &str, new: &str, ext: &str) -> Vec<String> {
    let symbols = outline::extract_symbols(new, ext);
    let diff = similar::TextDiff::from_lines(old, new);
    let mut changed_lines: Vec<usize> = Vec::new();

    for change in diff.iter_all_changes() {
        if change.tag() == ChangeTag::Insert {
            if let Some(idx) = change.new_index() {
                changed_lines.push(idx + 1);
            }
        }
    }

    let mut functions = Vec::new();
    for line in &changed_lines {
        for sym in &symbols {
            if *line >= sym.start_line && *line <= sym.end_line && !functions.contains(&sym.name) {
                functions.push(sym.name.clone());
            }
        }
    }

    functions
}

fn detect_semantic_changes(
    old: &str,
    new: &str,
    ext: Option<&str>,
    path: &str,
    notes: &mut Vec<String>,
) {
    let ext_str = match ext {
        Some(e) => e,
        None => return,
    };

    let old_symbols = outline::extract_symbols(old, ext_str);
    let new_symbols = outline::extract_symbols(new, ext_str);

    let old_names: Vec<&str> = old_symbols.iter().map(|s| s.name.as_str()).collect();
    let new_names: Vec<&str> = new_symbols.iter().map(|s| s.name.as_str()).collect();

    for name in &new_names {
        if !old_names.contains(name) {
            notes.push(format!("{path}: new symbol `{name}`"));
        }
    }

    for name in &old_names {
        if !new_names.contains(name) {
            notes.push(format!("{path}: removed symbol `{name}`"));
        }
    }

    for old_sym in &old_symbols {
        if let Some(new_sym) = new_symbols.iter().find(|s| s.name == old_sym.name) {
            if old_sym.signature != new_sym.signature {
                notes.push(format!("{path}: signature changed `{}`", old_sym.name));
            }
        }
    }
}

fn build_summary(files: usize, additions: usize, deletions: usize, functions: &[String]) -> String {
    let func_part = if functions.is_empty() {
        String::new()
    } else if functions.len() <= 3 {
        format!(". Functions: {}", functions.join(", "))
    } else {
        format!(". {} functions changed", functions.len())
    };

    format!(
        "{} file(s) changed, +{} -{}{func_part}",
        files, additions, deletions
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_summary_basic() {
        let s = build_summary(2, 10, 3, &[]);
        assert_eq!(s, "2 file(s) changed, +10 -3");
    }

    #[test]
    fn build_summary_with_functions() {
        let funcs = vec!["auth.rs:login".to_string()];
        let s = build_summary(1, 5, 2, &funcs);
        assert!(s.contains("login"));
    }

    #[test]
    fn detect_new_symbol() {
        let old = "fn hello() {}\n";
        let new = "fn hello() {}\nfn world() {}\n";
        let mut notes = Vec::new();
        detect_semantic_changes(old, new, Some("rs"), "test.rs", &mut notes);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("new symbol") && n.contains("world")),
            "should detect new symbol: {notes:?}"
        );
    }

    #[test]
    fn detect_removed_symbol() {
        let old = "fn hello() {}\nfn world() {}\n";
        let new = "fn hello() {}\n";
        let mut notes = Vec::new();
        detect_semantic_changes(old, new, Some("rs"), "test.rs", &mut notes);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("removed symbol") && n.contains("world")),
            "should detect removed symbol: {notes:?}"
        );
    }

    #[test]
    fn detect_signature_change() {
        let old = "fn hello(x: i32) {}\n";
        let new = "fn hello(x: i32, y: i32) {}\n";
        let mut notes = Vec::new();
        detect_semantic_changes(old, new, Some("rs"), "test.rs", &mut notes);
        assert!(
            notes.iter().any(|n| n.contains("signature changed")),
            "should detect signature change: {notes:?}"
        );
    }

    #[test]
    fn no_changes_detected() {
        let content = "fn hello() {}\n";
        let mut notes = Vec::new();
        detect_semantic_changes(content, content, Some("rs"), "test.rs", &mut notes);
        assert!(notes.is_empty());
    }

    #[test]
    fn diff_change_serializes() {
        let change = DiffChange {
            change_type: "addition".to_string(),
            old: None,
            new: Some("let x = 1;".to_string()),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("\"type\":\"addition\""));
    }

    #[test]
    fn stat_only_has_no_hunks() {
        let output = DiffOutput {
            summary: "test".to_string(),
            stats: DiffStats {
                additions: 1,
                deletions: 0,
                files_changed: 1,
                functions_changed: vec![],
            },
            semantic_notes: vec![],
            hunks: None,
        };
        let json = serde_json::to_value(&output).unwrap();
        assert!(json.get("hunks").is_none());
    }
}
