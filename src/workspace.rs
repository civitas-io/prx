use std::path::{Path, PathBuf};

/// Compute relative path from `target` to `base`, normalized with forward slashes.
pub fn relative_path(target: &Path, base: &Path) -> Option<String> {
    let target_abs = std::fs::canonicalize(target).ok()?;
    let base_abs = std::fs::canonicalize(base).ok()?;
    target_abs
        .strip_prefix(&base_abs)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// Check if a relative path refers to a test file.
pub fn is_test_file(rel_str: &str) -> bool {
    if rel_str.contains("/tests/") || rel_str.starts_with("tests/") {
        return true;
    }
    if rel_str.contains("__tests__/") {
        return true;
    }
    let name = Path::new(rel_str)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name.starts_with("test_") {
        return true;
    }
    if name.contains("_test.") || name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    false
}

/// Walk upward from `target` to find the project root directory.
/// Checks for `.git`, `.prx`, or `Cargo.toml` markers.
pub fn find_workspace_root(target: &Path) -> Option<PathBuf> {
    let abs = std::fs::canonicalize(target).ok()?;
    let mut current = if abs.is_file() {
        abs.parent()?.to_path_buf()
    } else {
        abs
    };
    for _ in 0..32 {
        if current.join(".git").exists()
            || current.join(".prx").exists()
            || current.join("Cargo.toml").exists()
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
    None
}
