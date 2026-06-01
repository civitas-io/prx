use std::collections::HashMap;

const RRF_K: f32 = 60.0;

pub fn rrf_fuse(
    semantic_scores: &[(usize, f32)],
    bm25_scores: &[(usize, f32)],
    alpha: f32,
) -> Vec<(usize, f32)> {
    let semantic_rrf = to_rrf(semantic_scores);
    let bm25_rrf = to_rrf(bm25_scores);

    let mut all_ids: Vec<usize> = semantic_rrf
        .keys()
        .chain(bm25_rrf.keys())
        .copied()
        .collect();
    all_ids.sort_unstable();
    all_ids.dedup();

    let mut combined: Vec<(usize, f32)> = all_ids
        .into_iter()
        .map(|id| {
            let sem = semantic_rrf.get(&id).copied().unwrap_or(0.0);
            let bm = bm25_rrf.get(&id).copied().unwrap_or(0.0);
            (id, alpha * sem + (1.0 - alpha) * bm)
        })
        .collect();

    combined.sort_by(crate::ranking::cmp_score_desc);
    combined
}

fn to_rrf(scores: &[(usize, f32)]) -> HashMap<usize, f32> {
    let mut sorted: Vec<(usize, f32)> = scores.to_vec();
    sorted.sort_by(crate::ranking::cmp_score_desc);

    sorted
        .iter()
        .enumerate()
        .map(|(rank, (id, _))| {
            let rrf = 1.0 / (RRF_K + (rank + 1) as f32);
            (*id, rrf)
        })
        .collect()
}

pub fn resolve_alpha(query: &str, override_alpha: Option<f32>) -> f32 {
    if let Some(a) = override_alpha {
        return a.clamp(0.0, 1.0);
    }
    if is_symbol_query(query) {
        0.1
    } else {
        let expanded = expand_synonyms(query);
        if expanded != query { 0.5 } else { 0.6 }
    }
}

static SYNONYMS: &[(&str, &str)] = &[
    ("auth", "authentication"),
    ("authn", "authentication"),
    ("authz", "authorization"),
    ("db", "database"),
    ("k8s", "kubernetes"),
    ("config", "configuration"),
    ("deps", "dependencies"),
    ("env", "environment"),
    ("impl", "implementation"),
    ("repo", "repository"),
    ("msg", "message"),
    ("req", "request"),
    ("res", "response"),
    ("err", "error"),
    ("fn", "function"),
    ("ctx", "context"),
    ("cb", "callback"),
    ("infra", "infrastructure"),
];

pub fn expand_synonyms(query: &str) -> String {
    let mut result = query.to_string();
    for (abbrev, full) in SYNONYMS {
        if query
            .split_whitespace()
            .any(|w| w.eq_ignore_ascii_case(abbrev))
        {
            result = format!("{result} {full}");
        }
    }
    result
}

pub fn is_symbol_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.contains("::") {
        return true;
    }
    if trimmed.starts_with('_') && trimmed.len() > 1 {
        return true;
    }
    let word_count = trimmed.split_whitespace().count();
    if word_count != 1 {
        return false;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() >= 2 {
        let has_internal_upper = chars[1..].iter().any(|c| c.is_uppercase());
        let starts_upper_then_lower =
            chars[0].is_uppercase() && chars.get(1).is_some_and(|c| c.is_lowercase());
        if has_internal_upper || starts_upper_then_lower {
            return true;
        }
    }
    if trimmed.contains('_') && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_rank1_score() {
        let scores = vec![(0, 1.0)];
        let rrf = to_rrf(&scores);
        let expected = 1.0 / (60.0 + 1.0);
        assert!((rrf[&0] - expected).abs() < 1e-6);
    }

    #[test]
    fn rrf_preserves_ranking() {
        let scores = vec![(0, 10.0), (1, 5.0), (2, 1.0)];
        let rrf = to_rrf(&scores);
        assert!(rrf[&0] > rrf[&1]);
        assert!(rrf[&1] > rrf[&2]);
    }

    #[test]
    fn fuse_combines_both_retrievers() {
        let semantic = vec![(0, 1.0), (1, 0.5)];
        let bm25 = vec![(1, 1.0), (2, 0.5)];
        let fused = rrf_fuse(&semantic, &bm25, 0.5);

        let ids: Vec<usize> = fused.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));

        // Chunk 1 appears in both, should rank highest
        assert_eq!(fused[0].0, 1);
    }

    #[test]
    fn alpha_zero_is_pure_bm25() {
        let semantic = vec![(0, 1.0)];
        let bm25 = vec![(1, 1.0)];
        let fused = rrf_fuse(&semantic, &bm25, 0.0);
        assert_eq!(fused[0].0, 1, "alpha=0 should prefer BM25 result");
    }

    #[test]
    fn alpha_one_is_pure_semantic() {
        let semantic = vec![(0, 1.0)];
        let bm25 = vec![(1, 1.0)];
        let fused = rrf_fuse(&semantic, &bm25, 1.0);
        assert_eq!(fused[0].0, 0, "alpha=1 should prefer semantic result");
    }

    #[test]
    fn resolve_alpha_symbol_queries() {
        assert_eq!(resolve_alpha("Foo::bar", None), 0.1);
        assert_eq!(resolve_alpha("_private", None), 0.1);
        assert_eq!(resolve_alpha("getUserById", None), 0.1);
        assert_eq!(resolve_alpha("HandlerStack", None), 0.1);
        assert_eq!(resolve_alpha("feature_impact", None), 0.1);
    }

    #[test]
    fn resolve_alpha_natural_language() {
        assert_eq!(resolve_alpha("retry logic for api", None), 0.6);
        assert_eq!(resolve_alpha("find the error handler", None), 0.6);
    }

    #[test]
    fn resolve_alpha_synonym_expansion() {
        assert_eq!(resolve_alpha("how is auth handled", None), 0.5);
        assert_eq!(resolve_alpha("db migration", None), 0.5);
    }

    #[test]
    fn synonym_expansion() {
        let expanded = expand_synonyms("auth db handler");
        assert!(expanded.contains("authentication"));
        assert!(expanded.contains("database"));
    }

    #[test]
    fn resolve_alpha_override() {
        assert_eq!(resolve_alpha("anything", Some(0.8)), 0.8);
        assert_eq!(resolve_alpha("Foo::bar", Some(0.1)), 0.1);
    }

    #[test]
    fn resolve_alpha_clamps() {
        assert_eq!(resolve_alpha("x", Some(5.0)), 1.0);
        assert_eq!(resolve_alpha("x", Some(-1.0)), 0.0);
    }

    #[test]
    fn empty_inputs() {
        let fused = rrf_fuse(&[], &[], 0.5);
        assert!(fused.is_empty());
    }

    #[test]
    fn one_side_empty() {
        let semantic = vec![(0, 1.0), (1, 0.5)];
        let fused = rrf_fuse(&semantic, &[], 0.5);
        assert_eq!(fused.len(), 2);
    }
}
