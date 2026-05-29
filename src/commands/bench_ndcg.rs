//! NDCG benchmark runner: measures search quality against a labeled relevance dataset.
//!
//! Unlike the `tests/ndcg.rs` integration test which spawns the `prx` binary per query,
//! this subcommand invokes the in-process search helper directly. The index is loaded
//! once and reused across all queries, avoiding 49× I/O cost per dataset run.

use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use serde::{Deserialize, Serialize};

use crate::index::persist;
use crate::output::AgError;

#[derive(Args)]
pub struct BenchNdcgArgs {
    /// Path to NDCG dataset JSON file
    pub dataset: String,

    /// Root path to search (overrides dataset root)
    #[arg(default_value = ".")]
    pub root: String,
}

#[derive(Deserialize)]
struct Dataset {
    #[serde(default)]
    root: Option<String>,
    queries: Vec<DatasetQuery>,
}

#[derive(Deserialize)]
struct DatasetQuery {
    query: String,
    category: String,
    relevant: Vec<RelevantFile>,
}

#[derive(Deserialize)]
struct RelevantFile {
    file: String,
    relevance: u32,
}

#[derive(Serialize)]
struct BenchNdcgOutput {
    dataset: String,
    root: String,
    queries_evaluated: usize,
    ndcg5: f64,
    ndcg10: f64,
    by_category: HashMap<String, CategoryStats>,
    misses: Vec<String>,
    per_query: Vec<PerQueryResult>,
}

#[derive(Serialize)]
struct CategoryStats {
    ndcg10: f64,
    count: usize,
}

#[derive(Serialize)]
struct PerQueryResult {
    query: String,
    category: String,
    ndcg5: f64,
    ndcg10: f64,
    top_results: Vec<String>,
}

/// Run the NDCG benchmark against a labeled dataset.
///
/// Loads the persistent index once, then runs each query through
/// `search::hybrid_search_with_preloaded`. Deduplicates results by file,
/// scores them against ground-truth relevances, and aggregates NDCG@5 /
/// NDCG@10 overall and per category.
pub fn run(args: BenchNdcgArgs) -> Result<serde_json::Value, AgError> {
    let dataset_path = Path::new(&args.dataset);
    if !dataset_path.exists() {
        return Err(AgError::FileNotFound {
            path: args.dataset.clone(),
        });
    }

    let dataset_str = std::fs::read_to_string(dataset_path)?;
    let dataset: Dataset =
        serde_json::from_str(&dataset_str).map_err(|e| AgError::InvalidArgument {
            flag: "dataset".to_string(),
            message: format!("failed to parse dataset JSON: {e}"),
        })?;

    let root = dataset.root.clone().unwrap_or_else(|| args.root.clone());

    let root_path = Path::new(&root);
    if !root_path.exists() {
        return Err(AgError::FileNotFound { path: root });
    }

    let (chunks, bm25_index) = persist::load(root_path)?;
    let chunk_texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    let chunk_file_paths: Vec<String> = chunks.iter().map(|c| c.file_path.clone()).collect();
    let embeddings = persist::load_embeddings(root_path);
    let model = crate::index::dense::load_model();
    let import_graph = crate::search::graph::ImportGraph::load(&root_path.join(".prx/index")).ok();
    let symbols = persist::load_symbols(root_path);

    let mut all_ndcg5 = Vec::new();
    let mut all_ndcg10 = Vec::new();
    let mut by_category_scores: HashMap<String, Vec<f64>> = HashMap::new();
    let mut misses = Vec::new();
    let mut per_query = Vec::new();

    for q in &dataset.queries {
        let result = match crate::commands::search::hybrid_search_with_preloaded(
            &q.query,
            root_path,
            &chunks,
            &chunk_texts,
            &chunk_file_paths,
            &bm25_index,
            embeddings.as_ref(),
            model.as_ref(),
            import_graph.as_ref(),
            symbols.as_ref(),
            10,
            None,
            None,
            0,
        ) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let matched_files = extract_unique_files(&result);
        let relevances = score_results(&matched_files, &q.relevant);

        let ideal: Vec<f64> = q.relevant.iter().map(|r| r.relevance as f64).collect();
        let n5 = ndcg(&relevances, &ideal, 5);
        let n10 = ndcg(&relevances, &ideal, 10);

        all_ndcg5.push(n5);
        all_ndcg10.push(n10);
        by_category_scores
            .entry(q.category.clone())
            .or_default()
            .push(n10);

        if n10 == 0.0 {
            misses.push(q.query.clone());
        }

        per_query.push(PerQueryResult {
            query: q.query.clone(),
            category: q.category.clone(),
            ndcg5: round4(n5),
            ndcg10: round4(n10),
            top_results: matched_files,
        });
    }

    let mean_ndcg5 = mean(&all_ndcg5);
    let mean_ndcg10 = mean(&all_ndcg10);

    let by_category: HashMap<String, CategoryStats> = by_category_scores
        .into_iter()
        .map(|(cat, scores)| {
            let count = scores.len();
            (
                cat,
                CategoryStats {
                    ndcg10: round4(mean(&scores)),
                    count,
                },
            )
        })
        .collect();

    let output = BenchNdcgOutput {
        dataset: args.dataset,
        root,
        queries_evaluated: all_ndcg10.len(),
        ndcg5: round4(mean_ndcg5),
        ndcg10: round4(mean_ndcg10),
        by_category,
        misses,
        per_query,
    };

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

/// Render the bench-ndcg JSON output as a human-readable table.
pub fn render_plain(data: &serde_json::Value) {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();

    let ndcg5 = data.get("ndcg5").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ndcg10 = data.get("ndcg10").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let queries = data
        .get("queries_evaluated")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let misses = data
        .get("misses")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let _ = writeln!(
        stdout,
        "NDCG@5:  {ndcg5:.3}    NDCG@10: {ndcg10:.3}\nQueries: {queries:<8} Misses:  {misses}\n"
    );

    if let Some(by_cat) = data.get("by_category").and_then(|v| v.as_object()) {
        let _ = writeln!(stdout, "Category          NDCG@10  Count");
        let _ = writeln!(stdout, "─────────────────────────────────");

        let mut rows: Vec<(&String, f64, u64)> = by_cat
            .iter()
            .map(|(k, v)| {
                let n = v.get("ndcg10").and_then(|x| x.as_f64()).unwrap_or(0.0);
                let c = v.get("count").and_then(|x| x.as_u64()).unwrap_or(0);
                (k, n, c)
            })
            .collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (cat, ndcg, count) in rows {
            let _ = writeln!(stdout, "{cat:<17} {ndcg:<7.3}  {count}");
        }
        let _ = writeln!(stdout);
    }

    if let Some(miss_arr) = data.get("misses").and_then(|v| v.as_array())
        && !miss_arr.is_empty()
    {
        let _ = writeln!(stdout, "Misses:");
        for m in miss_arr {
            if let Some(s) = m.as_str() {
                let _ = writeln!(stdout, "  - {s:?}");
            }
        }
    }
}

fn extract_unique_files(result: &serde_json::Value) -> Vec<String> {
    let matches = match result.get("matches").and_then(|m| m.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for m in matches {
        if let Some(file) = m.get("file").and_then(|f| f.as_str())
            && seen.insert(file.to_string())
        {
            unique.push(file.to_string());
        }
    }
    unique
}

// Suffix-match in either direction so absolute and relative paths interop.
fn score_results(matched: &[String], relevant: &[RelevantFile]) -> Vec<f64> {
    matched
        .iter()
        .map(|file| {
            relevant
                .iter()
                .find(|r| file == &r.file || file.ends_with(&r.file) || r.file.ends_with(file))
                .map(|r| r.relevance as f64)
                .unwrap_or(0.0)
        })
        .collect()
}

/// Discounted Cumulative Gain at rank k.
/// DCG@k = sum over i in 1..=k of (2^rel_i - 1) / log2(i + 1)
fn dcg(relevances: &[f64], k: usize) -> f64 {
    relevances
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, &rel)| (2.0_f64.powf(rel) - 1.0) / (i as f64 + 2.0).log2())
        .sum()
}

/// Normalized DCG at rank k: DCG@k divided by the ideal DCG@k computed from
/// the full set of relevant items (not just the matched ones).
fn ndcg(relevances: &[f64], ideal: &[f64], k: usize) -> f64 {
    let mut sorted_ideal = ideal.to_vec();
    sorted_ideal.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let ideal_dcg = dcg(&sorted_ideal, k);
    if ideal_dcg == 0.0 {
        return 0.0;
    }
    dcg(relevances, k) / ideal_dcg
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndcg_perfect_score() {
        let relevances = vec![3.0, 2.0, 1.0];
        let ideal = vec![3.0, 2.0, 1.0];
        let n5 = ndcg(&relevances, &ideal, 5);
        let n10 = ndcg(&relevances, &ideal, 10);
        assert!((n5 - 1.0).abs() < 1e-9, "expected 1.0, got {n5}");
        assert!((n10 - 1.0).abs() < 1e-9, "expected 1.0, got {n10}");
    }

    #[test]
    fn ndcg_empty_results() {
        let relevances: Vec<f64> = vec![];
        let ideal = vec![3.0, 2.0, 1.0];
        assert_eq!(ndcg(&relevances, &ideal, 5), 0.0);
        assert_eq!(ndcg(&relevances, &ideal, 10), 0.0);
    }

    #[test]
    fn ndcg_all_zero_results() {
        let relevances = vec![0.0, 0.0, 0.0];
        let ideal = vec![3.0, 2.0];
        assert_eq!(ndcg(&relevances, &ideal, 5), 0.0);
    }

    #[test]
    fn dcg_computation() {
        // DCG@2 for [3, 2]:
        //   (2^3 - 1) / log2(2) + (2^2 - 1) / log2(3)
        //   = 7 / 1.0 + 3 / 1.5849625...
        //   = 7.0 + 1.892789...
        //   = 8.892789...
        let relevances = vec![3.0, 2.0];
        let computed = dcg(&relevances, 2);
        let expected = 7.0 / 1.0_f64 + 3.0 / 3.0_f64.log2();
        assert!(
            (computed - expected).abs() < 1e-9,
            "expected {expected}, got {computed}"
        );
    }

    #[test]
    fn dcg_respects_k_cutoff() {
        let relevances = vec![3.0, 3.0, 3.0, 3.0, 3.0];
        let at_2 = dcg(&relevances, 2);
        let at_5 = dcg(&relevances, 5);
        assert!(at_5 > at_2, "DCG@5 must exceed DCG@2 with positive gains");
    }

    #[test]
    fn extract_unique_files_dedupes_chunks() {
        let result = serde_json::json!({
            "matches": [
                {"file": "src/auth.rs", "line": 1},
                {"file": "src/auth.rs", "line": 42},
                {"file": "src/token.rs", "line": 7},
                {"file": "src/auth.rs", "line": 100},
            ]
        });
        let files = extract_unique_files(&result);
        assert_eq!(files, vec!["src/auth.rs", "src/token.rs"]);
    }

    #[test]
    fn score_results_handles_suffix_match() {
        let matched = vec![
            "fiddler-v2/fiddler2/libs/authn/providers/zitadel/zitadel.py".to_string(),
            "noise.py".to_string(),
        ];
        let relevant = vec![
            RelevantFile {
                file: "fiddler-v2/fiddler2/libs/authn/providers/zitadel/zitadel.py".to_string(),
                relevance: 3,
            },
            RelevantFile {
                file: "missing.py".to_string(),
                relevance: 2,
            },
        ];
        let scores = score_results(&matched, &relevant);
        assert_eq!(scores, vec![3.0, 0.0]);
    }
}
