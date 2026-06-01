use std::path::Path;

use bloomfilter::Bloom;
use clap::Args;
use serde::Serialize;

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

    let query_tokens = tokenize::tokenize(&args.pattern);
    let all_present = !query_tokens.is_empty() && query_tokens.iter().all(|t| bloom.check(t));

    let output = ExistsOutput {
        exists: all_present,
        confidence: if all_present {
            "probable".to_string()
        } else {
            "exact".to_string()
        },
        pattern: args.pattern,
    };

    to_json(output)
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
