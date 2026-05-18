use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::commands::read::SymbolEntry;
use crate::output::AgError;
use crate::parsing::{self, outline};

#[derive(Args)]
pub struct OutlineArgs {
    /// File or directory path
    pub path: String,

    /// For directories, max depth
    #[arg(long)]
    pub depth: Option<usize>,

    /// Filter by symbol kind
    #[arg(long)]
    pub kind: Option<String>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct OutlineOutput {
    pub file: String,
    pub language: Option<String>,
    pub symbols: Vec<SymbolEntry>,
}

pub fn run(args: OutlineArgs) -> Result<serde_json::Value, AgError> {
    let path = Path::new(&args.path);
    if !path.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    if path.is_dir() {
        return outline_directory(path, &args);
    }

    let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
    let ext = parsing::extension_from_path(path);
    let language = ext
        .and_then(parsing::languages::language_name_for_extension)
        .map(String::from);

    let symbols = ext
        .map(|e| outline::extract_symbols(&content, e))
        .unwrap_or_default();

    let entries = symbols_to_entries(&symbols, args.kind.as_deref());

    let output = OutlineOutput {
        file: args.path,
        language,
        symbols: entries,
    };

    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn outline_directory(root: &Path, args: &OutlineArgs) -> Result<serde_json::Value, AgError> {
    use crate::walk::{self, WalkOpts};

    let entries = walk::walk(root, &WalkOpts::default());
    let mut file_outlines = Vec::new();

    for entry in &entries {
        let ext = match parsing::extension_from_path(&entry.path) {
            Some(e) => e,
            None => continue,
        };

        if let Some(max_depth) = args.depth {
            let rel = entry.path.strip_prefix(root).unwrap_or(&entry.path);
            let depth = rel.to_string_lossy().matches('/').count();
            if depth >= max_depth {
                continue;
            }
        }

        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let symbols = outline::extract_symbols(&content, ext);
        if symbols.is_empty() {
            continue;
        }

        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string();

        let language = parsing::languages::language_name_for_extension(ext).map(String::from);
        let symbol_entries = symbols_to_entries(&symbols, args.kind.as_deref());

        file_outlines.push(serde_json::json!({
            "file": rel_path,
            "language": language,
            "symbols": symbol_entries,
        }));
    }

    serde_json::to_value(serde_json::json!({
        "files": file_outlines,
    }))
    .map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn symbols_to_entries(symbols: &[outline::Symbol], kind_filter: Option<&str>) -> Vec<SymbolEntry> {
    symbols
        .iter()
        .filter_map(|s| {
            let kind_str = s.kind.to_string();
            if let Some(filter) = kind_filter {
                if kind_str != filter {
                    return None;
                }
            }
            Some(SymbolEntry {
                name: s.name.clone(),
                kind: kind_str,
                lines: (s.start_line, s.end_line),
                signature: s.signature.clone(),
                children: symbols_to_entries(&s.children, kind_filter),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn outlines_rust_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sample.rs");
        std::fs::write(&path, "fn hello() {}\nstruct Foo {}\n").unwrap();

        let args = OutlineArgs {
            path: path.to_string_lossy().to_string(),
            depth: None,
            kind: None,
        };
        let result = run(args).unwrap();
        let out: OutlineOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.language.as_deref(), Some("rust"));
        let names: Vec<&str> = out.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"Foo"));
    }

    #[test]
    fn kind_filter() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sample.rs");
        std::fs::write(&path, "fn hello() {}\nstruct Foo {}\n").unwrap();

        let args = OutlineArgs {
            path: path.to_string_lossy().to_string(),
            depth: None,
            kind: Some("function".to_string()),
        };
        let result = run(args).unwrap();
        let out: OutlineOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].name, "hello");
    }

    #[test]
    fn nonexistent_errors() {
        let args = OutlineArgs {
            path: "/nonexistent.rs".to_string(),
            depth: None,
            kind: None,
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::FileNotFound { .. }
        ));
    }
}
