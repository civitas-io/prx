use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::{AgError, to_json};
use crate::parsing::{self, outline, snap};
use crate::search::symbols::SymbolIndex;
use crate::workspace;

#[derive(Args)]
pub struct ExplainArgs {
    /// Symbol name to explain
    pub symbol: String,

    /// Root path of the codebase
    #[arg(default_value = ".")]
    pub path: String,

    /// Maximum output tokens
    #[arg(long)]
    pub budget: Option<usize>,
}

#[derive(Serialize, serde::Deserialize)]
struct ExplainOutput {
    symbol: String,
    definitions: Vec<Definition>,
    references: Vec<Reference>,
    tests: Vec<Reference>,
}

#[derive(Serialize, serde::Deserialize)]
struct Definition {
    file: String,
    line: usize,
    kind: String,
    body: Option<String>,
}

#[derive(Serialize, serde::Deserialize)]
struct Reference {
    file: String,
    line: Option<usize>,
    context: Option<String>,
}

pub fn run(args: ExplainArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let index_dir = root.join(".prx/index");
    let symbol_index = SymbolIndex::load(&index_dir).ok();

    let defs = find_definitions(&args.symbol, root, symbol_index.as_ref());
    if defs.is_empty() {
        return Err(AgError::InvalidArgument {
            flag: "symbol".to_string(),
            message: format!("symbol '{}' not found", args.symbol),
        });
    }

    let (refs, tests) = find_references(&args.symbol, root, &defs);

    let mut output = ExplainOutput {
        symbol: args.symbol,
        definitions: defs,
        references: refs,
        tests,
    };

    if let Some(budget) = args.budget {
        apply_budget(&mut output, budget);
    }

    to_json(output)
}

fn find_definitions(
    symbol: &str,
    root: &Path,
    symbol_index: Option<&SymbolIndex>,
) -> Vec<Definition> {
    let mut defs = Vec::new();

    if let Some(idx) = symbol_index {
        let matches = idx.lookup_flexible(symbol);
        for m in matches {
            let file_path = root.join(&m.file);
            let body = read_definition_body(&file_path, m.line);
            defs.push(Definition {
                file: m.file.clone(),
                line: m.line,
                kind: m.kind.clone(),
                body,
            });
        }
    }

    if defs.is_empty() {
        defs = scan_for_definitions(symbol, root);
    }

    defs
}

fn scan_for_definitions(symbol: &str, root: &Path) -> Vec<Definition> {
    let entries = crate::walk::walk(root, &crate::walk::WalkOpts::default());
    let mut defs = Vec::new();

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

        for sym in flat {
            if sym.name == symbol {
                let rel = entry
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&entry.path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let body = read_definition_body(&entry.path, sym.start_line);
                defs.push(Definition {
                    file: rel,
                    line: sym.start_line,
                    kind: sym.kind.clone(),
                    body,
                });
            }
        }
    }

    defs
}

fn read_definition_body(path: &Path, line: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let ext = parsing::extension_from_path(path)?;
    let result = snap::snap_to_structure(&content, ext, line, snap::SnapTarget::Function)
        .or_else(|| snap::snap_to_structure(&content, ext, line, snap::SnapTarget::Class))
        .or_else(|| snap::snap_to_structure(&content, ext, line, snap::SnapTarget::Block))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = result.start_line.saturating_sub(1);
    let end = result.end_line.min(lines.len());
    Some(lines[start..end].join("\n"))
}

fn find_references(
    symbol: &str,
    root: &Path,
    defs: &[Definition],
) -> (Vec<Reference>, Vec<Reference>) {
    let def_files: std::collections::HashSet<&str> = defs.iter().map(|d| d.file.as_str()).collect();

    let entries = crate::walk::walk(root, &crate::walk::WalkOpts::default());
    let mut refs = Vec::new();
    let mut tests = Vec::new();

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains(symbol) {
            continue;
        }

        let rel = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .replace('\\', "/");

        if def_files.contains(rel.as_str()) {
            continue;
        }

        let first_line = content
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains(symbol))
            .map(|(i, _)| i + 1);

        let context =
            first_line.and_then(|ln| content.lines().nth(ln - 1).map(|l| l.trim().to_string()));

        let reference = Reference {
            file: rel.clone(),
            line: first_line,
            context,
        };

        if workspace::is_test_file(&rel) {
            tests.push(reference);
        } else {
            refs.push(reference);
        }
    }

    (refs, tests)
}

fn estimate_tokens(output: &ExplainOutput) -> usize {
    serde_json::to_string(output)
        .map(|s| s.len() / 4)
        .unwrap_or(0)
}

fn apply_budget(output: &mut ExplainOutput, budget: usize) {
    while estimate_tokens(output) > budget && !output.references.is_empty() {
        output.references.pop();
    }
    while estimate_tokens(output) > budget && !output.tests.is_empty() {
        output.tests.pop();
    }
    while estimate_tokens(output) > budget {
        if let Some(def) = output.definitions.last_mut() {
            if def.body.is_some() {
                def.body = None;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_workspace() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("auth.rs"),
            "pub fn authenticate(user: &str) -> bool {\n    user == \"admin\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::auth;\nfn handle() {\n    authenticate(\"bob\");\n}\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::write(
            dir.path().join("tests/test_auth.rs"),
            "fn test_authenticate() {\n    assert!(authenticate(\"admin\"));\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn explains_symbol_with_definition_and_references() {
        let dir = make_workspace();
        let args = ExplainArgs {
            symbol: "authenticate".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            budget: None,
        };
        let result = run(args).unwrap();
        let out: ExplainOutput = serde_json::from_value(result).unwrap();

        assert!(!out.definitions.is_empty());
        assert_eq!(out.definitions[0].file, "auth.rs");
        assert_eq!(out.definitions[0].kind, "function");
        assert!(out.definitions[0].body.is_some());

        assert!(!out.references.is_empty());
        assert!(out.references.iter().any(|r| r.file == "handler.rs"));

        assert!(!out.tests.is_empty());
        assert!(out.tests.iter().any(|r| r.file.contains("test_auth")));
    }

    #[test]
    fn unknown_symbol_returns_error() {
        let dir = make_workspace();
        let args = ExplainArgs {
            symbol: "zzz_nonexistent_xyz".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            budget: None,
        };
        assert!(matches!(
            run(args).unwrap_err(),
            AgError::InvalidArgument { .. }
        ));
    }

    #[test]
    fn budget_trims_references() {
        let dir = make_workspace();
        let args = ExplainArgs {
            symbol: "authenticate".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            budget: Some(50),
        };
        let result = run(args).unwrap();
        let out: ExplainOutput = serde_json::from_value(result).unwrap();
        assert!(!out.definitions.is_empty());
    }
}
