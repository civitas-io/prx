pub mod boosting;
pub mod penalties;
pub mod proximity;

use std::collections::HashMap;

use crate::search::graph::ImportGraph;

pub fn cmp_score_desc(a: &(usize, f32), b: &(usize, f32)) -> std::cmp::Ordering {
    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
}

pub fn rerank(
    scores: &mut HashMap<usize, f32>,
    chunk_texts: &[String],
    file_paths: &[String],
    query: &str,
    top_k: usize,
    graph: Option<&ImportGraph>,
) -> Vec<(usize, f32)> {
    rerank_with_config(
        scores,
        chunk_texts,
        file_paths,
        query,
        top_k,
        graph,
        &RerankConfig::default(),
    )
}

#[derive(Clone)]
pub struct RerankConfig {
    pub file_coherence: bool,
    pub definitions: bool,
    pub stem_matches: bool,
    pub proximity: bool,
    pub noise_penalties: bool,
    pub saturation_decay: bool,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            file_coherence: true,
            definitions: true,
            stem_matches: true,
            proximity: true,
            noise_penalties: true,
            saturation_decay: true,
        }
    }
}

pub fn rerank_with_config(
    scores: &mut HashMap<usize, f32>,
    chunk_texts: &[String],
    file_paths: &[String],
    query: &str,
    top_k: usize,
    graph: Option<&ImportGraph>,
    config: &RerankConfig,
) -> Vec<(usize, f32)> {
    if config.file_coherence {
        boosting::boost_file_coherence(scores, file_paths);
    }
    if config.definitions {
        boosting::boost_definitions(scores, chunk_texts, file_paths, query);
    }
    if config.stem_matches {
        boosting::boost_stem_matches(scores, file_paths, query);
    }
    if config.proximity {
        if let Some(g) = graph {
            proximity::boost_proximity(scores, file_paths, g);
        }
    }
    if config.noise_penalties {
        penalties::apply_noise_penalties(scores, file_paths);
    }

    let mut ranked: Vec<(usize, f32)> = scores.iter().map(|(&id, &s)| (id, s)).collect();

    if config.saturation_decay {
        ranked.sort_by(cmp_score_desc);
        penalties::apply_saturation_decay(&ranked, file_paths, top_k)
    } else {
        let k = top_k.min(ranked.len());
        if k > 0 {
            ranked.select_nth_unstable_by(k - 1, cmp_score_desc);
            ranked.truncate(k);
            ranked.sort_by(cmp_score_desc);
        }
        ranked
    }
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
