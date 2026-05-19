use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::hash;
use crate::output::AgError;
use crate::parsing::{self, outline, snap, strip};
use crate::tokens;

#[derive(Args)]
pub struct ReadArgs {
    /// File path
    pub file: String,

    /// Line range (e.g., 10-20)
    #[arg(long)]
    pub lines: Option<String>,

    /// Expand range to enclosing structure
    #[arg(long)]
    pub snap: Option<String>,

    /// Return signatures and exports only
    #[arg(long)]
    pub skeleton: bool,

    /// Return symbol table
    #[arg(long)]
    pub outline: bool,

    /// Return content hash only
    #[arg(long)]
    pub hash: bool,

    /// Token budget for content
    #[arg(long)]
    pub budget: Option<usize>,

    /// Include file metadata
    #[arg(long)]
    pub meta: bool,

    /// Return cached stub if file hash matches (skip content/outline)
    #[arg(long)]
    pub if_changed: Option<String>,

    /// Read mode: aggressive (strip comments), entropy (filter repetitive lines)
    #[arg(long)]
    pub mode: Option<String>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct ReadOutput {
    pub file: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cached: bool,
    pub meta: FileMeta,
    pub content: Option<ContentBlock>,
    pub outline: Option<Vec<SymbolEntry>>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct FileMeta {
    pub language: Option<String>,
    pub lines: usize,
    pub bytes: usize,
    pub modified: Option<u64>,
    pub hash: String,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct ContentBlock {
    pub range: (usize, usize),
    pub snap: Option<String>,
    pub snap_reason: Option<String>,
    pub text: String,
    pub tokens: usize,
    pub truncated: bool,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: String,
    pub lines: (usize, usize),
    pub signature: String,
    pub children: Vec<SymbolEntry>,
}

fn validate_hash(input: &str) -> Result<String, AgError> {
    let normalized = input.to_ascii_lowercase();
    if normalized.len() != 32 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AgError::InvalidArgument {
            flag: "if-changed".to_string(),
            message: format!("expected 32-char hex hash, got `{input}`"),
        });
    }
    Ok(normalized)
}

pub fn run(args: ReadArgs) -> Result<serde_json::Value, AgError> {
    let path = Path::new(&args.file);
    if !path.exists() {
        return Err(AgError::FileNotFound {
            path: args.file.clone(),
        });
    }

    let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
    let file_hash = hash::hash_bytes(content.as_bytes());
    let line_count = content.lines().count();
    let byte_count = content.len();
    let ext = parsing::extension_from_path(path);
    let language = ext
        .and_then(parsing::languages::language_name_for_extension)
        .map(String::from);

    let modified = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    let meta = FileMeta {
        language: language.clone(),
        lines: line_count,
        bytes: byte_count,
        modified,
        hash: file_hash.clone(),
    };

    if let Some(ref prev_hash) = args.if_changed {
        let normalized = validate_hash(prev_hash)?;
        if normalized == file_hash {
            let output = ReadOutput {
                file: args.file,
                cached: true,
                meta,
                content: None,
                outline: None,
            };
            return to_json(output);
        }
    }

    if args.hash {
        let output = ReadOutput {
            file: args.file,
            cached: false,
            meta,
            content: None,
            outline: None,
        };
        return to_json(output);
    }

    let symbols = ext
        .map(|e| outline::extract_symbols(&content, e))
        .unwrap_or_default();
    let symbol_entries = symbols_to_entries(&symbols);

    if args.outline && !args.skeleton {
        let output = ReadOutput {
            file: args.file,
            cached: false,
            meta,
            content: None,
            outline: Some(symbol_entries),
        };
        return to_json(output);
    }

    if args.skeleton {
        let skeleton_text = symbols
            .iter()
            .map(|s| format!("{}  // L{}", s.signature, s.start_line))
            .collect::<Vec<_>>()
            .join("\n");
        let tok = tokens::count_fast(&skeleton_text);
        let output = ReadOutput {
            file: args.file,
            cached: false,
            meta,
            content: Some(ContentBlock {
                range: (1, line_count),
                snap: None,
                snap_reason: None,
                text: skeleton_text,
                tokens: tok,
                truncated: false,
            }),
            outline: Some(symbol_entries),
        };
        return to_json(output);
    }

    let (text, range, snap_info) = if let Some(ref line_spec) = args.lines {
        let (start, end) = parse_line_range(line_spec)?;
        if let Some(ref snap_target) = args.snap {
            let target = parse_snap_target(snap_target)?;
            if let Some(ext_str) = ext {
                if let Some(result) = snap::snap_to_structure(&content, ext_str, start, target) {
                    let text = extract_lines(&content, result.start_line, result.end_line);
                    (
                        text,
                        (result.start_line, result.end_line),
                        Some((
                            snap_target.clone(),
                            format!(
                                "expanded to enclosing {} `{}`",
                                result.target_kind,
                                result.target_name.as_deref().unwrap_or("anonymous")
                            ),
                        )),
                    )
                } else {
                    let text = extract_lines(&content, start, end);
                    (text, (start, end), None)
                }
            } else {
                let text = extract_lines(&content, start, end);
                (text, (start, end), None)
            }
        } else {
            let text = extract_lines(&content, start, end);
            (text, (start, end), None)
        }
    } else {
        (content.clone(), (1, line_count), None)
    };

    let text = apply_read_mode(text, &args.mode, ext)?;

    let mut truncated = false;
    let final_text = if let Some(budget) = args.budget {
        let tok = tokens::count_fast(&text);
        if tok > budget {
            truncated = true;
            truncate_to_budget(&text, budget)
        } else {
            text
        }
    } else {
        text
    };

    let tok = tokens::count_fast(&final_text);
    let output = ReadOutput {
        file: args.file,
        cached: false,
        meta,
        content: Some(ContentBlock {
            range,
            snap: snap_info.as_ref().map(|(s, _)| s.clone()),
            snap_reason: snap_info.map(|(_, r)| r),
            text: final_text,
            tokens: tok,
            truncated,
        }),
        outline: Some(symbol_entries),
    };

    to_json(output)
}

fn apply_read_mode(
    text: String,
    mode: &Option<String>,
    ext: Option<&str>,
) -> Result<String, AgError> {
    match mode.as_deref() {
        None => Ok(text),
        Some("aggressive") => Ok(strip::strip_comments(&text, ext.unwrap_or(""))),
        Some("entropy") => Ok(entropy_filter(&text)),
        Some(other) => Err(AgError::InvalidArgument {
            flag: "mode".to_string(),
            message: format!("expected aggressive|entropy, got `{other}`"),
        }),
    }
}

fn entropy_filter(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 10 {
        return text.to_string();
    }

    let mut seen_patterns: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut result = Vec::new();
    let mut suppressed = 0usize;

    for line in &lines {
        let normalized = line.trim().replace(|c: char| c.is_ascii_digit(), "N");
        let count = seen_patterns.entry(normalized).or_insert(0);
        *count += 1;

        if *count <= 3 {
            result.push(*line);
        } else {
            suppressed += 1;
        }
    }

    if suppressed > 0 {
        let mut out = result.join("\n");
        out.push_str(&format!(
            "\n\n... ({suppressed} repetitive lines filtered)\n"
        ));
        out
    } else {
        text.to_string()
    }
}

fn parse_line_range(spec: &str) -> Result<(usize, usize), AgError> {
    let parts: Vec<&str> = spec.split('-').collect();
    if parts.len() != 2 {
        return Err(AgError::InvalidArgument {
            flag: "lines".to_string(),
            message: format!("expected START-END, got `{spec}`"),
        });
    }
    let start: usize = parts[0].parse().map_err(|_| AgError::InvalidArgument {
        flag: "lines".to_string(),
        message: format!("invalid start line: `{}`", parts[0]),
    })?;
    let end: usize = parts[1].parse().map_err(|_| AgError::InvalidArgument {
        flag: "lines".to_string(),
        message: format!("invalid end line: `{}`", parts[1]),
    })?;
    Ok((start, end))
}

fn parse_snap_target(s: &str) -> Result<snap::SnapTarget, AgError> {
    match s {
        "function" | "fn" => Ok(snap::SnapTarget::Function),
        "class" => Ok(snap::SnapTarget::Class),
        "block" => Ok(snap::SnapTarget::Block),
        _ => Err(AgError::InvalidArgument {
            flag: "snap".to_string(),
            message: format!("expected function|class|block, got `{s}`"),
        }),
    }
}

fn extract_lines(content: &str, start: usize, end: usize) -> String {
    content
        .lines()
        .enumerate()
        .filter(|(i, _)| {
            let line_num = i + 1;
            line_num >= start && line_num <= end
        })
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_to_budget(text: &str, budget: usize) -> String {
    let target_chars = budget * 4;
    if text.len() <= target_chars {
        return text.to_string();
    }
    let mid = text.len() / 2;
    let half = target_chars / 2;
    let start = mid.saturating_sub(half);
    let end = (start + target_chars).min(text.len());
    text[start..end].to_string()
}

fn symbols_to_entries(symbols: &[outline::Symbol]) -> Vec<SymbolEntry> {
    symbols
        .iter()
        .map(|s| SymbolEntry {
            name: s.name.clone(),
            kind: s.kind.to_string(),
            lines: (s.start_line, s.end_line),
            signature: s.signature.clone(),
            children: symbols_to_entries(&s.children),
        })
        .collect()
}

fn to_json(output: ReadOutput) -> Result<serde_json::Value, AgError> {
    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_dir() -> (TempDir, String) {
        let dir = TempDir::new().unwrap();
        let rs_path = dir.path().join("sample.rs");
        std::fs::write(
            &rs_path,
            "fn hello() {\n    println!(\"hi\");\n}\n\nfn world(x: i32) -> i32 {\n    x + 1\n}\n\nstruct Point {\n    x: f64,\n    y: f64,\n}\n",
        )
        .unwrap();
        (dir, rs_path.to_string_lossy().to_string())
    }

    fn read_args(file: &str) -> ReadArgs {
        ReadArgs {
            file: file.to_string(),
            lines: None,
            snap: None,
            skeleton: false,
            outline: false,
            hash: false,
            budget: None,
            meta: false,
            if_changed: None,
            mode: None,
        }
    }

    #[test]
    fn reads_full_file() {
        let (_dir, path) = make_test_dir();
        let result = run(read_args(&path)).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.content.is_some());
        let content = out.content.unwrap();
        assert!(content.text.contains("fn hello"));
        assert!(content.text.contains("fn world"));
        assert!(!content.truncated);
    }

    #[test]
    fn includes_meta_always() {
        let (_dir, path) = make_test_dir();
        let result = run(read_args(&path)).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.meta.language.as_deref(), Some("rust"));
        assert!(out.meta.lines > 0);
        assert!(out.meta.bytes > 0);
        assert_eq!(out.meta.hash.len(), 32);
    }

    #[test]
    fn includes_outline_by_default() {
        let (_dir, path) = make_test_dir();
        let result = run(read_args(&path)).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let outline = out.outline.unwrap();
        let names: Vec<&str> = outline.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"), "missing hello: {names:?}");
        assert!(names.contains(&"world"), "missing world: {names:?}");
        assert!(names.contains(&"Point"), "missing Point: {names:?}");
    }

    #[test]
    fn hash_only_returns_no_content() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.hash = true;
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.content.is_none());
        assert!(out.outline.is_none());
        assert_eq!(out.meta.hash.len(), 32);
    }

    #[test]
    fn outline_only_returns_no_content() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.outline = true;
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.content.is_none());
        assert!(out.outline.is_some());
        assert!(!out.outline.unwrap().is_empty());
    }

    #[test]
    fn skeleton_returns_signatures_only() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.skeleton = true;
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let content = out.content.unwrap();
        assert!(content.text.contains("fn hello()"));
        assert!(content.text.contains("fn world(x: i32)"));
        assert!(!content.text.contains("println!"));
        assert!(content.tokens < out.meta.bytes / 4);
    }

    #[test]
    fn line_range() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.lines = Some("1-3".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let content = out.content.unwrap();
        assert_eq!(content.range, (1, 3));
        assert!(content.text.contains("fn hello"));
        assert!(!content.text.contains("fn world"));
    }

    #[test]
    fn snap_to_function() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.lines = Some("2-2".to_string());
        args.snap = Some("function".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let content = out.content.unwrap();
        assert!(content.text.contains("fn hello"));
        assert!(content.snap.is_some());
        assert!(content.snap_reason.is_some());
    }

    #[test]
    fn budget_truncates() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.budget = Some(5);
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let content = out.content.unwrap();
        assert!(content.truncated);
        assert!(content.tokens <= 5);
    }

    #[test]
    fn nonexistent_file_errors() {
        let args = read_args("/nonexistent/file.rs");
        let err = run(args).unwrap_err();
        assert!(matches!(err, AgError::FileNotFound { .. }));
    }

    #[test]
    fn invalid_line_range_errors() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.lines = Some("abc".to_string());
        let err = run(args).unwrap_err();
        assert!(matches!(err, AgError::InvalidArgument { .. }));
    }

    #[test]
    fn invalid_snap_target_errors() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.lines = Some("1-3".to_string());
        args.snap = Some("invalid".to_string());
        let err = run(args).unwrap_err();
        assert!(matches!(err, AgError::InvalidArgument { .. }));
    }

    #[test]
    fn parse_line_range_valid() {
        assert_eq!(parse_line_range("10-20").unwrap(), (10, 20));
        assert_eq!(parse_line_range("1-1").unwrap(), (1, 1));
    }

    #[test]
    fn parse_line_range_invalid() {
        assert!(parse_line_range("abc").is_err());
        assert!(parse_line_range("1-2-3").is_err());
        assert!(parse_line_range("a-b").is_err());
    }

    #[test]
    fn if_changed_match_returns_cached_stub() {
        let (_dir, path) = make_test_dir();
        let first = run(read_args(&path)).unwrap();
        let first_out: ReadOutput = serde_json::from_value(first).unwrap();
        let hash = first_out.meta.hash.clone();

        let mut args = read_args(&path);
        args.if_changed = Some(hash);
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.cached);
        assert!(out.content.is_none());
        assert!(out.outline.is_none());
        assert_eq!(out.meta.hash, first_out.meta.hash);
        assert_eq!(out.meta.bytes, first_out.meta.bytes);
    }

    #[test]
    fn if_changed_mismatch_returns_full_content() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.if_changed = Some("00000000000000000000000000000000".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(!out.cached);
        assert!(out.content.is_some());
        assert!(out.outline.is_some());
    }

    #[test]
    fn if_changed_malformed_hash_errors() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.if_changed = Some("not-a-hash".to_string());
        let err = run(args).unwrap_err();
        assert!(matches!(err, AgError::InvalidArgument { .. }));
    }

    #[test]
    fn if_changed_match_with_skeleton_still_returns_stub() {
        let (_dir, path) = make_test_dir();
        let first = run(read_args(&path)).unwrap();
        let hash = first
            .get("meta")
            .unwrap()
            .get("hash")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let mut args = read_args(&path);
        args.if_changed = Some(hash);
        args.skeleton = true;
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.cached);
        assert!(out.content.is_none());
    }

    #[test]
    fn if_changed_match_with_lines_still_returns_stub() {
        let (_dir, path) = make_test_dir();
        let first = run(read_args(&path)).unwrap();
        let hash = first
            .get("meta")
            .unwrap()
            .get("hash")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let mut args = read_args(&path);
        args.if_changed = Some(hash);
        args.lines = Some("1-5".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.cached);
        assert!(out.content.is_none());
    }

    #[test]
    fn if_changed_uppercase_hash_matches() {
        let (_dir, path) = make_test_dir();
        let first = run(read_args(&path)).unwrap();
        let hash = first
            .get("meta")
            .unwrap()
            .get("hash")
            .unwrap()
            .as_str()
            .unwrap()
            .to_uppercase();

        let mut args = read_args(&path);
        args.if_changed = Some(hash);
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        assert!(out.cached);
    }

    #[test]
    fn cached_false_not_serialized() {
        let (_dir, path) = make_test_dir();
        let result = run(read_args(&path)).unwrap();
        assert!(result.get("cached").is_none());
    }

    #[test]
    fn mode_aggressive_strips_comments() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("commented.rs");
        std::fs::write(
            &path,
            "// header\nfn main() {\n    // body comment\n    let x = 1;\n}\n",
        )
        .unwrap();
        let path_str = path.to_string_lossy().to_string();

        let mut args = read_args(&path_str);
        args.mode = Some("aggressive".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let text = out.content.unwrap().text;
        assert!(!text.contains("// header"));
        assert!(!text.contains("// body comment"));
        assert!(text.contains("fn main()"));
        assert!(text.contains("let x = 1;"));
    }

    #[test]
    fn mode_aggressive_fewer_tokens() {
        let (_dir, path) = make_test_dir();
        let normal = run(read_args(&path)).unwrap();
        let normal_out: ReadOutput = serde_json::from_value(normal).unwrap();
        let normal_len = normal_out.content.unwrap().text.len();

        let mut args = read_args(&path);
        args.mode = Some("aggressive".to_string());
        let aggressive = run(args).unwrap();
        let agg_out: ReadOutput = serde_json::from_value(aggressive).unwrap();
        let agg_len = agg_out.content.unwrap().text.len();

        assert!(agg_len <= normal_len);
    }

    #[test]
    fn mode_entropy_filters_repetitive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("repetitive.rs");
        let mut content = String::from("fn main() {\n");
        for i in 0..20 {
            content.push_str(&format!("    let field_{i} = \"value\";\n"));
        }
        content.push_str("}\n");
        std::fs::write(&path, &content).unwrap();
        let path_str = path.to_string_lossy().to_string();

        let mut args = read_args(&path_str);
        args.mode = Some("entropy".to_string());
        let result = run(args).unwrap();
        let out: ReadOutput = serde_json::from_value(result).unwrap();

        let text = out.content.unwrap().text;
        assert!(text.contains("repetitive lines filtered"));
        assert!(text.len() < content.len());
    }

    #[test]
    fn mode_invalid_errors() {
        let (_dir, path) = make_test_dir();
        let mut args = read_args(&path);
        args.mode = Some("bogus".to_string());
        let err = run(args).unwrap_err();
        assert!(matches!(err, AgError::InvalidArgument { .. }));
    }
}
