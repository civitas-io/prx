use sprs::TriMat;
use std::collections::HashMap;

use crate::search::tokenize::tokenize;

const K1: f32 = 1.5;
const B: f32 = 0.75;

pub struct SparseIndex {
    term_to_col: HashMap<String, usize>,
    scores: sprs::CsMat<f32>,
    n_chunks: usize,
}

impl SparseIndex {
    pub fn build(chunk_texts: &[String]) -> Self {
        let n_chunks = chunk_texts.len();
        if n_chunks == 0 {
            return Self {
                term_to_col: HashMap::new(),
                scores: sprs::CsMat::empty(sprs::CompressedStorage::CSC, 0),
                n_chunks: 0,
            };
        }

        let mut term_to_col: HashMap<String, usize> = HashMap::new();
        let mut doc_tokens: Vec<Vec<usize>> = Vec::with_capacity(n_chunks);

        for text in chunk_texts {
            let tokens = tokenize(text);
            let col_ids: Vec<usize> = tokens
                .iter()
                .map(|t| {
                    let len = term_to_col.len();
                    *term_to_col.entry(t.clone()).or_insert(len)
                })
                .collect();
            doc_tokens.push(col_ids);
        }

        let n_terms = term_to_col.len();
        let avgdl = doc_tokens.iter().map(|d| d.len() as f32).sum::<f32>() / n_chunks as f32;

        // df[term] = number of documents containing term
        let mut df: Vec<usize> = vec![0; n_terms];
        for doc in &doc_tokens {
            let mut seen = vec![false; n_terms];
            for &col in doc {
                if !seen[col] {
                    df[col] += 1;
                    seen[col] = true;
                }
            }
        }

        // IDF: log((N - df + 0.5) / (df + 0.5) + 1)
        let idf: Vec<f32> = df
            .iter()
            .map(|&d| {
                let d = d as f32;
                let n = n_chunks as f32;
                ((n - d + 0.5) / (d + 0.5) + 1.0).ln()
            })
            .collect();

        // Build BM25 score matrix as triplets (row=chunk, col=term)
        let mut tri = TriMat::new((n_chunks, n_terms));

        for (doc_idx, doc) in doc_tokens.iter().enumerate() {
            let dl = doc.len() as f32;
            let mut tf: HashMap<usize, usize> = HashMap::new();
            for &col in doc {
                *tf.entry(col).or_insert(0) += 1;
            }

            for (&col, &count) in &tf {
                let tf_val = count as f32;
                let score =
                    idf[col] * (tf_val * (K1 + 1.0)) / (tf_val + K1 * (1.0 - B + B * dl / avgdl));
                if score > 0.0 {
                    tri.add_triplet(doc_idx, col, score);
                }
            }
        }

        let scores = tri.to_csc();

        Self {
            term_to_col,
            scores,
            n_chunks,
        }
    }

    pub fn query(&self, query_text: &str, top_k: usize) -> Vec<(usize, f32)> {
        if self.n_chunks == 0 {
            return vec![];
        }

        let query_tokens = tokenize(query_text);
        let mut chunk_scores = vec![0.0f32; self.n_chunks];

        for token in &query_tokens {
            if let Some(&col) = self.term_to_col.get(token.as_str()) {
                let col_view = self.scores.outer_view(col);
                if let Some(col_data) = col_view {
                    for (row, &val) in col_data.iter() {
                        chunk_scores[row] += val;
                    }
                }
            }
        }

        let mut scored: Vec<(usize, f32)> = chunk_scores
            .into_iter()
            .enumerate()
            .filter(|(_, s)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }
}

pub fn enrich_for_bm25(content: &str, file_path: &str) -> String {
    let path = std::path::Path::new(file_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let dir_parts: Vec<&str> = path
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let dir_text = dir_parts
        .iter()
        .rev()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(" ");

    format!("{content} {stem} {stem} {dir_text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_query() {
        let chunks = vec![
            "fn authenticate(user: &User) -> Token".to_string(),
            "fn process_data(data: &[u8]) -> Result".to_string(),
            "fn validate_token(token: &Token) -> bool".to_string(),
        ];
        let index = SparseIndex::build(&chunks);
        let results = index.query("authenticate", 3);

        assert!(!results.is_empty());
        assert_eq!(results[0].0, 0, "authenticate chunk should rank first");
    }

    #[test]
    fn compound_identifier_match() {
        let chunks = vec![
            "fn getHTTPResponse(url: &str) -> Response".to_string(),
            "fn process(data: Data) -> Output".to_string(),
        ];
        let index = SparseIndex::build(&chunks);

        let results = index.query("http response", 2);
        assert!(!results.is_empty());
        assert_eq!(
            results[0].0, 0,
            "HTTP chunk should rank first on sub-token match"
        );
    }

    #[test]
    fn empty_index() {
        let index = SparseIndex::build(&[]);
        let results = index.query("anything", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let chunks = vec!["fn hello() {}".to_string()];
        let index = SparseIndex::build(&chunks);
        let results = index.query("zzzzz", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn scores_are_positive() {
        let chunks = vec!["fn test() { let x = 1; }".to_string()];
        let index = SparseIndex::build(&chunks);
        let results = index.query("test", 5);
        for (_, score) in &results {
            assert!(*score > 0.0);
        }
    }

    #[test]
    fn enrich_adds_stem_and_dirs() {
        let enriched = enrich_for_bm25("fn auth()", "src/auth/handler.rs");
        assert!(enriched.contains("handler handler"));
        assert!(enriched.contains("auth"));
    }

    #[test]
    fn top_k_limits_results() {
        let chunks: Vec<String> = (0..20)
            .map(|i| format!("fn func_{i}(x: i32) -> i32 {{ x + {i} }}"))
            .collect();
        let index = SparseIndex::build(&chunks);
        let results = index.query("func", 5);
        assert!(results.len() <= 5);
    }
}
