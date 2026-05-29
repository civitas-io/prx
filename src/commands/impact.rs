//! Analyze reverse dependencies (impact) for a file or symbol.
//!
//! Computes which files depend on a target file by walking the import graph
//! in the reverse direction. Optionally narrows the analysis to a specific
//! symbol exported by the target file.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::index::persist;
use crate::output::AgError;
use crate::parsing::{self, imports, outline};
use crate::search::graph::ImportGraph;
use crate::tokens;
use crate::walk::{WalkOpts, walk};

const MAX_HOPS: u8 = 5;
const HIGH_FAN_IN_THRESHOLD: usize = 50;

/// CLI arguments for `prx impact`.
#[derive(Args)]
pub struct ImpactArgs {
    /// Target file path
    pub path: String,

    /// Narrow impact to a specific symbol
    #[arg(long)]
    pub symbol: Option<String>,

    /// Maximum hops in dependency graph (default 2, max 5)
    #[arg(long, default_value = "2")]
    pub hops: u8,

    /// Maximum output tokens
    #[arg(long)]
    pub budget: Option<usize>,

    /// Exclude test files from results
    #[arg(long)]
    pub no_tests: bool,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct ImpactOutput {
    target: Target,
    dependents: Vec<Dependent>,
    stats: Stats,
    truncated: bool,
    warnings: Vec<String>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Target {
    file: String,
    symbol: Option<String>,
    exports: Vec<Export>,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Export {
    name: String,
    kind: String,
    line: usize,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Dependent {
    file: String,
    hops: u8,
    uses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<String>,
    is_test: bool,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
struct Stats {
    direct: usize,
    transitive: usize,
    test_files: usize,
    total: usize,
    max_hops_reached: u8,
}

/// Entry point for the `prx impact` subcommand.
pub fn run(args: ImpactArgs) -> Result<serde_json::Value, AgError> {
    let path = Path::new(&args.path);
    if !path.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }
    if !path.is_file() {
        return Err(AgError::InvalidArgument {
            flag: "path".to_string(),
            message: format!("path must be a file: {}", args.path),
        });
    }

    let mut warnings: Vec<String> = Vec::new();

    let mut hops = args.hops;
    if hops > MAX_HOPS {
        warnings.push(format!("hops capped from {hops} to {MAX_HOPS}"));
        hops = MAX_HOPS;
    }

    let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
    let ext = parsing::extension_from_path(path).unwrap_or("");
    let symbols = outline::extract_symbols(&content, ext);
    let mut exports = flat_exports(&symbols);

    if let Some(sym) = &args.symbol {
        if !exports.iter().any(|e| &e.name == sym) {
            return Err(AgError::InvalidArgument {
                flag: "symbol".to_string(),
                message: format!("symbol '{sym}' not found in {}", args.path),
            });
        }
    }

    let workspace_root = find_workspace_root(path);
    let (graph, target_id) = load_or_build_graph(workspace_root.as_deref(), path, &mut warnings);

    let mut dependents: Vec<Dependent> =
        compute_dependents(&graph, target_id, hops, &exports, workspace_root.as_deref());

    if args.no_tests {
        dependents.retain(|d| !d.is_test);
    }

    let direct_count_initial = dependents.iter().filter(|d| d.hops == 1).count();
    if direct_count_initial > HIGH_FAN_IN_THRESHOLD && hops > 1 {
        warnings.push(format!(
            "high fan-in ({direct_count_initial} direct dependents); hops capped to 1"
        ));
        dependents.retain(|d| d.hops == 1);
    }

    if let Some(sym) = &args.symbol {
        exports.retain(|e| &e.name == sym);

        let hop1_with_sym: HashSet<String> = dependents
            .iter()
            .filter(|d| d.hops == 1 && d.uses.iter().any(|u| u == sym))
            .map(|d| d.file.clone())
            .collect();

        dependents.retain(|d| {
            if d.hops == 1 {
                d.uses.iter().any(|u| u == sym)
            } else {
                d.via.as_ref().is_some_and(|v| hop1_with_sym.contains(v))
            }
        });
    }

    dependents.sort_by(|a, b| {
        a.hops
            .cmp(&b.hops)
            .then_with(|| b.uses.len().cmp(&a.uses.len()))
            .then_with(|| a.file.cmp(&b.file))
    });

    let direct = dependents.iter().filter(|d| d.hops == 1).count();
    let transitive = dependents.iter().filter(|d| d.hops > 1).count();
    let test_files = dependents.iter().filter(|d| d.is_test).count();
    let total = dependents.len();
    let max_hops_reached = dependents.iter().map(|d| d.hops).max().unwrap_or(0);

    let stats = Stats {
        direct,
        transitive,
        test_files,
        total,
        max_hops_reached,
    };

    let mut output = ImpactOutput {
        target: Target {
            file: args.path.clone(),
            symbol: args.symbol.clone(),
            exports,
        },
        dependents,
        stats,
        truncated: false,
        warnings: Vec::new(),
    };

    if let Some(b) = args.budget {
        if apply_budget(&mut output, b) {
            output.truncated = true;
            warnings.push(format!("output truncated to {b} tokens"));
        }
    }
    output.warnings = warnings;

    serde_json::to_value(&output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn flat_exports(symbols: &[outline::Symbol]) -> Vec<Export> {
    let mut out = Vec::new();
    for s in symbols {
        out.push(Export {
            name: s.name.clone(),
            kind: s.kind.to_string(),
            line: s.start_line,
        });
        for child in &s.children {
            out.push(Export {
                name: child.name.clone(),
                kind: child.kind.to_string(),
                line: child.start_line,
            });
        }
    }
    out
}

fn load_or_build_graph(
    workspace_root: Option<&Path>,
    target: &Path,
    warnings: &mut Vec<String>,
) -> (ImportGraph, Option<u32>) {
    let root = match workspace_root {
        Some(r) => r,
        None => {
            warnings.push("no workspace root found; cannot compute dependents".to_string());
            let empty = ImportGraph::build_full(&[], |_| None);
            return (empty, None);
        }
    };

    let graph = if let Ok(g) = ImportGraph::load(&persist::index_path(root)) {
        g
    } else {
        warnings.push("index not built; building import graph on-the-fly".to_string());
        let entries = walk(root, &WalkOpts::default());
        let paths: Vec<String> = entries
            .iter()
            .map(|e| {
                e.path
                    .strip_prefix(root)
                    .unwrap_or(&e.path)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        ImportGraph::build_full(&paths, |p| std::fs::read_to_string(root.join(p)).ok())
    };

    let target_rel = relative_path(target, root);
    let target_id = target_rel
        .as_deref()
        .and_then(|r| graph.path_to_id.get(r))
        .copied();
    (graph, target_id)
}

fn compute_dependents(
    graph: &ImportGraph,
    target_id: Option<u32>,
    hops: u8,
    exports: &[Export],
    workspace_root: Option<&Path>,
) -> Vec<Dependent> {
    let Some(tid) = target_id else {
        return Vec::new();
    };

    let hop_map = reverse_bfs(graph, tid, hops);

    let hop1_files: HashSet<u32> = hop_map
        .iter()
        .filter(|&(_, &h)| h == 1)
        .map(|(&id, _)| id)
        .collect();

    let target_export_names: HashSet<String> = exports.iter().map(|e| e.name.clone()).collect();

    let mut out = Vec::new();
    for (&id, &hop) in &hop_map {
        if hop == 0 {
            continue;
        }
        let file_path = match graph.paths.get(id as usize) {
            Some(p) => p.clone(),
            None => continue,
        };
        let is_test = is_test_file(&file_path);

        let uses: Vec<String> = if hop == 1 {
            compute_uses(workspace_root, &file_path, &target_export_names)
        } else {
            Vec::new()
        };

        let via = if hop > 1 {
            graph.forward.get(id as usize).and_then(|targets| {
                targets
                    .iter()
                    .find(|t| hop1_files.contains(t))
                    .and_then(|&t| graph.paths.get(t as usize).cloned())
            })
        } else {
            None
        };

        out.push(Dependent {
            file: file_path,
            hops: hop,
            uses,
            via,
            is_test,
        });
    }
    out
}

fn compute_uses(
    workspace_root: Option<&Path>,
    rel_path: &str,
    target_export_names: &HashSet<String>,
) -> Vec<String> {
    if target_export_names.is_empty() {
        return Vec::new();
    }
    let dep_full_path = workspace_root
        .map(|r| r.join(rel_path))
        .unwrap_or_else(|| PathBuf::from(rel_path));
    let dep_ext = parsing::extension_from_path(&dep_full_path).unwrap_or("");
    let dep_content = match std::fs::read_to_string(&dep_full_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let raw_imports = imports::extract_imports(&dep_content, dep_ext);

    let mut found: Vec<String> = Vec::new();
    for imp in &raw_imports {
        let segments: Vec<&str> = imp
            .split(|c: char| {
                c == ':'
                    || c == '.'
                    || c == '/'
                    || c == '{'
                    || c == '}'
                    || c == ','
                    || c.is_whitespace()
            })
            .filter(|s| !s.is_empty())
            .collect();
        for seg in segments {
            if target_export_names.contains(seg) && !found.iter().any(|f| f == seg) {
                found.push(seg.to_string());
            }
        }
    }
    found.sort();
    found
}

fn reverse_bfs(graph: &ImportGraph, seed: u32, max_hops: u8) -> HashMap<u32, u8> {
    let mut visited: HashMap<u32, u8> = HashMap::new();
    let mut queue: VecDeque<(u32, u8)> = VecDeque::new();

    visited.insert(seed, 0);
    queue.push_back((seed, 0));

    while let Some((node, hop)) = queue.pop_front() {
        if hop >= max_hops {
            continue;
        }
        let next_hop = hop + 1;
        if let Some(neighbors) = graph.reverse.get(node as usize) {
            for &neighbor in neighbors {
                if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(neighbor) {
                    e.insert(next_hop);
                    queue.push_back((neighbor, next_hop));
                }
            }
        }
    }
    visited
}

fn find_workspace_root(target: &Path) -> Option<PathBuf> {
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

use crate::workspace::{is_test_file, relative_path};

fn current_tokens(output: &ImpactOutput) -> usize {
    let s = serde_json::to_string(output).unwrap_or_default();
    tokens::count_fast(&s)
}

fn apply_budget(output: &mut ImpactOutput, budget: usize) -> bool {
    if current_tokens(output) <= budget {
        return false;
    }
    let mut truncated = false;
    // Dependents are sorted by hops asc, uses desc, path asc.
    // Tail = highest hops, fewest uses, latest paths. Pop from tail.
    while !output.dependents.is_empty() && current_tokens(output) > budget {
        output.dependents.pop();
        truncated = true;
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn default_args(path: String) -> ImpactArgs {
        ImpactArgs {
            path,
            symbol: None,
            hops: 2,
            budget: None,
            no_tests: false,
        }
    }

    fn make_workspace(dir: &Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"impacttest\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
    }

    #[test]
    fn impact_nonexistent_path() {
        let result = run(default_args("/nonexistent_impact_test_path".to_string()));
        assert!(matches!(result, Err(AgError::FileNotFound { .. })));
    }

    #[test]
    fn impact_single_file_no_dependents() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn lonely() {}\n").unwrap();

        let result = run(default_args(lib.to_string_lossy().to_string())).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.dependents.len(), 0);
        assert_eq!(out.stats.total, 0);
        assert_eq!(out.stats.direct, 0);
        assert_eq!(out.stats.transitive, 0);
        assert!(out.target.exports.iter().any(|e| e.name == "lonely"));
    }

    #[test]
    fn impact_direct_dependent() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\npub struct Token;\n").unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::lib::authenticate;\npub fn handle() { authenticate(); }\n",
        )
        .unwrap();

        let result = run(default_args(lib.to_string_lossy().to_string())).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert_eq!(out.stats.direct, 1, "expected one direct dependent");
        assert!(
            out.dependents
                .iter()
                .any(|d| d.file.ends_with("handler.rs") && d.hops == 1),
            "handler.rs missing at hop=1: {:?}",
            out.dependents
        );
        let handler = out
            .dependents
            .iter()
            .find(|d| d.file.ends_with("handler.rs"))
            .unwrap();
        assert!(handler.uses.iter().any(|u| u == "authenticate"));
    }

    #[test]
    fn impact_transitive_dependent() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\n").unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::lib::authenticate;\npub fn handle() { authenticate(); }\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("routes.rs"),
            "use crate::handler;\npub fn route() { handler::handle(); }\n",
        )
        .unwrap();

        let result = run(default_args(lib.to_string_lossy().to_string())).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(
            out.dependents
                .iter()
                .any(|d| d.file.ends_with("handler.rs") && d.hops == 1),
            "handler.rs missing at hop=1: {:?}",
            out.dependents
        );
        assert!(
            out.dependents
                .iter()
                .any(|d| d.file.ends_with("routes.rs") && d.hops == 2),
            "routes.rs missing at hop=2: {:?}",
            out.dependents
        );
        assert_eq!(out.stats.direct, 1);
        assert_eq!(out.stats.transitive, 1);
        assert_eq!(out.stats.max_hops_reached, 2);

        let routes = out
            .dependents
            .iter()
            .find(|d| d.file.ends_with("routes.rs"))
            .unwrap();
        assert!(
            routes
                .via
                .as_ref()
                .is_some_and(|v| v.ends_with("handler.rs")),
            "routes.rs via should be handler.rs: {:?}",
            routes.via
        );
    }

    #[test]
    fn impact_symbol_filter() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\npub struct Token;\n").unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::lib::authenticate;\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("token_user.rs"), "use crate::lib::Token;\n").unwrap();

        let mut args = default_args(lib.to_string_lossy().to_string());
        args.symbol = Some("authenticate".to_string());

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(
            out.dependents
                .iter()
                .all(|d| d.uses.iter().any(|u| u == "authenticate")),
            "all dependents should use authenticate: {:?}",
            out.dependents
        );
        assert!(
            out.dependents
                .iter()
                .any(|d| d.file.ends_with("handler.rs")),
            "handler.rs should be present"
        );
        assert!(
            !out.dependents
                .iter()
                .any(|d| d.file.ends_with("token_user.rs")),
            "token_user.rs should be filtered out"
        );
        assert_eq!(out.target.exports.len(), 1);
        assert_eq!(out.target.exports[0].name, "authenticate");
    }

    #[test]
    fn impact_invalid_symbol() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\n").unwrap();

        let mut args = default_args(lib.to_string_lossy().to_string());
        args.symbol = Some("does_not_exist_anywhere".to_string());

        let result = run(args);
        assert!(
            matches!(result, Err(AgError::InvalidArgument { .. })),
            "expected InvalidArgument, got {result:?}"
        );
    }

    #[test]
    fn impact_excludes_tests() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\npub struct Token;\n").unwrap();
        std::fs::write(
            dir.path().join("handler.rs"),
            "use crate::lib::authenticate;\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("tests").join("test_auth.rs"),
            "use crate::lib::Token;\n",
        )
        .unwrap();

        let mut args = default_args(lib.to_string_lossy().to_string());
        args.no_tests = true;

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(
            out.dependents.iter().all(|d| !d.is_test),
            "no test files should remain: {:?}",
            out.dependents
        );
        assert!(
            !out.dependents
                .iter()
                .any(|d| d.file.contains("test_auth.rs")),
            "test_auth.rs should be excluded"
        );
        assert_eq!(out.stats.test_files, 0);
    }

    #[test]
    fn impact_budget_truncates() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn authenticate() {}\n").unwrap();
        for i in 0..15 {
            std::fs::write(
                dir.path().join(format!("dep_{i}.rs")),
                "use crate::lib::authenticate;\npub fn use_it() { authenticate(); }\n",
            )
            .unwrap();
        }

        let mut args = default_args(lib.to_string_lossy().to_string());
        args.budget = Some(50);

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(out.truncated, "output should be truncated");
        assert_eq!(
            out.stats.total, 15,
            "stats must reflect pre-truncation count"
        );
        assert_eq!(out.stats.direct, 15);
        assert!(
            out.dependents.len() < 15,
            "dependents should be trimmed: got {}",
            out.dependents.len()
        );
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("truncated to 50 tokens")),
            "missing truncation warning: {:?}",
            out.warnings
        );
    }

    #[test]
    fn impact_clamps_hops_above_max() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.rs");
        std::fs::write(&lib, "pub fn f() {}\n").unwrap();

        let mut args = default_args(lib.to_string_lossy().to_string());
        args.hops = 10;

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(
            out.warnings.iter().any(|w| w.contains("hops capped")),
            "missing hops cap warning: {:?}",
            out.warnings
        );
    }

    #[test]
    fn impact_directory_returns_error() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());

        let result = run(default_args(dir.path().to_string_lossy().to_string()));
        assert!(
            matches!(result, Err(AgError::InvalidArgument { .. })),
            "expected InvalidArgument for directory path, got {result:?}"
        );
    }

    #[test]
    fn impact_exports_include_children() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());
        let lib = dir.path().join("lib.py");
        std::fs::write(&lib, "class Foo:\n    def bar(self):\n        pass\n").unwrap();

        let result = run(default_args(lib.to_string_lossy().to_string())).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        assert!(
            out.target.exports.iter().any(|e| e.name == "Foo"),
            "Foo class should be in exports: {:?}",
            out.target.exports
        );
        assert!(
            out.target.exports.iter().any(|e| e.name == "bar"),
            "bar method should be in exports: {:?}",
            out.target.exports
        );
    }

    #[test]
    fn impact_test_file_patterns() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());

        // Create target file
        let target = dir.path().join("lib.py");
        std::fs::write(&target, "def helper():\n    pass\n").unwrap();

        // Create test files that import target
        std::fs::create_dir_all(dir.path().join("src").join("__tests__")).unwrap();
        std::fs::write(
            dir.path().join("src").join("__tests__").join("test.py"),
            "from lib import helper\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src").join("foo_test.py"),
            "from lib import helper\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src").join("foo.test.js"),
            "import { helper } from '../lib';\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src").join("foo.spec.ts"),
            "import { helper } from '../lib';\n",
        )
        .unwrap();

        // Create normal file that imports target
        std::fs::write(
            dir.path().join("src").join("normal.py"),
            "from lib import helper\n",
        )
        .unwrap();

        let mut args = default_args(target.to_string_lossy().to_string());
        args.no_tests = true;

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        // Only normal.py should appear (test files filtered out)
        assert!(
            out.dependents.iter().all(|d| !d.is_test),
            "no test files should remain: {:?}",
            out.dependents
        );
        assert!(
            out.dependents.iter().any(|d| d.file.ends_with("normal.py")),
            "normal.py should be present: {:?}",
            out.dependents
        );
        assert!(
            !out.dependents.iter().any(|d| d.file.contains("__tests__")),
            "__tests__ should be filtered: {:?}",
            out.dependents
        );
        assert!(
            !out.dependents.iter().any(|d| d.file.contains("_test.py")),
            "_test.py should be filtered: {:?}",
            out.dependents
        );
        assert!(
            !out.dependents.iter().any(|d| d.file.contains(".test.js")),
            ".test.js should be filtered: {:?}",
            out.dependents
        );
        assert!(
            !out.dependents.iter().any(|d| d.file.contains(".spec.ts")),
            ".spec.ts should be filtered: {:?}",
            out.dependents
        );
    }

    #[test]
    fn impact_fan_in_protection() {
        let dir = TempDir::new().unwrap();
        make_workspace(dir.path());

        // Create target file
        let target = dir.path().join("target.py");
        std::fs::write(&target, "def helper():\n    pass\n").unwrap();

        // Create 55 files that import target
        for i in 0..55 {
            std::fs::write(
                dir.path().join(format!("dep_{i}.py")),
                format!("from target import helper\ndef use_{i}():\n    helper()\n"),
            )
            .unwrap();
        }

        let mut args = default_args(target.to_string_lossy().to_string());
        args.hops = 2;

        let result = run(args).unwrap();
        let out: ImpactOutput = serde_json::from_value(result).unwrap();

        // Check for high fan-in warning
        assert!(
            out.warnings.iter().any(|w| w.contains("high fan-in")),
            "missing high fan-in warning: {:?}",
            out.warnings
        );

        // All dependents should have hops == 1 (capped)
        assert!(
            out.dependents.iter().all(|d| d.hops == 1),
            "all dependents should have hops == 1 due to fan-in cap: {:?}",
            out.dependents.iter().map(|d| d.hops).collect::<Vec<_>>()
        );
    }
}
