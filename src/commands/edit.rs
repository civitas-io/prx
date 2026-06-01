use std::path::Path;

use clap::Args;
use regex::Regex;
use serde::Serialize;

use crate::hash;
use crate::output::{AgError, to_json};
use crate::parsing;

#[derive(Args)]
pub struct EditArgs {
    /// File path
    pub file: String,

    /// Text to find
    #[arg(long)]
    pub find: Vec<String>,

    /// Replacement text
    #[arg(long)]
    pub replace: Vec<String>,

    /// Interpret --find as regex
    #[arg(long)]
    pub regex: bool,

    /// Apply changes to file (default: dry-run)
    #[arg(long)]
    pub apply: bool,

    /// Scope replacement to named function
    #[arg(long)]
    pub in_function: Option<String>,

    /// Scope replacement to named class
    #[arg(long)]
    pub in_class: Option<String>,

    /// Replace all occurrences
    #[arg(long)]
    pub all: bool,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct EditOutput {
    pub file: String,
    pub dry_run: bool,
    pub changes: Vec<Change>,
    pub total_replacements: usize,
    pub syntax_valid: bool,
    pub syntax_error: Option<String>,
    pub hash_before: String,
    pub hash_after: String,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct Change {
    pub line: usize,
    pub function: Option<String>,
    pub before: String,
    pub after: String,
}

pub fn run(args: EditArgs) -> Result<serde_json::Value, AgError> {
    let path = Path::new(&args.file);
    if !path.exists() {
        return Err(AgError::FileNotFound {
            path: args.file.clone(),
        });
    }

    if args.find.is_empty() {
        return Err(AgError::InvalidArgument {
            flag: "find".to_string(),
            message: "at least one --find value required".to_string(),
        });
    }

    if args.find.len() != args.replace.len() {
        return Err(AgError::InvalidArgument {
            flag: "replace".to_string(),
            message: format!(
                "--find and --replace must have same count ({} vs {})",
                args.find.len(),
                args.replace.len()
            ),
        });
    }

    let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
    let hash_before = hash::hash_bytes(content.as_bytes());
    let ext = parsing::extension_from_path(path);

    let scope = resolve_scope(&content, ext, &args.in_function, &args.in_class);

    let mut modified = content.clone();
    let mut all_changes = Vec::new();

    for (find_pat, replace_with) in args.find.iter().zip(args.replace.iter()) {
        let changes = apply_replacements(
            &mut modified,
            find_pat,
            replace_with,
            args.regex,
            args.all,
            &scope,
        )?;
        all_changes.extend(changes);
    }

    let total_replacements = all_changes.len();
    let hash_after = hash::hash_bytes(modified.as_bytes());

    let (syntax_valid, syntax_error) = if total_replacements > 0 {
        check_syntax(&modified, ext)
    } else {
        (true, None)
    };

    if args.apply && total_replacements > 0 && syntax_valid {
        std::fs::write(path, &modified).map_err(AgError::Io)?;
    }

    let output = EditOutput {
        file: args.file,
        dry_run: !args.apply,
        changes: all_changes,
        total_replacements,
        syntax_valid,
        syntax_error,
        hash_before,
        hash_after,
    };

    to_json(output)
}

struct Scope {
    start_line: usize,
    end_line: usize,
    name: Option<String>,
}

fn resolve_scope(
    content: &str,
    ext: Option<&str>,
    in_function: &Option<String>,
    in_class: &Option<String>,
) -> Option<Scope> {
    let name = if let Some(name) = in_function {
        name
    } else if let Some(name) = in_class {
        name
    } else {
        return None;
    };

    let ext_str = ext?;
    let symbols = parsing::outline::extract_symbols(content, ext_str);
    let sym = find_symbol_by_name(&symbols, name)?;

    Some(Scope {
        start_line: sym.start_line,
        end_line: sym.end_line,
        name: Some(sym.name.clone()),
    })
}

fn find_symbol_by_name<'a>(
    symbols: &'a [parsing::outline::Symbol],
    name: &str,
) -> Option<&'a parsing::outline::Symbol> {
    for sym in symbols {
        if sym.name == name {
            return Some(sym);
        }
        if let Some(child) = find_symbol_by_name(&sym.children, name) {
            return Some(child);
        }
    }
    None
}

fn apply_replacements(
    content: &mut String,
    find: &str,
    replace: &str,
    use_regex: bool,
    replace_all: bool,
    scope: &Option<Scope>,
) -> Result<Vec<Change>, AgError> {
    let lines: Vec<String> = content.lines().map(String::from).collect();
    let mut changes = Vec::new();
    let mut new_lines = lines.clone();

    let re = if use_regex {
        Some(Regex::new(find).map_err(|e| AgError::InvalidArgument {
            flag: "find".to_string(),
            message: format!("invalid regex: {e}"),
        })?)
    } else {
        None
    };

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;

        if let Some(s) = scope {
            if line_num < s.start_line || line_num > s.end_line {
                continue;
            }
        }

        let (matched, new_line) = if let Some(ref re) = re {
            if re.is_match(line) {
                let replaced = if replace_all {
                    re.replace_all(line, replace).to_string()
                } else {
                    re.replace(line, replace).to_string()
                };
                (true, replaced)
            } else {
                (false, line.clone())
            }
        } else if line.contains(find) {
            let replaced = if replace_all {
                line.replace(find, replace)
            } else {
                line.replacen(find, replace, 1)
            };
            (true, replaced)
        } else {
            (false, line.clone())
        };

        if matched {
            changes.push(Change {
                line: line_num,
                function: scope.as_ref().and_then(|s| s.name.clone()),
                before: line.clone(),
                after: new_line.clone(),
            });
            new_lines[idx] = new_line;

            if !replace_all && changes.len() == 1 && !use_regex {
                break;
            }
        }
    }

    let had_trailing_newline = content.ends_with('\n');
    *content = new_lines.join("\n");
    if had_trailing_newline && !content.ends_with('\n') {
        content.push('\n');
    }

    Ok(changes)
}

fn check_syntax(content: &str, ext: Option<&str>) -> (bool, Option<String>) {
    let ext_str = match ext {
        Some(e) => e,
        None => return (true, None),
    };

    let lang = match parsing::languages::language_for_extension(ext_str) {
        Some(l) => l,
        None => return (true, None),
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return (true, None);
    }

    match parser.parse(content, None) {
        Some(tree) => {
            if tree.root_node().has_error() {
                (false, Some("syntax error detected after edit".to_string()))
            } else {
                (true, None)
            }
        }
        None => (true, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_file(content: &str) -> (TempDir, String) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.rs");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_string_lossy().to_string())
    }

    fn edit_args(file: &str, find: &str, replace: &str) -> EditArgs {
        EditArgs {
            file: file.to_string(),
            find: vec![find.to_string()],
            replace: vec![replace.to_string()],
            regex: false,
            apply: false,
            in_function: None,
            in_class: None,
            all: false,
        }
    }

    #[test]
    fn dry_run_does_not_modify_file() {
        let content = "fn hello() {\n    let x = 1;\n}\n";
        let (_dir, path) = make_test_file(content);
        let args = edit_args(&path, "let x = 1", "let x = 2");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert!(out.dry_run);
        assert_eq!(out.total_replacements, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn apply_modifies_file() {
        let (_dir, path) = make_test_file("fn hello() {\n    let x = 1;\n}\n");
        let mut args = edit_args(&path, "let x = 1", "let x = 42");
        args.apply = true;
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert!(!out.dry_run);
        assert_eq!(out.total_replacements, 1);
        let modified = std::fs::read_to_string(&path).unwrap();
        assert!(modified.contains("let x = 42"));
        assert!(!modified.contains("let x = 1"));
    }

    #[test]
    fn shows_before_and_after() {
        let (_dir, path) = make_test_file("fn hello() {\n    let x = 1;\n}\n");
        let args = edit_args(&path, "let x = 1", "let x = 2");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.changes[0].before, "    let x = 1;");
        assert_eq!(out.changes[0].after, "    let x = 2;");
        assert_eq!(out.changes[0].line, 2);
    }

    #[test]
    fn replaces_first_only_by_default() {
        let (_dir, path) = make_test_file("let a = 1;\nlet b = 1;\nlet c = 1;\n");
        let args = edit_args(&path, "= 1", "= 99");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.total_replacements, 1);
    }

    #[test]
    fn replace_all_flag() {
        let (_dir, path) = make_test_file("let a = 1;\nlet b = 1;\nlet c = 1;\n");
        let mut args = edit_args(&path, "= 1", "= 99");
        args.all = true;
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.total_replacements, 3);
    }

    #[test]
    fn regex_mode() {
        let (_dir, path) = make_test_file("let x = 123;\nlet y = 456;\n");
        let mut args = edit_args(&path, r"\d+", "0");
        args.regex = true;
        args.all = true;
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert!(out.total_replacements >= 2);
    }

    #[test]
    fn in_function_scoping() {
        let content = "fn foo() {\n    let x = 1;\n}\n\nfn bar() {\n    let x = 1;\n}\n";
        let (_dir, path) = make_test_file(content);
        let mut args = edit_args(&path, "let x = 1", "let x = 99");
        args.in_function = Some("foo".to_string());
        args.all = true;
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.total_replacements, 1);
        assert_eq!(out.changes[0].function.as_deref(), Some("foo"));
    }

    #[test]
    fn syntax_validation_detects_errors() {
        let (_dir, path) = make_test_file("fn hello() {\n    let x = 1;\n}\n");
        let args = edit_args(&path, "let x = 1;", "let x = ;");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert!(!out.syntax_valid);
        assert!(out.syntax_error.is_some());
    }

    #[test]
    fn syntax_valid_on_good_edit() {
        let (_dir, path) = make_test_file("fn hello() {\n    let x = 1;\n}\n");
        let args = edit_args(&path, "let x = 1", "let x = 2");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert!(out.syntax_valid);
    }

    #[test]
    fn hashes_differ_on_change() {
        let (_dir, path) = make_test_file("fn hello() {\n    let x = 1;\n}\n");
        let args = edit_args(&path, "let x = 1", "let x = 2");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_ne!(out.hash_before, out.hash_after);
    }

    #[test]
    fn no_match_returns_zero_changes() {
        let (_dir, path) = make_test_file("fn hello() {}\n");
        let args = edit_args(&path, "nonexistent", "replacement");
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.total_replacements, 0);
        assert_eq!(out.hash_before, out.hash_after);
    }

    #[test]
    fn nonexistent_file_errors() {
        let args = edit_args("/nonexistent/file.rs", "a", "b");
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::FileNotFound { .. }
        ));
    }

    #[test]
    fn mismatched_find_replace_count_errors() {
        let (_dir, path) = make_test_file("hello\n");
        let args = EditArgs {
            file: path,
            find: vec!["a".to_string(), "b".to_string()],
            replace: vec!["c".to_string()],
            regex: false,
            apply: false,
            in_function: None,
            in_class: None,
            all: false,
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::InvalidArgument { .. }
        ));
    }

    #[test]
    fn multi_edit_batching() {
        let (_dir, path) = make_test_file("let a = 1;\nlet b = 2;\n");
        let args = EditArgs {
            file: path,
            find: vec!["a = 1".to_string(), "b = 2".to_string()],
            replace: vec!["a = 10".to_string(), "b = 20".to_string()],
            regex: false,
            apply: false,
            in_function: None,
            in_class: None,
            all: false,
        };
        let result = run(args).unwrap();
        let out: EditOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.total_replacements, 2);
    }

    #[test]
    fn invalid_regex_errors() {
        let (_dir, path) = make_test_file("hello\n");
        let mut args = edit_args(&path, "[invalid(", "x");
        args.regex = true;
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::InvalidArgument { .. }
        ));
    }
}
