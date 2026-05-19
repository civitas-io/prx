pub mod boosting;
pub mod penalties;
pub mod proximity;

use std::collections::HashMap;

use crate::search::graph::ImportGraph;

pub fn rerank(
    scores: &mut HashMap<usize, f32>,
    chunk_texts: &[String],
    file_paths: &[String],
    query: &str,
    top_k: usize,
    graph: Option<&ImportGraph>,
) -> Vec<(usize, f32)> {
    boosting::boost_file_coherence(scores, file_paths);
    boosting::boost_definitions(scores, chunk_texts, file_paths, query);
    boosting::boost_stem_matches(scores, file_paths, query);
    if let Some(g) = graph {
        proximity::boost_proximity(scores, file_paths, g);
    }
    penalties::apply_noise_penalties(scores, file_paths);

    let mut ranked: Vec<(usize, f32)> = scores.iter().map(|(&id, &s)| (id, s)).collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    penalties::apply_saturation_decay(&ranked, file_paths, top_k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_pipeline_runs() {
        let mut scores = HashMap::from([(0, 1.0), (1, 0.8), (2, 0.5)]);
        let texts = vec![
            "fn authenticate(user: &User) -> Token { }".to_string(),
            "let result = authenticate(current_user);".to_string(),
            "fn process(data: &[u8]) -> Result".to_string(),
        ];
        let paths = vec![
            "src/auth.rs".to_string(),
            "src/handler.rs".to_string(),
            "tests/test_process.py".to_string(),
        ];
        let result = rerank(&mut scores, &texts, &paths, "authenticate", 3, None);

        assert!(!result.is_empty());
        assert_eq!(result[0].0, 0, "definition chunk should rank first");
    }

    #[test]
    fn test_file_ranks_last() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let texts = vec!["fn auth() {}".to_string(), "fn test_auth() {}".to_string()];
        let paths = vec!["src/auth.rs".to_string(), "tests/test_auth.py".to_string()];
        let result = rerank(&mut scores, &texts, &paths, "auth", 2, None);
        assert_eq!(result[0].0, 0, "source file should rank above test file");
    }

    #[test]
    fn empty_scores_no_panic() {
        let mut scores = HashMap::new();
        let result = rerank(&mut scores, &[], &[], "test", 5, None);
        assert!(result.is_empty());
    }
}
