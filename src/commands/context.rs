//! Assemble a context package for a file or directory.
//!
//! Collects file-level outline, doc snippets, entrypoints, and import graph edges
//! into a single JSON envelope that an agent can consume to bootstrap exploration.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::index::persist;
use crate::output::AgError;
use crate::parsing::{self, outline};
use crate::search::graph::ImportGraph;
use crate::search::symbols::SymbolIndex;
use crate::tokens;
use crate::walk::{WalkOpts, walk};

const DEFAULT_BUDGET: usize = 4000;
const MAX_DOC_TOKENS: usize = 400;
const MAX_ENTRYPOINTS: usize = 10;
const MAX_EDGES: usize = 10;

/// CLI arguments for `prx context`.
#[derive(Args)]
pub struct ContextArgs {
    /// File or directory path
    pub path: String,

    /// Maximum output tokens
    #[arg(long)]
    pub budget: Option<usize>,

    /// Skip documentation extraction
    #[arg(long)]
    pub no_doc: bool,

    /// Skip import graph edges
    #[arg(long)]
    pub no_edges: bool,

    /// Include test files
    #[arg(long)]
    pub include_tests: bool,

    /// Max directory depth (default 3)
    #[arg(long, default_value = "3")]
    pub depth: usize,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct ContextOutput {
    target: String,
    kind: String,
    stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<DocBlock>,
    entrypoints: Vec<Entrypoint>,
    files: Vec<FileEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edges: Option<Edges>,
    truncated: bool,
    warnings: Vec<String>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Stats {
    files: usize,
    lines: usize,
    symbols: usize,
    languages: HashMap<String, usize>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct DocBlock {
    source: String,
    text: String,
    tokens: usize,
    truncated: bool,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Entrypoint {
    name: String,
    kind: String,
    file: String,
    line: usize,
    signature: String,
    ref_count: u32,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct FileEntry {
    path: String,
    lines: usize,
    language: Option<String>,
    symbols: Vec<SymbolInfo>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct SymbolInfo {
    name: String,
    kind: String,
    line: usize,
    signature: String,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Edges {
    imports_in: Vec<EdgeIn>,
    imports_out: Vec<EdgeOut>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct EdgeIn {
    file: String,
    symbols: Vec<String>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct EdgeOut {
    file: String,
    symbols: usize,
}

/// Entry point for the `prx context` subcommand.
pub fn run(args: ContextArgs) -> Result<serde_json::Value, AgError> {
    let path = Path::new(&args.path);
    if !path.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let mut warnings = Vec::new();
    let budget = args.budget.unwrap_or(DEFAULT_BUDGET);

    let workspace_root = find_workspace_root(path);
    let symbol_index: Option<SymbolIndex> =
        workspace_root.as_deref().and_then(persist::load_symbols);
    if symbol_index.is_none() {
        warnings.push("index not built; ref counts unavailable".to_string());
    }

    let mut output = if path.is_file() {
        build_file_context(path, &args, symbol_index.as_ref())?
    } else {
        build_directory_context(path, &args, symbol_index.as_ref())?
    };

    if !args.no_edges {
        let edges = workspace_root.as_deref().and_then(|ws| {
            let graph = ImportGraph::load(&persist::index_path(ws)).ok()?;
            let prefix = crate::workspace::relative_path(path, ws)?;
            Some(compute_edges(&graph, &prefix, path.is_file()))
        });
        output.edges = edges;
    }

    let truncated = apply_budget(&mut output, budget);
    if truncated {
        warnings.push(format!("output truncated to {budget} tokens"));
    }
    output.warnings = warnings;

    serde_json::to_value(&output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn build_file_context(
    path: &Path,
    args: &ContextArgs,
    symbol_index: Option<&SymbolIndex>,
) -> Result<ContextOutput, AgError> {
    let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
    let ext = parsing::extension_from_path(path);
    let language = ext
        .and_then(parsing::languages::language_name_for_extension)
        .map(String::from);
    let lines = content.lines().count();
    let symbols = ext
        .map(|e| outline::extract_symbols(&content, e))
        .unwrap_or_default();
    let symbol_infos = flat_symbol_infos(&symbols);
    let symbol_count = symbol_infos.len();

    let mut languages = HashMap::new();
    if let Some(lang) = &language {
        languages.insert(lang.clone(), 1);
    }

    let path_str = path.to_string_lossy().replace('\\', "/");
    let file_pairs = vec![(path_str.clone(), symbols.clone())];
    let entrypoints = compute_entrypoints(&file_pairs, symbol_index);

    let doc = if args.no_doc {
        None
    } else {
        extract_file_doc(path, &content)
    };

    let stats = Stats {
        files: 1,
        lines,
        symbols: symbol_count,
        languages,
    };

    let file_entry = FileEntry {
        path: path_str,
        lines,
        language,
        symbols: symbol_infos,
    };

    Ok(ContextOutput {
        target: args.path.clone(),
        kind: "file".to_string(),
        stats,
        doc,
        entrypoints,
        files: vec![file_entry],
        edges: None,
        truncated: false,
        warnings: Vec::new(),
    })
}

fn build_directory_context(
    root: &Path,
    args: &ContextArgs,
    symbol_index: Option<&SymbolIndex>,
) -> Result<ContextOutput, AgError> {
    let entries = walk(root, &WalkOpts::default());

    let mut file_entries: Vec<FileEntry> = Vec::new();
    let mut file_symbol_pairs: Vec<(String, Vec<outline::Symbol>)> = Vec::new();
    let mut total_lines = 0usize;
    let mut total_symbols = 0usize;
    let mut languages: HashMap<String, usize> = HashMap::new();

    for entry in &entries {
        let rel = entry.path.strip_prefix(root).unwrap_or(&entry.path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let depth = rel_str.matches('/').count();
        if depth >= args.depth {
            continue;
        }

        if !args.include_tests && crate::workspace::is_test_file(&rel_str) {
            continue;
        }

        let ext = match parsing::extension_from_path(&entry.path) {
            Some(e) => e,
            None => continue,
        };

        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines = content.lines().count();
        let symbols = outline::extract_symbols(&content, ext);
        let symbol_infos = flat_symbol_infos(&symbols);
        let language = parsing::languages::language_name_for_extension(ext).map(String::from);

        if let Some(lang) = &language {
            *languages.entry(lang.clone()).or_insert(0) += 1;
        }

        total_lines += lines;
        total_symbols += symbol_infos.len();

        let file_path_str = entry.path.to_string_lossy().replace('\\', "/");
        file_symbol_pairs.push((file_path_str.clone(), symbols));

        file_entries.push(FileEntry {
            path: file_path_str,
            lines,
            language,
            symbols: symbol_infos,
        });
    }

    file_entries.sort_by(|a, b| {
        let da = a.path.matches('/').count();
        let db = b.path.matches('/').count();
        da.cmp(&db)
            .then_with(|| b.symbols.len().cmp(&a.symbols.len()))
    });

    let entrypoints = compute_entrypoints(&file_symbol_pairs, symbol_index);

    let doc = if args.no_doc {
        None
    } else {
        extract_directory_doc(root)
    };

    let stats = Stats {
        files: file_entries.len(),
        lines: total_lines,
        symbols: total_symbols,
        languages,
    };

    Ok(ContextOutput {
        target: args.path.clone(),
        kind: "directory".to_string(),
        stats,
        doc,
        entrypoints,
        files: file_entries,
        edges: None,
        truncated: false,
        warnings: Vec::new(),
    })
}

fn flat_symbol_infos(symbols: &[outline::Symbol]) -> Vec<SymbolInfo> {
    let mut out = Vec::new();
    for s in symbols {
        out.push(SymbolInfo {
            name: s.name.clone(),
            kind: s.kind.to_string(),
            line: s.start_line,
            signature: s.signature.clone(),
        });
        for child in &s.children {
            out.push(SymbolInfo {
                name: child.name.clone(),
                kind: child.kind.to_string(),
                line: child.start_line,
                signature: child.signature.clone(),
            });
        }
    }
    out
}

fn compute_entrypoints(
    file_symbols: &[(String, Vec<outline::Symbol>)],
    symbol_index: Option<&SymbolIndex>,
) -> Vec<Entrypoint> {
    let mut all: Vec<Entrypoint> = Vec::new();
    for (file, symbols) in file_symbols {
        collect_entrypoints_from(symbols, file, symbol_index, &mut all);
    }

    all.sort_by(|a, b| {
        b.ref_count
            .cmp(&a.ref_count)
            .then_with(|| kind_priority(&b.kind).cmp(&kind_priority(&a.kind)))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    all.truncate(MAX_ENTRYPOINTS);
    all
}

fn collect_entrypoints_from(
    symbols: &[outline::Symbol],
    file: &str,
    symbol_index: Option<&SymbolIndex>,
    out: &mut Vec<Entrypoint>,
) {
    for s in symbols {
        let ref_count = symbol_index
            .and_then(|idx| idx.ref_counts.get(&s.name).copied())
            .unwrap_or(0);
        out.push(Entrypoint {
            name: s.name.clone(),
            kind: s.kind.to_string(),
            file: file.to_string(),
            line: s.start_line,
            signature: s.signature.clone(),
            ref_count,
        });
        if !s.children.is_empty() {
            collect_entrypoints_from(&s.children, file, symbol_index, out);
        }
    }
}

fn kind_priority(kind: &str) -> u32 {
    match kind {
        "class" => 5,
        "struct" => 4,
        "trait" | "interface" => 4,
        "function" => 3,
        "method" => 2,
        "const" => 1,
        _ => 0,
    }
}

fn extract_directory_doc(root: &Path) -> Option<DocBlock> {
    for candidate in ["README.md", "README", "readme.md"] {
        let path = root.join(candidate);
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(make_doc_block(
                path.to_string_lossy().replace('\\', "/"),
                content,
            ));
        }
    }
    for candidate in ["mod.rs", "lib.rs"] {
        let path = root.join(candidate);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(text) = extract_first_doc_comment(&content) {
                return Some(make_doc_block(
                    path.to_string_lossy().replace('\\', "/"),
                    text,
                ));
            }
        }
    }
    None
}

fn extract_file_doc(path: &Path, content: &str) -> Option<DocBlock> {
    let text = extract_first_doc_comment(content)?;
    Some(make_doc_block(
        path.to_string_lossy().replace('\\', "/"),
        text,
    ))
}

fn make_doc_block(source: String, text: String) -> DocBlock {
    let raw_tokens = tokens::count_fast(&text);
    let (final_text, truncated) = if raw_tokens > MAX_DOC_TOKENS {
        let max_bytes = MAX_DOC_TOKENS * 4;
        let mut cut = max_bytes.min(text.len());
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        (text[..cut].to_string(), true)
    } else {
        (text, false)
    };
    let final_tokens = tokens::count_fast(&final_text);
    DocBlock {
        source,
        text: final_text,
        tokens: final_tokens,
        truncated,
    }
}

fn extract_first_doc_comment(content: &str) -> Option<String> {
    let mut collected: Vec<String> = Vec::new();
    let mut started = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            collected.push(rest.trim().to_string());
            started = true;
        } else if let Some(rest) = trimmed.strip_prefix("///") {
            collected.push(rest.trim().to_string());
            started = true;
        } else if started {
            break;
        } else if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            continue;
        } else {
            break;
        }
    }
    if collected.is_empty() {
        return None;
    }
    let joined = collected.join("\n");
    let trimmed = joined.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn find_workspace_root(target: &Path) -> Option<PathBuf> {
    crate::workspace::find_workspace_root(target)
}

fn compute_edges(graph: &ImportGraph, target_prefix: &str, target_is_file: bool) -> Edges {
    let mut target_ids: HashSet<u32> = HashSet::new();
    for (i, path) in graph.paths.iter().enumerate() {
        let is_inside = if target_is_file {
            path == target_prefix
        } else if target_prefix.is_empty() {
            true
        } else {
            path == target_prefix || path.starts_with(&format!("{target_prefix}/"))
        };
        if is_inside {
            target_ids.insert(i as u32);
        }
    }

    let mut in_map: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut out_map: HashMap<u32, usize> = HashMap::new();

    for &target_id in &target_ids {
        if let Some(incoming) = graph.reverse.get(target_id as usize) {
            for &src in incoming {
                if !target_ids.contains(&src) {
                    in_map.entry(src).or_default().insert(target_id);
                }
            }
        }
        if let Some(outgoing) = graph.forward.get(target_id as usize) {
            for &dst in outgoing {
                if !target_ids.contains(&dst) {
                    *out_map.entry(dst).or_insert(0) += 1;
                }
            }
        }
    }

    let mut imports_in: Vec<EdgeIn> = in_map
        .into_iter()
        .filter_map(|(ext_id, target_set)| {
            let file = graph.paths.get(ext_id as usize)?.clone();
            let mut symbols: Vec<String> = target_set
                .into_iter()
                .filter_map(|tid| {
                    graph.paths.get(tid as usize).and_then(|p| {
                        Path::new(p)
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                    })
                })
                .collect();
            symbols.sort();
            symbols.dedup();
            Some(EdgeIn { file, symbols })
        })
        .collect();
    imports_in.sort_by(|a, b| {
        b.symbols
            .len()
            .cmp(&a.symbols.len())
            .then_with(|| a.file.cmp(&b.file))
    });
    imports_in.truncate(MAX_EDGES);

    let mut imports_out: Vec<EdgeOut> = out_map
        .into_iter()
        .filter_map(|(ext_id, count)| {
            graph.paths.get(ext_id as usize).map(|p| EdgeOut {
                file: p.clone(),
                symbols: count,
            })
        })
        .collect();
    imports_out.sort_by(|a, b| b.symbols.cmp(&a.symbols).then_with(|| a.file.cmp(&b.file)));
    imports_out.truncate(MAX_EDGES);

    Edges {
        imports_in,
        imports_out,
    }
}

fn current_tokens(output: &ContextOutput) -> usize {
    let s = serde_json::to_string(output).unwrap_or_default();
    tokens::count_fast(&s)
}

fn apply_budget(output: &mut ContextOutput, budget: usize) -> bool {
    if current_tokens(output) <= budget {
        return false;
    }

    let mut truncated = false;

    if output.edges.is_some() {
        output.edges = None;
        truncated = true;
        if current_tokens(output) <= budget {
            output.truncated = true;
            return true;
        }
    }

    while output.files.len() > 1 && current_tokens(output) > budget {
        output.files.pop();
        truncated = true;
    }
    if current_tokens(output) <= budget {
        if truncated {
            output.truncated = true;
        }
        return truncated;
    }

    while !output.entrypoints.is_empty() && current_tokens(output) > budget {
        output.entrypoints.pop();
        truncated = true;
    }
    if current_tokens(output) <= budget {
        if truncated {
            output.truncated = true;
        }
        return truncated;
    }

    if output.doc.is_some() {
        loop {
            let current = current_tokens(output);
            let Some(doc) = output.doc.as_mut() else {
                break;
            };
            if doc.text.len() <= 50 || current <= budget {
                break;
            }
            let mut cut = doc.text.len() / 2;
            while cut > 0 && !doc.text.is_char_boundary(cut) {
                cut -= 1;
            }
            doc.text.truncate(cut);
            doc.tokens = tokens::count_fast(&doc.text);
            doc.truncated = true;
            truncated = true;
            if cut == 0 {
                break;
            }
        }
    }

    if truncated {
        output.truncated = true;
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn default_args(path: String) -> ContextArgs {
        ContextArgs {
            path,
            budget: None,
            no_doc: false,
            no_edges: false,
            include_tests: false,
            depth: 3,
        }
    }

    #[test]
    fn context_single_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("lib.rs");
        std::fs::write(
            &file,
            "/// Auth library.\npub fn authenticate() {}\npub struct Token;\n",
        )
        .unwrap();

        let result = run(default_args(file.to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.kind, "file");
        assert_eq!(out.stats.files, 1);
        assert_eq!(out.files.len(), 1);
        assert!(
            out.files[0]
                .symbols
                .iter()
                .any(|s| s.name == "authenticate")
        );
        assert!(out.entrypoints.iter().any(|e| e.name == "authenticate"));
        assert_eq!(out.files[0].language.as_deref(), Some("rust"));
    }

    #[test]
    fn context_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("a.rs"),
            "pub fn func_a() {}\npub struct StructA;\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.rs"),
            "pub fn func_b() {}\npub struct StructB;\n",
        )
        .unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.kind, "directory");
        assert_eq!(out.stats.files, 2);
        assert!(out.files.len() >= 2);
        assert!(out.stats.symbols >= 4);
        assert!(out.stats.languages.contains_key("rust"));
    }

    #[test]
    fn context_nonexistent_path() {
        let result = run(default_args("/nonexistent_path_42_abc_xyz".to_string()));
        assert!(matches!(result, Err(AgError::FileNotFound { .. })));
    }

    #[test]
    fn context_no_doc_flag() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "# Test\n\nA test directory.\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.no_doc = true;

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert!(out.doc.is_none());
    }

    #[test]
    fn context_no_edges_flag() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.no_edges = true;

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert!(out.edges.is_none());
    }

    #[test]
    fn context_budget_truncates() {
        let dir = TempDir::new().unwrap();
        for i in 0..15 {
            let content = format!(
                "pub fn function_with_a_very_descriptive_name_{i}() {{}}\n\
                 pub struct StructWithDescriptiveName_{i};\n\
                 impl StructWithDescriptiveName_{i} {{\n    pub fn method_one() {{}}\n    pub fn method_two() {{}}\n}}\n"
            );
            std::fs::write(dir.path().join(format!("file_{i}.rs")), content).unwrap();
        }

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.budget = Some(100);

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert!(out.truncated);
        assert_eq!(
            out.stats.files, 15,
            "stats must be preserved after trimming"
        );
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("output truncated to 100 tokens"))
        );
    }

    #[test]
    fn context_excludes_test_files_by_default() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.path().join("test_helper.rs"), "fn helper() {}\n").unwrap();
        std::fs::write(dir.path().join("auth_test.rs"), "fn test() {}\n").unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let names: Vec<&str> = out
            .files
            .iter()
            .filter_map(|f| Path::new(&f.path).file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names.contains(&"main.rs"));
        assert!(!names.contains(&"test_helper.rs"));
        assert!(!names.contains(&"auth_test.rs"));
    }

    #[test]
    fn context_includes_tests_with_flag() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.path().join("test_helper.rs"), "fn helper() {}\n").unwrap();

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.include_tests = true;

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let names: Vec<&str> = out
            .files
            .iter()
            .filter_map(|f| Path::new(&f.path).file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names.contains(&"main.rs"));
        assert!(names.contains(&"test_helper.rs"));
    }

    #[test]
    fn context_reads_readme_doc() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "# Auth\n\nAuthentication module.\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("lib.rs"), "fn placeholder() {}\n").unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let doc = out.doc.expect("doc should be present");
        assert!(doc.source.ends_with("README.md"));
        assert!(doc.text.contains("Authentication module"));
    }

    #[test]
    fn context_falls_back_to_doc_comment() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("lib.rs"),
            "//! Auth fallback doc.\n//! Multi-line context.\npub fn run() {}\n",
        )
        .unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let doc = out.doc.expect("doc should be present");
        assert!(doc.text.contains("Auth fallback doc"));
        assert!(doc.text.contains("Multi-line context"));
    }

    #[test]
    fn context_warns_when_no_index() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert!(out.warnings.iter().any(|w| w.contains("index not built")));
    }

    #[test]
    fn context_respects_depth() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("top.rs"), "fn t() {}\n").unwrap();
        std::fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        std::fs::write(dir.path().join("a/b/c/deep.rs"), "fn d() {}\n").unwrap();

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.depth = 1;

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let names: Vec<&str> = out
            .files
            .iter()
            .filter_map(|f| Path::new(&f.path).file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names.contains(&"top.rs"));
        assert!(!names.contains(&"deep.rs"));
    }

    #[test]
    fn context_python_class_with_methods() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("module.py");
        std::fs::write(
            &file,
            "class Foo:\n    def bar(self):\n        pass\n    def baz(self):\n        pass\n",
        )
        .unwrap();

        let result = run(default_args(file.to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.files.len(), 1);
        let symbols = &out.files[0].symbols;

        let symbol_names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(symbol_names.contains(&"Foo"));
        assert!(symbol_names.contains(&"bar"));
        assert!(symbol_names.contains(&"baz"));
    }

    #[test]
    fn context_entrypoints_sorted_by_kind() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("mixed.rs"),
            "pub struct MyStruct;\npub fn my_function() {}\npub const MY_CONST: i32 = 42;\n",
        )
        .unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let kinds: Vec<&str> = out.entrypoints.iter().map(|e| e.kind.as_str()).collect();

        let struct_idx = kinds.iter().position(|k| *k == "struct");
        let fn_idx = kinds.iter().position(|k| *k == "function");
        let const_idx = kinds.iter().position(|k| *k == "const");

        if let (Some(s), Some(f)) = (struct_idx, fn_idx) {
            assert!(s < f);
        }
        if let (Some(f), Some(c)) = (fn_idx, const_idx) {
            assert!(f < c);
        }
    }

    #[test]
    fn context_test_file_patterns() {
        let dir = TempDir::new().unwrap();

        std::fs::create_dir_all(dir.path().join("src/__tests__")).unwrap();
        std::fs::write(
            dir.path().join("src/__tests__/foo.py"),
            "def test_foo(): pass\n",
        )
        .unwrap();

        std::fs::write(dir.path().join("src/foo_test.py"), "def test_foo(): pass\n").unwrap();

        std::fs::write(
            dir.path().join("src/foo.test.js"),
            "test('foo', () => {});\n",
        )
        .unwrap();

        std::fs::write(
            dir.path().join("src/foo.spec.ts"),
            "describe('foo', () => {});\n",
        )
        .unwrap();

        std::fs::write(dir.path().join("src/normal.py"), "def normal(): pass\n").unwrap();

        let result = run(default_args(dir.path().to_string_lossy().to_string())).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let file_names: Vec<&str> = out
            .files
            .iter()
            .filter_map(|f| Path::new(&f.path).file_name().and_then(|n| n.to_str()))
            .collect();

        assert!(file_names.contains(&"normal.py"));
        assert!(!file_names.contains(&"foo.py"));
        assert!(!file_names.contains(&"foo_test.py"));
        assert!(!file_names.contains(&"foo.test.js"));
        assert!(!file_names.contains(&"foo.spec.ts"));
    }

    #[test]
    fn context_budget_trims_entrypoints() {
        let dir = TempDir::new().unwrap();

        for i in 0..12 {
            let content = format!(
                "pub fn function_{i}() {{}}\n\
                 pub struct Struct_{i};\n\
                 pub const CONST_{i}: i32 = {i};\n"
            );
            std::fs::write(dir.path().join(format!("file_{i}.rs")), content).unwrap();
        }

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.budget = Some(200);

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        assert!(out.truncated);
        assert!(out.entrypoints.len() <= MAX_ENTRYPOINTS);
    }

    #[test]
    fn context_budget_trims_doc() {
        let dir = TempDir::new().unwrap();

        let long_doc = "# Documentation\n\n".to_string()
            + &"This is a very long documentation string that contains many words. ".repeat(100);

        std::fs::write(dir.path().join("README.md"), &long_doc).unwrap();
        std::fs::write(dir.path().join("lib.rs"), "fn placeholder() {}\n").unwrap();

        let mut args = default_args(dir.path().to_string_lossy().to_string());
        args.budget = Some(300);

        let result = run(args).unwrap();
        let out: ContextOutput = serde_json::from_value(result).unwrap();

        let doc = out.doc.expect("doc should be present");
        assert!(doc.truncated);
        assert!(doc.text.len() < long_doc.len());
    }
}
