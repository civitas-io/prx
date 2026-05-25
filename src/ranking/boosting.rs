use std::collections::HashMap;
use std::path::Path;

use crate::search::tokenize;

const DEFINITION_BOOST_MULTIPLIER: f32 = 4.0;
const SYMBOL_DEFINITION_BOOST_MULTIPLIER: f32 = 12.0;
const FILE_COHERENCE_BOOST_FRAC: f32 = 0.15;
const STEM_BOOST_MULTIPLIER: f32 = 1.5;
const IMPORT_LINE_PENALTY: f32 = 0.2;

const DEFINITION_KEYWORDS: &[&str] = &[
    "class",
    "module",
    "def",
    "interface",
    "struct",
    "enum",
    "trait",
    "type",
    "func",
    "function",
    "fn",
    "fun",
    "object",
    "record",
    "protocol",
    "typedef",
    "namespace",
    "package",
];

pub fn boost_file_coherence(scores: &mut HashMap<usize, f32>, file_paths: &[String]) {
    if scores.is_empty() {
        return;
    }
    let max_score = scores.values().copied().fold(0.0f32, f32::max);
    if max_score == 0.0 {
        return;
    }

    let mut file_sum: HashMap<&str, f32> = HashMap::new();
    let mut best_chunk: HashMap<&str, usize> = HashMap::new();

    for (&chunk_id, &score) in scores.iter() {
        if let Some(path) = file_paths.get(chunk_id) {
            *file_sum.entry(path.as_str()).or_insert(0.0) += score;
            let current_best = best_chunk.entry(path.as_str()).or_insert(chunk_id);
            if score > *scores.get(current_best).unwrap_or(&0.0) {
                *current_best = chunk_id;
            }
        }
    }

    let max_file_sum = file_sum.values().copied().fold(0.0f32, f32::max);
    if max_file_sum == 0.0 {
        return;
    }

    let boost_unit = max_score * FILE_COHERENCE_BOOST_FRAC;
    for (path, &chunk_id) in &best_chunk {
        let ratio = file_sum[path] / max_file_sum;
        if let Some(score) = scores.get_mut(&chunk_id) {
            *score += boost_unit * ratio;
        }
    }
}

pub fn boost_definitions(
    scores: &mut HashMap<usize, f32>,
    chunk_texts: &[String],
    file_paths: &[String],
    query: &str,
) {
    let max_score = scores.values().copied().fold(0.0f32, f32::max);
    if max_score == 0.0 {
        return;
    }

    let is_symbol = crate::search::fusion::is_symbol_query(query);
    let multiplier = if is_symbol {
        SYMBOL_DEFINITION_BOOST_MULTIPLIER
    } else {
        DEFINITION_BOOST_MULTIPLIER
    };
    let boost_unit = max_score * multiplier;

    let query_tokens = tokenize::tokenize(query);
    let query_names: Vec<String> = query_tokens
        .iter()
        .filter(|t| t.len() > 2)
        .cloned()
        .collect();

    if query_names.is_empty() {
        return;
    }

    for (&chunk_id, score) in scores.iter_mut() {
        let text = match chunk_texts.get(chunk_id) {
            Some(t) => t.as_str(),
            None => continue,
        };

        if chunk_is_mostly_imports(text) {
            *score *= IMPORT_LINE_PENALTY;
            continue;
        }

        if chunk_defines_symbol(text, &query_names) {
            let stem_match = file_paths
                .get(chunk_id)
                .map(|p| {
                    let stem = Path::new(p)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    query_names.iter().any(|n| stem.contains(n.as_str()))
                })
                .unwrap_or(false);

            *score += boost_unit * if stem_match { 1.5 } else { 1.0 };
        }
    }
}

fn chunk_defines_symbol(text: &str, names: &[String]) -> bool {
    for line in text.lines() {
        let trimmed = line.trim();
        let has_keyword = DEFINITION_KEYWORDS
            .iter()
            .any(|kw| trimmed.starts_with(kw) && trimmed[kw.len()..].starts_with([' ', '(']));
        if has_keyword {
            let lower = trimmed.to_lowercase();
            if names.iter().any(|n| lower.contains(n.as_str())) {
                return true;
            }
        }
    }
    false
}

const IMPORT_PREFIXES: &[&str] = &[
    "import ",
    "from ",
    "use ",
    "require(",
    "require_relative",
    "#include",
    "extern crate",
    "pub use ",
    "pub(crate) use ",
    "export {",
    "export *",
];

fn chunk_is_mostly_imports(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 3 {
        return false;
    }
    let import_count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            IMPORT_PREFIXES.iter().any(|p| t.starts_with(p))
        })
        .count();
    import_count as f32 / lines.len() as f32 > 0.6
}

pub fn boost_stem_matches(scores: &mut HashMap<usize, f32>, file_paths: &[String], query: &str) {
    let max_score = scores.values().copied().fold(0.0f32, f32::max);
    if max_score == 0.0 {
        return;
    }

    let keywords: Vec<String> = query
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .collect();

    if keywords.is_empty() {
        return;
    }

    let boost = max_score * STEM_BOOST_MULTIPLIER;

    for (&chunk_id, score) in scores.iter_mut() {
        let path = match file_paths.get(chunk_id) {
            Some(p) => p,
            None => continue,
        };
        let path_parts = extract_path_parts(path);
        let matches = keywords
            .iter()
            .filter(|kw| path_parts.iter().any(|p| p.starts_with(kw.as_str())))
            .count();

        let match_ratio = matches as f32 / keywords.len() as f32;
        if match_ratio >= 0.1 {
            *score += boost * match_ratio;
        }
    }
}

fn extract_path_parts(path: &str) -> Vec<String> {
    let p = Path::new(path);
    let mut parts = Vec::new();

    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
        parts.extend(tokenize::split_identifier(stem));
    }
    if let Some(parent) = p
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
    {
        parts.extend(tokenize::split_identifier(parent));
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_coherence_boosts_multi_chunk_file() {
        let mut scores = HashMap::from([(0, 1.0), (1, 0.8), (2, 0.5)]);
        let paths = vec![
            "src/auth.rs".to_string(),
            "src/auth.rs".to_string(),
            "src/other.rs".to_string(),
        ];
        let original_best = scores[&0];
        boost_file_coherence(&mut scores, &paths);
        assert!(
            scores[&0] > original_best,
            "top chunk of multi-match file should be boosted"
        );
    }

    #[test]
    fn definition_boost_applies() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let texts = vec![
            "fn authenticate(user: &User) -> Token { }".to_string(),
            "let result = authenticate(current_user);".to_string(),
        ];
        let paths = vec!["src/auth.rs".to_string(), "src/handler.rs".to_string()];
        boost_definitions(&mut scores, &texts, &paths, "authenticate");
        assert!(
            scores[&0] > scores[&1],
            "definition site should rank higher"
        );
    }

    #[test]
    fn definition_boost_with_stem_match() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let texts = vec![
            "fn authenticate() {}".to_string(),
            "fn authenticate() {}".to_string(),
        ];
        let paths = vec!["src/auth.rs".to_string(), "src/handler.rs".to_string()];
        boost_definitions(&mut scores, &texts, &paths, "auth");
        assert!(scores[&0] > scores[&1], "stem match should add extra boost");
    }

    #[test]
    fn stem_matching_boosts_path_match() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let paths = vec![
            "src/auth/handler.rs".to_string(),
            "src/utils/helper.rs".to_string(),
        ];
        boost_stem_matches(&mut scores, &paths, "auth handler");
        assert!(scores[&0] > scores[&1]);
    }

    #[test]
    fn stem_matching_requires_minimum_ratio() {
        let mut scores = HashMap::from([(0, 1.0)]);
        let paths = vec!["src/totally_unrelated.rs".to_string()];
        let original = scores[&0];
        boost_stem_matches(&mut scores, &paths, "auth handler login validate");
        assert_eq!(
            scores[&0], original,
            "no keywords matched, should not boost"
        );
    }

    #[test]
    fn empty_scores_no_panic() {
        let mut scores = HashMap::new();
        boost_file_coherence(&mut scores, &[]);
        boost_definitions(&mut scores, &[], &[], "test");
        boost_stem_matches(&mut scores, &[], "test");
    }

    #[test]
    fn chunk_defines_symbol_detection() {
        assert!(chunk_defines_symbol(
            "fn authenticate(user: &User) -> Token",
            &["authenticate".to_string()]
        ));
        assert!(chunk_defines_symbol(
            "class UserService:",
            &["userservice".to_string()]
        ));
        assert!(!chunk_defines_symbol(
            "let x = authenticate();",
            &["authenticate".to_string()]
        ));
    }

    #[test]
    fn chunk_defines_python_class() {
        assert!(chunk_defines_symbol(
            "class ConfigurationManager:\n    def __init__(self):\n        pass",
            &["configurationmanager".to_string()]
        ));
        assert!(chunk_defines_symbol(
            "def get_event_store(config):\n    return EventStore(config)",
            &["get_event_store".to_string()]
        ));
    }

    #[test]
    fn import_chunk_detection() {
        let import_heavy =
            "import os\nimport sys\nimport json\nfrom pathlib import Path\nimport logging\n";
        assert!(chunk_is_mostly_imports(import_heavy));

        let mixed = "import os\n\ndef main():\n    print('hello')\n    return 0\n";
        assert!(!chunk_is_mostly_imports(mixed));

        let code_only = "def main():\n    x = 1\n    return x\n";
        assert!(!chunk_is_mostly_imports(code_only));
    }

    #[test]
    fn import_penalty_applied() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let texts = vec![
            "import os\nimport sys\nimport json\nfrom pathlib import Path\n".to_string(),
            "class ConfigManager:\n    def get(self, key):\n        return self.store[key]\n"
                .to_string(),
        ];
        let paths = vec!["src/imports.py".to_string(), "src/config.py".to_string()];
        boost_definitions(&mut scores, &texts, &paths, "ConfigManager");
        assert!(
            scores[&1] > scores[&0],
            "definition chunk should rank above import-heavy chunk"
        );
    }
}
