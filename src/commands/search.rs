use std::collections::HashMap;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose;
use clap::Args;
use regex::Regex;
use serde::Serialize;

use crate::chunking;
use crate::hash;
use crate::index::dense::DenseIndex;
use crate::index::persist;
use crate::index::sparse::{self, SparseIndex};
use crate::output::AgError;
use crate::parsing;
use crate::ranking;
use crate::search::{fusion, graph::ImportGraph, structural};
use crate::walk::{self, WalkOpts};

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Root path to search
    #[arg(default_value = ".")]
    pub path: String,

    /// Force literal/regex matching
    #[arg(long)]
    pub literal: bool,

    /// Force semantic search
    #[arg(long)]
    pub semantic: bool,

    /// Force ast-grep structural matching
    #[arg(long)]
    pub structural: bool,

    /// Number of results
    #[arg(long, default_value = "5")]
    pub top_k: usize,

    /// Token budget for results
    #[arg(long)]
    pub budget: Option<usize>,

    /// Return enclosing structural unit
    #[arg(long)]
    pub context: Option<String>,

    /// Bloom filter quick check
    #[arg(long)]
    pub exists: bool,

    /// Resume paginated results
    #[arg(long, name = "continue")]
    pub continue_token: Option<String>,

    /// Override RRF alpha weight
    #[arg(long)]
    pub alpha: Option<f32>,
}

#[derive(Serialize, serde::Deserialize)]
pub struct SearchOutput {
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub returned: usize,
    pub budget_used: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Serialize, serde::Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line: usize,
    pub column: usize,
    #[serde(rename = "match")]
    pub matched: String,
    pub snippet: String,
    pub relevance: f32,
    pub hash: String,
}

pub fn run(args: SearchArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let skip = args
        .continue_token
        .as_deref()
        .and_then(decode_continuation)
        .unwrap_or(0);

    let mode = detect_mode(&args);
    match mode {
        SearchMode::Literal => literal_search(&args.query, root, args.top_k, args.budget, skip),
        SearchMode::Semantic | SearchMode::Hybrid => {
            hybrid_search(&args.query, root, args.top_k, args.budget, args.alpha, skip)
        }
        SearchMode::Structural => structural_search_cmd(&args.query, root, args.top_k, args.budget),
    }
}

#[derive(Debug, PartialEq)]
enum SearchMode {
    Literal,
    Semantic,
    Hybrid,
    Structural,
}

fn detect_mode(args: &SearchArgs) -> SearchMode {
    if args.literal {
        return SearchMode::Literal;
    }
    if args.semantic {
        return SearchMode::Semantic;
    }
    if args.structural {
        return SearchMode::Structural;
    }

    let query = args.query.trim();

    if query.contains("$") {
        return SearchMode::Structural;
    }

    if fusion::is_symbol_query(query) {
        return SearchMode::Hybrid;
    }

    let has_regex_meta = query.contains('[')
        || query.contains('(')
        || query.contains('{')
        || query.contains('|')
        || query.contains('\\');
    let token_count = query.split_whitespace().count();

    if token_count < 3 || has_regex_meta {
        return SearchMode::Literal;
    }

    SearchMode::Hybrid
}

fn literal_search(
    pattern: &str,
    root: &Path,
    top_k: usize,
    budget: Option<usize>,
    skip: usize,
) -> Result<serde_json::Value, AgError> {
    let re = Regex::new(pattern).map_err(|e| AgError::InvalidArgument {
        flag: "query".to_string(),
        message: format!("invalid regex: {e}"),
    })?;

    let entries = walk::walk(root, &WalkOpts::default());
    let mut all_matches = Vec::new();

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file_hash = hash::hash_bytes(content.as_bytes());

        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .replace('\\', "/");

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(m) = re.find(line) {
                let start_line = line_idx + 1;
                let snippet = build_snippet(&content, line_idx, 2);

                all_matches.push(SearchMatch {
                    file: rel_path.clone(),
                    line: start_line,
                    column: m.start() + 1,
                    matched: m.as_str().to_string(),
                    snippet,
                    relevance: 1.0,
                    hash: file_hash.clone(),
                });
            }
        }
    }

    let total_matches = all_matches.len();

    if skip > 0 {
        all_matches = all_matches.into_iter().skip(skip).collect();
    }

    if let Some(budget) = budget {
        let mut used = 0;
        all_matches.retain(|m| {
            let cost = m.snippet.len() / 4;
            if used + cost <= budget {
                used += cost;
                true
            } else {
                false
            }
        });
    }

    all_matches.truncate(top_k);
    let has_more = skip + all_matches.len() < total_matches;
    let next_skip = skip + all_matches.len();
    to_search_output(all_matches, total_matches, has_more, next_skip)
}

fn hybrid_search(
    query: &str,
    root: &Path,
    top_k: usize,
    budget: Option<usize>,
    alpha_override: Option<f32>,
    skip: usize,
) -> Result<serde_json::Value, AgError> {
    let (all_chunks, chunk_texts, chunk_file_paths, bm25_index) = if persist::is_valid(root) {
        let (chunks, bm25) = persist::load(root)?;
        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let paths: Vec<String> = chunks.iter().map(|c| c.file_path.clone()).collect();
        (chunks, texts, paths, bm25)
    } else {
        build_index_in_memory(root)?
    };

    if all_chunks.is_empty() {
        return to_search_output(vec![], 0, false, 0);
    }

    let embeddings = persist::load_embeddings(root);
    let model = build_dense_index_model_only();
    let symbols = persist::load_symbols(root);

    hybrid_search_with_preloaded(
        query,
        root,
        &all_chunks,
        &chunk_texts,
        &chunk_file_paths,
        &bm25_index,
        embeddings.as_ref(),
        model.as_ref(),
        None,
        symbols.as_ref(),
        top_k,
        budget,
        alpha_override,
        skip,
    )
}

/// Run hybrid search with pre-loaded index data. Used by bench-ndcg to avoid
/// reloading the index per query.
#[allow(clippy::too_many_arguments)]
pub fn hybrid_search_with_preloaded(
    query: &str,
    root: &Path,
    all_chunks: &[chunking::Chunk],
    chunk_texts: &[String],
    chunk_file_paths: &[String],
    bm25_index: &SparseIndex,
    embeddings: Option<&persist::Embeddings>,
    model: Option<&DenseIndex>,
    preloaded_graph: Option<&ImportGraph>,
    symbols: Option<&crate::search::symbols::SymbolIndex>,
    top_k: usize,
    budget: Option<usize>,
    alpha_override: Option<f32>,
    skip: usize,
) -> Result<serde_json::Value, AgError> {
    if all_chunks.is_empty() {
        return to_search_output(vec![], 0, false, 0);
    }

    let expanded = fusion::expand_synonyms(query);
    let bm25_results = bm25_index.query(&expanded, top_k * 5);

    let semantic_results = match (embeddings, model) {
        (Some(emb), Some(idx)) => {
            let query_vec = idx.embed_text(query);
            let emb_view = emb.view();
            let mut scores: Vec<(usize, f32)> = emb_view
                .rows()
                .into_iter()
                .enumerate()
                .map(|(i, row)| (i, row.dot(&query_vec)))
                .collect();
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scores.truncate(top_k * 5);
            scores
        }
        _ => {
            let dense_index = build_dense_index(chunk_texts);
            match &dense_index {
                Some(idx) => idx.search(query, top_k * 5),
                None => vec![],
            }
        }
    };

    let alpha = fusion::resolve_alpha(query, alpha_override);
    let fused = fusion::rrf_fuse(&semantic_results, &bm25_results, alpha);

    let import_graph_owned = if preloaded_graph.is_some() {
        None
    } else {
        load_or_build_graph(root, &fused, chunk_file_paths)
    };
    let import_graph: Option<&ImportGraph> = preloaded_graph.or(import_graph_owned.as_ref());

    let mut score_map: HashMap<usize, f32> = fused.into_iter().collect();

    if fusion::is_symbol_query(query)
        && let Some(sym_idx) = symbols
    {
        boost_symbol_definitions_with(sym_idx, query, all_chunks, &mut score_map);
    }

    let ranked = ranking::rerank(
        &mut score_map,
        chunk_texts,
        chunk_file_paths,
        query,
        top_k * 2,
        import_graph,
    );

    let mut matches = Vec::new();
    for (chunk_id, score) in &ranked {
        let chunk = match all_chunks.get(*chunk_id) {
            Some(c) => c,
            None => continue,
        };
        let file_hash = std::fs::read(root.join(&chunk.file_path))
            .map(|bytes| hash::hash_bytes(&bytes))
            .unwrap_or_default();

        matches.push(SearchMatch {
            file: chunk.file_path.clone(),
            line: chunk.start_line,
            column: 1,
            matched: query.to_string(),
            snippet: chunk.content.clone(),
            relevance: *score,
            hash: file_hash,
        });
    }

    let total_matches = matches.len();

    if skip > 0 {
        matches = matches.into_iter().skip(skip).collect();
    }

    if let Some(budget) = budget {
        let mut used = 0;
        matches.retain(|m| {
            let cost = m.snippet.len() / 4;
            if used + cost <= budget {
                used += cost;
                true
            } else {
                false
            }
        });
    }

    matches.truncate(top_k);
    let has_more = skip + matches.len() < total_matches;
    let next_skip = skip + matches.len();
    to_search_output(matches, total_matches, has_more, next_skip)
}

const SYMBOL_INDEX_BOOST: f32 = 50.0;

fn boost_symbol_definitions_with(
    symbol_index: &crate::search::symbols::SymbolIndex,
    query: &str,
    chunks: &[chunking::Chunk],
    scores: &mut HashMap<usize, f32>,
) {
    let symbol_name = query.trim().split("::").last().unwrap_or(query.trim());
    let defs = symbol_index.lookup_flexible(symbol_name);
    if defs.is_empty() {
        return;
    }

    for def in &defs {
        for (chunk_id, chunk) in chunks.iter().enumerate() {
            if chunk.file_path == def.file
                && chunk.start_line <= def.line
                && def.line <= chunk.end_line
            {
                let existing = scores.get(&chunk_id).copied().unwrap_or(0.0);
                scores.insert(chunk_id, existing + SYMBOL_INDEX_BOOST);
            }
        }
    }
}

type InMemoryIndex = (Vec<chunking::Chunk>, Vec<String>, Vec<String>, SparseIndex);

fn load_or_build_graph(
    root: &Path,
    fused: &[(usize, f32)],
    chunk_file_paths: &[String],
) -> Option<ImportGraph> {
    let index_dir = root.join(".prx").join("index");
    if let Ok(graph) = ImportGraph::load(&index_dir) {
        return Some(graph);
    }

    let mut seed_paths: Vec<&str> = fused
        .iter()
        .take(20)
        .filter_map(|(id, _)| chunk_file_paths.get(*id).map(|s| s.as_str()))
        .collect();
    seed_paths.sort();
    seed_paths.dedup();

    let all_paths: Vec<String> = {
        let mut p: Vec<String> = chunk_file_paths.to_vec();
        p.sort();
        p.dedup();
        p
    };

    if seed_paths.is_empty() || all_paths.is_empty() {
        return None;
    }

    Some(ImportGraph::build_partial(
        &seed_paths,
        &all_paths,
        |path| std::fs::read_to_string(root.join(path)).ok(),
    ))
}

fn build_index_in_memory(root: &Path) -> Result<InMemoryIndex, AgError> {
    let entries = walk::walk(root, &WalkOpts::default());

    let mut all_chunks = Vec::new();
    let mut chunk_texts = Vec::new();
    let mut chunk_file_paths = Vec::new();

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .replace('\\', "/");

        let ext = parsing::extension_from_path(&entry.path);
        let chunks = chunking::chunk_file(&content, &rel_path, ext);

        for chunk in chunks {
            chunk_texts.push(chunk.content.clone());
            chunk_file_paths.push(rel_path.clone());
            all_chunks.push(chunk);
        }
    }

    let enriched_texts: Vec<String> = all_chunks
        .iter()
        .map(|c| sparse::enrich_for_bm25(&c.content, &c.file_path))
        .collect();
    let bm25_index = SparseIndex::build(&enriched_texts);

    Ok((all_chunks, chunk_texts, chunk_file_paths, bm25_index))
}

fn build_dense_index_model_only() -> Option<DenseIndex> {
    crate::index::dense::load_model()
}

fn build_dense_index(chunk_texts: &[String]) -> Option<DenseIndex> {
    let mut index = crate::index::dense::load_model()?;
    let refs: Vec<&str> = chunk_texts.iter().map(|s| s.as_str()).collect();
    index.index_chunks(&refs);
    Some(index)
}

fn to_search_output(
    matches: Vec<SearchMatch>,
    total_matches: usize,
    has_more: bool,
    next_skip: usize,
) -> Result<serde_json::Value, AgError> {
    to_search_output_with_warning(matches, total_matches, has_more, next_skip, None)
}

fn to_search_output_with_warning(
    matches: Vec<SearchMatch>,
    total_matches: usize,
    has_more: bool,
    next_skip: usize,
    warning: Option<String>,
) -> Result<serde_json::Value, AgError> {
    let returned = matches.len();
    let budget_used = matches.iter().map(|m| m.snippet.len() / 4).sum();
    let continuation_token = if has_more {
        Some(encode_continuation(next_skip))
    } else {
        None
    };
    let output = SearchOutput {
        matches,
        total_matches,
        returned,
        budget_used,
        continuation_token,
        warning,
    };
    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn encode_continuation(skip: usize) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    let _ = write!(buf, "{skip}");
    general_purpose::STANDARD.encode(&buf)
}

fn decode_continuation(token: &str) -> Option<usize> {
    let bytes = general_purpose::STANDARD.decode(token).ok()?;
    let s = std::str::from_utf8(&bytes).ok()?;
    s.parse().ok()
}

fn structural_search_cmd(
    query: &str,
    root: &Path,
    top_k: usize,
    budget: Option<usize>,
) -> Result<serde_json::Value, AgError> {
    let result = structural::structural_search(query, root, top_k * 5)?;

    let mut matches: Vec<SearchMatch> = result
        .matches
        .into_iter()
        .map(|m| SearchMatch {
            file: m.file,
            line: m.line,
            column: m.column,
            matched: m.matched_text,
            snippet: m.snippet,
            relevance: 1.0,
            hash: String::new(),
        })
        .collect();

    let total_matches = matches.len();

    if let Some(budget) = budget {
        let mut used = 0;
        matches.retain(|m| {
            let cost = m.snippet.len() / 4;
            if used + cost <= budget {
                used += cost;
                true
            } else {
                false
            }
        });
    }

    matches.truncate(top_k);
    to_search_output_with_warning(matches, total_matches, false, 0, result.warning)
}

fn build_snippet(content: &str, match_line: usize, context_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = match_line.saturating_sub(context_lines);
    let end = (match_line + context_lines + 1).min(lines.len());
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() {\n    let x = 1;\n}\n",
        )
        .unwrap();

        std::fs::write(
            dir.path().join("lib.py"),
            "def authenticate(user):\n    return check_password(user)\n\ndef process(data):\n    return data\n",
        )
        .unwrap();

        std::fs::write(dir.path().join("empty.txt"), "").unwrap();

        // binary file — should be skipped
        let mut bin = std::fs::File::create(dir.path().join("data.bin")).unwrap();
        bin.write_all(&[0u8; 64]).unwrap();

        dir
    }

    fn args(query: &str, path: &str) -> SearchArgs {
        SearchArgs {
            query: query.to_string(),
            path: path.to_string(),
            literal: true,
            semantic: false,
            structural: false,
            top_k: 5,
            budget: None,
            context: None,
            exists: false,
            continue_token: None,
            alpha: None,
        }
    }

    #[test]
    fn finds_literal_match() {
        let dir = make_test_dir();
        let a = args("fn main", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert_eq!(data.total_matches, 1);
        assert_eq!(data.returned, 1);
        assert_eq!(data.matches[0].file, "main.rs");
        assert_eq!(data.matches[0].line, 1);
        assert_eq!(data.matches[0].column, 1);
        assert_eq!(data.matches[0].matched, "fn main");
    }

    #[test]
    fn finds_across_multiple_files() {
        let dir = make_test_dir();
        let a = args("def ", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert_eq!(data.total_matches, 2);
        let files: Vec<&str> = data.matches.iter().map(|m| m.file.as_str()).collect();
        assert!(files.contains(&"lib.py"));
    }

    #[test]
    fn no_matches_returns_empty() {
        let dir = make_test_dir();
        let a = args("nonexistent_pattern_xyz", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert_eq!(data.total_matches, 0);
        assert_eq!(data.returned, 0);
        assert!(data.matches.is_empty());
    }

    #[test]
    fn nonexistent_path_returns_error() {
        let a = args("test", "/nonexistent/path/zzz");
        let err = run(a).unwrap_err();
        assert!(matches!(err, AgError::FileNotFound { .. }));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let dir = make_test_dir();
        let a = args("[invalid(", dir.path().to_str().unwrap());
        let err = run(a).unwrap_err();
        assert!(matches!(err, AgError::InvalidArgument { .. }));
    }

    #[test]
    fn top_k_limits_results() {
        let dir = make_test_dir();
        let mut a = args(".", dir.path().to_str().unwrap());
        a.top_k = 2;
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert!(data.total_matches > 2);
        assert_eq!(data.returned, 2);
        assert_eq!(data.matches.len(), 2);
    }

    #[test]
    fn budget_limits_results() {
        let dir = make_test_dir();
        let mut a = args(".", dir.path().to_str().unwrap());
        a.top_k = 100;
        a.budget = Some(10);
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert!(data.budget_used <= 10);
        assert!(data.returned < data.total_matches);
    }

    #[test]
    fn matches_have_hashes() {
        let dir = make_test_dir();
        let a = args("fn main", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        for m in &data.matches {
            assert_eq!(m.hash.len(), 32, "hash should be 32 hex chars");
        }
    }

    #[test]
    fn matches_have_context_snippet() {
        let dir = make_test_dir();
        let a = args("helper", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert_eq!(data.matches.len(), 1);
        let snippet = &data.matches[0].snippet;
        assert!(
            snippet.lines().count() >= 3,
            "snippet should include context lines"
        );
    }

    #[test]
    fn skips_binary_files() {
        let dir = make_test_dir();
        let a = args(".", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        let files: Vec<&str> = data.matches.iter().map(|m| m.file.as_str()).collect();
        assert!(
            !files.contains(&"data.bin"),
            "binary file should not appear in results"
        );
    }

    #[test]
    fn regex_matching_works() {
        let dir = make_test_dir();
        let a = args(r"fn \w+\(", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert!(
            data.total_matches >= 2,
            "regex should match fn main( and fn helper("
        );
    }

    #[test]
    fn column_is_one_indexed() {
        let dir = make_test_dir();
        let a = args("println", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();

        assert!(data.matches[0].column >= 1, "column should be 1-indexed");
    }

    #[test]
    fn build_snippet_with_context() {
        let content = "line0\nline1\nline2\nline3\nline4\nline5\n";
        let snippet = build_snippet(content, 3, 2);
        assert!(snippet.contains("line1"));
        assert!(snippet.contains("line3"));
        assert!(snippet.contains("line5"));
    }

    #[test]
    fn build_snippet_at_start_of_file() {
        let content = "line0\nline1\nline2\n";
        let snippet = build_snippet(content, 0, 2);
        assert!(snippet.contains("line0"));
        assert!(snippet.contains("line2"));
    }

    #[test]
    fn build_snippet_at_end_of_file() {
        let content = "line0\nline1\nline2\n";
        let snippet = build_snippet(content, 2, 2);
        assert!(snippet.contains("line0"));
        assert!(snippet.contains("line2"));
    }

    #[test]
    fn output_deserializes_correctly() {
        let dir = make_test_dir();
        let a = args("authenticate", dir.path().to_str().unwrap());
        let result = run(a).unwrap();

        let data: SearchOutput = serde_json::from_value(result).unwrap();
        assert_eq!(data.matches[0].matched, "authenticate");
        assert!(data.budget_used > 0);
    }

    #[test]
    fn auto_detect_literal_for_short_query() {
        let a = SearchArgs {
            query: "fn".to_string(),
            literal: false,
            semantic: false,
            structural: false,
            ..args("", ".")
        };
        assert_eq!(detect_mode(&a), SearchMode::Literal);
    }

    #[test]
    fn auto_detect_semantic_for_natural_language() {
        let a = SearchArgs {
            query: "how is authentication handled in this codebase".to_string(),
            literal: false,
            semantic: false,
            structural: false,
            ..args("", ".")
        };
        assert_eq!(detect_mode(&a), SearchMode::Hybrid);
    }

    #[test]
    fn auto_detect_structural_for_metavar() {
        let a = SearchArgs {
            query: "fn $NAME($$$)".to_string(),
            literal: false,
            semantic: false,
            structural: false,
            ..args("", ".")
        };
        assert_eq!(detect_mode(&a), SearchMode::Structural);
    }

    #[test]
    fn forced_literal_overrides_auto() {
        let a = SearchArgs {
            query: "how is authentication handled in this codebase".to_string(),
            literal: true,
            semantic: false,
            structural: false,
            ..args("", ".")
        };
        assert_eq!(detect_mode(&a), SearchMode::Literal);
    }

    #[test]
    fn forced_semantic_overrides_auto() {
        let a = SearchArgs {
            query: "fn".to_string(),
            literal: false,
            semantic: true,
            structural: false,
            ..args("", ".")
        };
        assert_eq!(detect_mode(&a), SearchMode::Semantic);
    }

    #[test]
    fn hybrid_search_returns_results() {
        let dir = make_test_dir();
        let a = SearchArgs {
            query: "authenticate user password".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            literal: false,
            semantic: true,
            structural: false,
            top_k: 5,
            budget: None,
            context: None,
            exists: false,
            continue_token: None,
            alpha: None,
        };
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();
        assert!(data.returned > 0, "hybrid search should return results");
    }

    #[test]
    fn hybrid_search_with_budget() {
        let dir = make_test_dir();
        let a = SearchArgs {
            query: "authenticate user password".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            literal: false,
            semantic: true,
            structural: false,
            top_k: 10,
            budget: Some(10),
            context: None,
            exists: false,
            continue_token: None,
            alpha: None,
        };
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();
        assert!(data.budget_used <= 10);
    }

    #[test]
    fn continuation_token_returned_when_more_results() {
        let dir = make_test_dir();
        let mut a = args(".", dir.path().to_str().unwrap());
        a.top_k = 1;
        a.literal = true;
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();
        assert!(
            data.continuation_token.is_some(),
            "should have continuation token when more results exist"
        );
    }

    #[test]
    fn continuation_token_absent_when_all_returned() {
        let dir = make_test_dir();
        let a = args("nonexistent_pattern_xyz", dir.path().to_str().unwrap());
        let result = run(a).unwrap();
        let data: SearchOutput = serde_json::from_value(result).unwrap();
        assert!(data.continuation_token.is_none());
    }

    #[test]
    fn continuation_token_paginates() {
        let dir = make_test_dir();
        let mut a1 = args(".", dir.path().to_str().unwrap());
        a1.top_k = 2;
        a1.literal = true;
        let r1 = run(a1).unwrap();
        let d1: SearchOutput = serde_json::from_value(r1).unwrap();

        let token = d1.continuation_token.unwrap();
        let mut a2 = args(".", dir.path().to_str().unwrap());
        a2.top_k = 2;
        a2.literal = true;
        a2.continue_token = Some(token);
        let r2 = run(a2).unwrap();
        let d2: SearchOutput = serde_json::from_value(r2).unwrap();

        let files1: Vec<_> = d1.matches.iter().map(|m| (&m.file, m.line)).collect();
        let files2: Vec<_> = d2.matches.iter().map(|m| (&m.file, m.line)).collect();
        assert_ne!(
            files1, files2,
            "page 2 should have different results than page 1"
        );
    }

    #[test]
    fn encode_decode_roundtrip() {
        let encoded = encode_continuation(42);
        let decoded = decode_continuation(&encoded);
        assert_eq!(decoded, Some(42));
    }
}
