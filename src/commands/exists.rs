use std::path::Path;

use bloomfilter::Bloom;
use clap::Args;
use serde::Serialize;

use crate::index::persist;
use crate::output::{AgError, to_json};
use crate::search::tokenize;
use crate::walk::{self, WalkOpts};

#[derive(Args)]
pub struct ExistsArgs {
    /// Pattern to check
    pub pattern: String,

    /// Root path
    #[arg(default_value = ".")]
    pub path: String,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct ExistsOutput {
    pub exists: bool,
    pub confidence: String,
    pub pattern: String,
}

pub fn run(args: ExistsArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let query_tokens = tokenize::tokenize(&args.pattern);

    let (all_present, confidence) = if persist::is_valid(root) {
        check_via_index(root, &query_tokens)
    } else {
        check_via_bloom(root, &query_tokens)?
    };

    let output = ExistsOutput {
        exists: all_present,
        confidence,
        pattern: args.pattern,
    };

    to_json(output)
}

fn check_via_index(root: &Path, query_tokens: &[String]) -> (bool, String) {
    match persist::load(root) {
        Ok((_chunks, bm25_index)) => {
            let all = !query_tokens.is_empty()
                && query_tokens.iter().all(|t| bm25_index.contains_term(t));
            let confidence = if all { "probable" } else { "exact" };
            (all, confidence.to_string())
        }
        Err(_) => (false, "exact".to_string()),
    }
}

fn check_via_bloom(root: &Path, query_tokens: &[String]) -> Result<(bool, String), AgError> {
    let entries = walk::walk(root, &WalkOpts::default());

    let mut bloom = Bloom::new_for_fp_rate(100_000, 0.02).map_err(|e| AgError::Internal {
        message: format!("bloom filter init: {e}"),
    })?;

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for token in tokenize::tokenize(&content) {
            bloom.set(&token);
        }
    }

    let all_present = !query_tokens.is_empty() && query_tokens.iter().all(|t| bloom.check(t));
    let confidence = if all_present { "probable" } else { "exact" };
    Ok((all_present, confidence.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn finds_existing_identifier() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn authenticate(user: &User) -> Token { }",
        )
        .unwrap();

        let args = ExistsArgs {
            pattern: "authenticate".to_string(),
            path: dir.path().to_string_lossy().to_string(),
        };
        let result = run(args).unwrap();
        let out: ExistsOutput = serde_json::from_value(result).unwrap();

        assert!(out.exists);
        assert_eq!(out.confidence, "probable");
    }

    #[test]
    fn absent_identifier_returns_false() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn hello() {}").unwrap();

        let args = ExistsArgs {
            pattern: "zzz_nonexistent_xyz".to_string(),
            path: dir.path().to_string_lossy().to_string(),
        };
        let result = run(args).unwrap();
        let out: ExistsOutput = serde_json::from_value(result).unwrap();

        assert!(!out.exists);
        assert_eq!(out.confidence, "exact");
    }

    #[test]
    fn uses_persisted_index_when_available() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn compute_hash(data: &[u8]) -> u64 { 42 }",
        )
        .unwrap();

        crate::index::persist::build_and_save(dir.path(), "builtin").unwrap();

        let args = ExistsArgs {
            pattern: "compute_hash".to_string(),
            path: dir.path().to_string_lossy().to_string(),
        };
        let result = run(args).unwrap();
        let out: ExistsOutput = serde_json::from_value(result).unwrap();
        assert!(out.exists);

        let args_absent = ExistsArgs {
            pattern: "zzz_missing_xyz".to_string(),
            path: dir.path().to_string_lossy().to_string(),
        };
        let result_absent = run(args_absent).unwrap();
        let out_absent: ExistsOutput = serde_json::from_value(result_absent).unwrap();
        assert!(!out_absent.exists);
    }

    #[test]
    fn nonexistent_path_errors() {
        let args = ExistsArgs {
            pattern: "test".to_string(),
            path: "/nonexistent/zzz".to_string(),
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::FileNotFound { .. }
        ));
    }
}
