use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::index::persist;
use crate::output::{AgError, to_json};

#[derive(Args)]
pub struct IndexArgs {
    /// Root path to index
    #[arg(default_value = ".")]
    pub path: String,

    /// Watch for file changes and re-index
    #[arg(long)]
    pub watch: bool,

    /// Force full re-index
    #[arg(long)]
    pub rebuild: bool,

    /// Print index statistics
    #[arg(long)]
    pub stats: bool,

    /// Embedding model tier: builtin (default), standard, large
    #[arg(long, default_value = "builtin")]
    pub model: String,
}

#[derive(Serialize)]
struct IndexOutput {
    path: String,
    files_indexed: usize,
    chunks: usize,
    files_changed: usize,
    files_unchanged: usize,
    languages: std::collections::HashMap<String, usize>,
    valid: bool,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub fn run(args: IndexArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    if args.stats {
        let valid = persist::is_valid(root);
        let index_dir = persist::index_path(root);
        let exists = index_dir.exists();
        return to_json(serde_json::json!({
            "path": args.path,
            "index_exists": exists,
            "valid": valid,
        }));
    }

    #[cfg(feature = "watch")]
    if args.watch {
        return watch_and_reindex(root, &args.path, &args.model);
    }

    #[cfg(not(feature = "watch"))]
    if args.watch {
        return Err(AgError::Internal {
            message: "--watch requires building with --features watch".to_string(),
        });
    }

    if !args.rebuild && persist::is_valid(root) {
        return to_json(serde_json::json!({
            "path": args.path,
            "status": "up_to_date",
            "message": "index is current, use --rebuild to force",
        }));
    }

    let start = std::time::Instant::now();
    let stats = persist::build_and_save(root, &args.model)?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let output = IndexOutput {
        path: args.path,
        files_indexed: stats.files,
        chunks: stats.chunks,
        files_changed: stats.files_changed,
        files_unchanged: stats.files_unchanged,
        languages: stats.languages,
        valid: true,
        duration_ms,
        warnings: stats.warnings,
    };

    to_json(output)
}

#[cfg(feature = "watch")]
fn watch_and_reindex(
    root: &Path,
    _path_str: &str,
    model_name: &str,
) -> Result<serde_json::Value, AgError> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc;

    let start = std::time::Instant::now();
    let stats = persist::build_and_save(root, model_name)?;
    eprintln!(
        "indexed {} files ({} chunks) in {}ms, watching for changes...",
        stats.files,
        stats.chunks,
        start.elapsed().as_millis()
    );

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let _ = tx.send(());
            }
        }
    })
    .map_err(|e| AgError::Internal {
        message: format!("failed to create watcher: {e}"),
    })?;

    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|e| AgError::Internal {
            message: format!("failed to watch: {e}"),
        })?;

    while rx.recv().is_ok() {
        std::thread::sleep(std::time::Duration::from_millis(500));
        while rx.try_recv().is_ok() {}

        let reindex_start = std::time::Instant::now();
        match persist::build_and_save(root, model_name) {
            Ok(s) => {
                eprintln!(
                    "re-indexed {} files ({} chunks) in {}ms",
                    s.files,
                    s.chunks,
                    reindex_start.elapsed().as_millis()
                );
            }
            Err(e) => {
                eprintln!("re-index error: {e}");
            }
        }
    }

    Ok(serde_json::json!({"status": "watch_stopped"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_args(dir: &TempDir, rebuild: bool, stats: bool) -> IndexArgs {
        IndexArgs {
            path: dir.path().to_string_lossy().to_string(),
            watch: false,
            rebuild,
            stats,
            model: "builtin".to_string(),
        }
    }

    #[test]
    fn build_index() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let result = run(test_args(&dir, false, false)).unwrap();
        assert!(result["files_indexed"].as_u64().unwrap() >= 1);
        assert!(result["valid"].as_bool().unwrap());
    }

    #[test]
    fn rebuild_skips_when_valid() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        run(test_args(&dir, false, false)).unwrap();

        let result = run(test_args(&dir, false, false)).unwrap();
        assert_eq!(result["status"], "up_to_date");
    }

    #[test]
    fn rebuild_forces_reindex() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        run(test_args(&dir, false, false)).unwrap();

        let result = run(test_args(&dir, true, false)).unwrap();
        assert!(result["files_indexed"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn stats_shows_validity() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        run(test_args(&dir, false, false)).unwrap();

        let result = run(test_args(&dir, false, true)).unwrap();
        assert!(result["valid"].as_bool().unwrap());
        assert!(result["index_exists"].as_bool().unwrap());
    }
}
