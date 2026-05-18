use std::collections::HashMap;
use std::path::Path;

const STRONG_PENALTY: f32 = 0.3;
const MODERATE_PENALTY: f32 = 0.5;
const MILD_PENALTY: f32 = 0.7;
const FILE_SATURATION_DECAY: f32 = 0.5;

pub fn apply_noise_penalties(scores: &mut HashMap<usize, f32>, file_paths: &[String]) {
    for (&chunk_id, score) in scores.iter_mut() {
        if let Some(path) = file_paths.get(chunk_id) {
            let penalty = file_path_penalty(path);
            *score *= penalty;
        }
    }
}

fn file_path_penalty(path: &str) -> f32 {
    let normalized = path.replace('\\', "/");
    let mut penalty = 1.0f32;

    if is_test_file(&normalized) {
        penalty *= STRONG_PENALTY;
    }
    if is_compat_dir(&normalized) {
        penalty *= STRONG_PENALTY;
    }
    if is_examples_dir(&normalized) {
        penalty *= STRONG_PENALTY;
    }
    if is_reexport_file(&normalized) {
        penalty *= MODERATE_PENALTY;
    }
    if normalized.ends_with(".d.ts") {
        penalty *= MILD_PENALTY;
    }

    penalty
}

fn is_test_file(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if name.starts_with("test_") && name.ends_with(".py") {
        return true;
    }
    if name.ends_with("_test.py") || name.ends_with("_test.go") || name.ends_with("_test.rs") {
        return true;
    }
    if name.ends_with(".test.ts")
        || name.ends_with(".test.js")
        || name.ends_with(".test.tsx")
        || name.ends_with(".test.jsx")
        || name.ends_with(".spec.ts")
        || name.ends_with(".spec.js")
    {
        return true;
    }

    path.contains("/tests/")
        || path.contains("/test/")
        || path.contains("/__tests__/")
        || path.contains("/spec/")
        || path.contains("/testing/")
}

fn is_compat_dir(path: &str) -> bool {
    path.contains("/compat/") || path.contains("/_compat/") || path.contains("/legacy/")
}

fn is_examples_dir(path: &str) -> bool {
    path.contains("/examples/") || path.contains("/example/") || path.contains("/docs_src/")
}

fn is_reexport_file(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    name == "__init__.py" || name == "package-info.java"
}

pub fn apply_saturation_decay(
    ranked: &[(usize, f32)],
    file_paths: &[String],
    top_k: usize,
) -> Vec<(usize, f32)> {
    let mut selected = Vec::new();
    let mut file_count: HashMap<&str, usize> = HashMap::new();

    for &(chunk_id, score) in ranked {
        if selected.len() >= top_k {
            break;
        }

        let path = file_paths.get(chunk_id).map(|s| s.as_str()).unwrap_or("");

        let count = file_count.entry(path).or_insert(0);
        let effective_score = if *count > 0 {
            score * FILE_SATURATION_DECAY.powi(*count as i32)
        } else {
            score
        };
        *count += 1;

        selected.push((chunk_id, effective_score));
    }

    selected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_penalized() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let paths = vec!["src/auth.rs".to_string(), "tests/test_auth.py".to_string()];
        apply_noise_penalties(&mut scores, &paths);
        assert!(scores[&0] > scores[&1], "test file should rank lower");
        assert!((scores[&1] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn compat_dir_penalized() {
        let mut scores = HashMap::from([(0, 1.0), (1, 1.0)]);
        let paths = vec![
            "src/auth.rs".to_string(),
            "src/compat/old_auth.rs".to_string(),
        ];
        apply_noise_penalties(&mut scores, &paths);
        assert!(scores[&0] > scores[&1]);
    }

    #[test]
    fn penalties_compound() {
        let mut scores = HashMap::from([(0, 1.0)]);
        let paths = vec!["tests/compat/test_old.py".to_string()];
        apply_noise_penalties(&mut scores, &paths);
        let expected = STRONG_PENALTY * STRONG_PENALTY;
        assert!(
            (scores[&0] - expected).abs() < 1e-6,
            "test + compat = {} but got {}",
            expected,
            scores[&0]
        );
    }

    #[test]
    fn dts_mild_penalty() {
        let mut scores = HashMap::from([(0, 1.0)]);
        let paths = vec!["types/index.d.ts".to_string()];
        apply_noise_penalties(&mut scores, &paths);
        assert!((scores[&0] - MILD_PENALTY).abs() < 1e-6);
    }

    #[test]
    fn reexport_moderate_penalty() {
        let mut scores = HashMap::from([(0, 1.0)]);
        let paths = vec!["src/utils/__init__.py".to_string()];
        apply_noise_penalties(&mut scores, &paths);
        assert!((scores[&0] - MODERATE_PENALTY).abs() < 1e-6);
    }

    #[test]
    fn normal_file_no_penalty() {
        let mut scores = HashMap::from([(0, 1.0)]);
        let paths = vec!["src/auth/handler.rs".to_string()];
        apply_noise_penalties(&mut scores, &paths);
        assert!((scores[&0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn saturation_decay_penalizes_same_file() {
        let ranked = vec![(0, 1.0), (1, 0.9), (2, 0.8)];
        let paths = vec![
            "src/auth.rs".to_string(),
            "src/auth.rs".to_string(),
            "src/other.rs".to_string(),
        ];
        let result = apply_saturation_decay(&ranked, &paths, 3);

        // Chunk 0 and 2 should be unpenalized, chunk 1 should be 0.9 * 0.5
        let chunk1_score = result.iter().find(|(id, _)| *id == 1).unwrap().1;
        assert!(
            (chunk1_score - 0.45).abs() < 1e-6,
            "2nd chunk from same file should be 0.9 * 0.5 = 0.45, got {chunk1_score}"
        );
    }

    #[test]
    fn saturation_top_k_limits() {
        let ranked: Vec<(usize, f32)> = (0..10).map(|i| (i, 1.0 - i as f32 * 0.1)).collect();
        let paths: Vec<String> = (0..10).map(|i| format!("file{i}.rs")).collect();
        let result = apply_saturation_decay(&ranked, &paths, 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn js_test_patterns() {
        assert!(is_test_file("src/auth.test.ts"));
        assert!(is_test_file("src/auth.spec.js"));
        assert!(is_test_file("src/__tests__/auth.ts"));
        assert!(!is_test_file("src/auth.ts"));
    }
}
