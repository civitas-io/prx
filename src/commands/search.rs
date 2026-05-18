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
use crate::search::{fusion, structural};
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
    let bm25_results = bm25_index.query(query, top_k * 5);

    let dense_index = build_dense_index(&chunk_texts);
    let semantic_results = match &dense_index {
        Some(idx) => idx.search(query, top_k * 5),
        None => vec![],
    };

    let alpha = fusion::resolve_alpha(query, alpha_override);
    let fused = fusion::rrf_fuse(&semantic_results, &bm25_results, alpha);

    let mut score_map: HashMap<usize, f32> = fused.into_iter().collect();
    let ranked = ranking::rerank(
        &mut score_map,
        &chunk_texts,
        &chunk_file_paths,
        query,
        top_k * 2,
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

type InMemoryIndex = (Vec<chunking::Chunk>, Vec<String>, Vec<String>, SparseIndex);

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

fn build_dense_index(chunk_texts: &[String]) -> Option<DenseIndex> {
    let model_bytes: &[u8] = include_bytes!("../../models/potion-code-16M.safetensors");
    if model_bytes.is_empty() {
        return None;
    }

    let tensors = safetensors::SafeTensors::deserialize(model_bytes).ok()?;

    let embedding_tensor = tensors
        .tensor("embeddings")
        .or_else(|_| tensors.tensor("model.embeddings"))
        .or_else(|_| {
            tensors
                .names()
                .into_iter()
                .find(|n| n.contains("embed"))
                .ok_or(safetensors::SafeTensorError::InvalidOffset(
                    "no embedding tensor".into(),
                ))
                .and_then(|name| tensors.tensor(name))
        })
        .ok()?;

    let shape = embedding_tensor.shape();
    if shape.len() != 2 {
        return None;
    }
    let (vocab_size, dim) = (shape[0], shape[1]);

    let data = embedding_tensor.data();
    let weights = match embedding_tensor.dtype() {
        safetensors::Dtype::F32 => {
            let floats: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            ndarray::Array2::from_shape_vec((vocab_size, dim), floats).ok()?
        }
        safetensors::Dtype::F16 => {
            let floats: Vec<f32> = data
                .chunks_exact(2)
                .map(|c| half::f16::from_le_bytes([c[0], c[1]]).to_f32())
                .collect();
            ndarray::Array2::from_shape_vec((vocab_size, dim), floats).ok()?
        }
        _ => return None,
    };

    let vocab = load_model2vec_vocab(vocab_size)?;

    let mut index = DenseIndex::new(vocab, weights);
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
    let results = structural::structural_search(query, root, top_k * 5)?;

    let mut matches: Vec<SearchMatch> = results
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
    to_search_output(matches, total_matches, false, 0)
}

fn load_model2vec_vocab(expected_size: usize) -> Option<HashMap<String, usize>> {
    let tokenizer_bytes: &[u8] = include_bytes!("../../models/model2vec_tokenizer.json");
    if tokenizer_bytes.is_empty() {
        return Some(
            (0..expected_size)
                .map(|i| (format!("token_{i}"), i))
                .collect(),
        );
    }

    let tokenizer_json: serde_json::Value = serde_json::from_slice(tokenizer_bytes).ok()?;
    let vocab_obj = tokenizer_json.get("model")?.get("vocab")?.as_object()?;

    let mut vocab = HashMap::with_capacity(vocab_obj.len());
    for (token, id_val) in vocab_obj {
        if let Some(id) = id_val.as_u64() {
            vocab.insert(token.clone(), id as usize);
        }
    }

    Some(vocab)
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
