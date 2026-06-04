use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::{AgError, to_json};
use crate::parsing::{self, outline};
use crate::search::symbols::SymbolIndex;
use crate::walk::{self, WalkOpts};
use crate::workspace;

#[derive(Args)]
pub struct RenameArgs {
    /// Current symbol name
    pub old_name: String,

    /// New symbol name
    pub new_name: String,

    /// Root path of the codebase
    #[arg(default_value = ".")]
    pub path: String,

    /// Apply changes (default: dry-run preview)
    #[arg(long)]
    pub apply: bool,

    /// Include test files in the rename
    #[arg(long)]
    pub include_tests: bool,
}

#[derive(Serialize, serde::Deserialize)]
struct RenameOutput {
    old_name: String,
    new_name: String,
    applied: bool,
    files_changed: usize,
    total_replacements: usize,
    changes: Vec<FileChange>,
}

#[derive(Serialize, serde::Deserialize)]
struct FileChange {
    file: String,
    replacements: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lines: Vec<LineChange>,
}

#[derive(Serialize, serde::Deserialize)]
struct LineChange {
    line: usize,
    before: String,
    after: String,
}

pub fn run(args: RenameArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    if args.old_name == args.new_name {
        return Err(AgError::InvalidArgument {
            flag: "new_name".to_string(),
            message: "new name must differ from old name".to_string(),
        });
    }

    let index_dir = root.join(".prx/index");
    let symbol_index = SymbolIndex::load(&index_dir).ok();

    let definition_exists = symbol_index
        .as_ref()
        .map(|idx| !idx.lookup_flexible(&args.old_name).is_empty())
        .unwrap_or(false)
        || scan_has_definition(&args.old_name, root);

    if !definition_exists {
        return Err(AgError::InvalidArgument {
            flag: "old_name".to_string(),
            message: format!("symbol '{}' not found in codebase", args.old_name),
        });
    }

    let changes = compute_changes(&args.old_name, &args.new_name, root, args.include_tests);

    let total_replacements: usize = changes.iter().map(|c| c.replacements).sum();

    if args.apply {
        for change in &changes {
            apply_file_change(root, change, &args.old_name, &args.new_name)?;
        }
    }

    let output = RenameOutput {
        old_name: args.old_name,
        new_name: args.new_name,
        applied: args.apply,
        files_changed: changes.len(),
        total_replacements,
        changes,
    };

    to_json(output)
}

fn scan_has_definition(symbol: &str, root: &Path) -> bool {
    let entries = walk::walk(root, &WalkOpts::default());
    for entry in &entries {
        let ext = match parsing::extension_from_path(&entry.path) {
            Some(e) => e,
            None => continue,
        };
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let symbols = outline::extract_symbols(&content, ext);
        let flat: Vec<_> = symbols.iter().flat_map(|s| s.flatten()).collect();
        if flat.iter().any(|s| s.name == symbol) {
            return true;
        }
    }
    false
}

fn compute_changes(
    old_name: &str,
    new_name: &str,
    root: &Path,
    include_tests: bool,
) -> Vec<FileChange> {
    let entries = walk::walk(root, &WalkOpts::default());
    let mut changes = Vec::new();

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains(old_name) {
            continue;
        }

        let rel = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .replace('\\', "/");

        if !include_tests && workspace::is_test_file(&rel) {
            continue;
        }

        let mut line_changes = Vec::new();
        let mut count = 0usize;

        for (i, line) in content.lines().enumerate() {
            if line.contains(old_name) {
                let replaced = line.replace(old_name, new_name);
                let occurrences = line.matches(old_name).count();
                count += occurrences;
                line_changes.push(LineChange {
                    line: i + 1,
                    before: line.to_string(),
                    after: replaced,
                });
            }
        }

        if count > 0 {
            changes.push(FileChange {
                file: rel,
                replacements: count,
                lines: line_changes,
            });
        }
    }

    changes.sort_by(|a, b| a.file.cmp(&b.file));
    changes
}

fn apply_file_change(
    root: &Path,
    change: &FileChange,
    old_name: &str,
    new_name: &str,
) -> Result<(), AgError> {
    let path = root.join(&change.file);
    let content = std::fs::read_to_string(&path).map_err(AgError::Io)?;
    let replaced = content.replace(old_name, new_name);
    std::fs::write(&path, replaced).map_err(AgError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_workspace() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("auth.rs"),
            "pub struct AuthManager {\n    token: String,\n}\n\nimpl AuthManager {\n    pub fn new() -> Self { Self { token: String::new() } }\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::auth::AuthManager;\n\nfn handle(mgr: &AuthManager) {\n    let _ = mgr;\n}\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::write(
            dir.path().join("tests/test_auth.rs"),
            "fn test_auth_manager() {\n    let mgr = AuthManager::new();\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn dry_run_shows_changes_without_modifying() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "AuthManager".to_string(),
            new_name: "SessionManager".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: false,
            include_tests: false,
        };
        let result = run(args).unwrap();
        let out: RenameOutput = serde_json::from_value(result).unwrap();

        assert!(!out.applied);
        assert!(out.files_changed >= 2);
        assert!(out.total_replacements >= 3);

        let content = std::fs::read_to_string(dir.path().join("auth.rs")).unwrap();
        assert!(content.contains("AuthManager"));
    }

    #[test]
    fn apply_renames_across_files() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "AuthManager".to_string(),
            new_name: "SessionManager".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: true,
            include_tests: true,
        };
        let result = run(args).unwrap();
        let out: RenameOutput = serde_json::from_value(result).unwrap();

        assert!(out.applied);
        assert!(out.files_changed >= 2);

        let auth = std::fs::read_to_string(dir.path().join("auth.rs")).unwrap();
        assert!(auth.contains("SessionManager"));
        assert!(!auth.contains("AuthManager"));

        let handler = std::fs::read_to_string(dir.path().join("handler.rs")).unwrap();
        assert!(handler.contains("SessionManager"));
    }

    #[test]
    fn same_name_returns_error() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "Foo".to_string(),
            new_name: "Foo".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: false,
            include_tests: false,
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::InvalidArgument { .. }
        ));
    }

    #[test]
    fn unknown_symbol_returns_error() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "zzz_nonexistent".to_string(),
            new_name: "something".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: false,
            include_tests: false,
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::InvalidArgument { .. }
        ));
    }

    #[test]
    fn excludes_tests_by_default() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "AuthManager".to_string(),
            new_name: "SessionManager".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: false,
            include_tests: false,
        };
        let result = run(args).unwrap();
        let out: RenameOutput = serde_json::from_value(result).unwrap();

        assert!(out.changes.iter().all(|c| !c.file.contains("test_")));
    }

    #[test]
    fn include_tests_flag_includes_tests() {
        let dir = make_workspace();
        let args = RenameArgs {
            old_name: "AuthManager".to_string(),
            new_name: "SessionManager".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            apply: false,
            include_tests: true,
        };
        let result = run(args).unwrap();
        let out: RenameOutput = serde_json::from_value(result).unwrap();

        assert!(out.changes.iter().any(|c| c.file.contains("test_")));
    }
}
