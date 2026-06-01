use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::chunking;
use crate::index::dense::DenseIndex;
use crate::output::{AgError, to_json};
use crate::parsing;
use crate::walk::{self, WalkOpts};

#[derive(Args)]
pub struct FindArgs {
    /// Root path
    #[arg(default_value = ".")]
    pub path: String,

    /// Filter by glob pattern
    #[arg(long)]
    pub pattern: Option<String>,

    /// Maximum directory depth
    #[arg(long)]
    pub depth: Option<usize>,

    /// Semantic relevance scoring
    #[arg(long)]
    pub related_to: Option<String>,

    /// Files modified since git ref or timestamp
    #[arg(long)]
    pub changed_since: Option<String>,

    /// Include per-file symbol counts
    #[arg(long)]
    pub outline: bool,

    /// Tree output only
    #[arg(long)]
    pub tree: bool,

    /// Flat list only
    #[arg(long)]
    pub flat: bool,

    /// Token budget
    #[arg(long)]
    pub budget: Option<usize>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct FindOutput {
    pub tree: Option<serde_json::Value>,
    pub flat: Option<Vec<FileEntry>>,
    pub stats: FindStats,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct FileEntry {
    pub path: String,
    pub lines: usize,
    pub bytes: usize,
    pub language: Option<String>,
    pub symbols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance: Option<f32>,
}

#[derive(Serialize, serde::Deserialize, Debug)]
pub struct FindStats {
    pub total_files: usize,
    pub returned: usize,
    pub budget_used: usize,
}

pub fn run(args: FindArgs) -> Result<serde_json::Value, AgError> {
    let root = Path::new(&args.path);
    if !root.exists() {
        return Err(AgError::FileNotFound {
            path: args.path.clone(),
        });
    }

    let changed_files = args
        .changed_since
        .as_deref()
        .and_then(|ref_str| get_changed_files(root, ref_str));

    let entries = walk::walk(root, &WalkOpts::default());
    let mut file_entries: Vec<FileEntry> = Vec::new();

    for entry in &entries {
        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .replace('\\', "/");

        if let Some(ref pattern) = args.pattern {
            if !glob_matches(pattern, &rel_path) {
                continue;
            }
        }

        if let Some(max_depth) = args.depth {
            let depth = rel_path.matches('/').count();
            if depth >= max_depth {
                continue;
            }
        }

        if let Some(ref changed) = changed_files {
            if !changed
                .iter()
                .any(|c| rel_path.ends_with(c) || c.ends_with(&rel_path))
            {
                continue;
            }
        }

        let line_count = std::fs::read_to_string(&entry.path)
            .map(|c| c.lines().count())
            .unwrap_or(0);

        let ext = parsing::extension_from_path(&entry.path);
        let language = ext
            .and_then(parsing::languages::language_name_for_extension)
            .map(String::from);

        let symbols = if args.outline {
            ext.and_then(|e| {
                std::fs::read_to_string(&entry.path)
                    .ok()
                    .map(|content| parsing::outline::extract_symbols(&content, e).len())
            })
        } else {
            None
        };

        file_entries.push(FileEntry {
            path: rel_path,
            lines: line_count,
            bytes: entry.size as usize,
            language,
            symbols,
            relevance: None,
        });
    }

    if let Some(ref query) = args.related_to {
        score_file_relevance(&mut file_entries, root, query);
        file_entries.sort_by(|a, b| {
            b.relevance
                .unwrap_or(0.0)
                .partial_cmp(&a.relevance.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let total_files = file_entries.len();

    if let Some(budget) = args.budget {
        let mut used = 0;
        file_entries.retain(|e| {
            let cost = e.path.len() / 4 + 2;
            if used + cost <= budget {
                used += cost;
                true
            } else {
                false
            }
        });
    }

    let returned = file_entries.len();
    let budget_used = file_entries.iter().map(|e| e.path.len() / 4 + 2).sum();

    let stats = FindStats {
        total_files,
        returned,
        budget_used,
    };

    let tree_val = if !args.flat {
        Some(build_tree(&file_entries))
    } else {
        None
    };

    let flat_val = if !args.tree { Some(file_entries) } else { None };

    let output = FindOutput {
        tree: tree_val,
        flat: flat_val,
        stats,
    };

    to_json(output)
}

fn score_file_relevance(entries: &mut [FileEntry], root: &Path, query: &str) {
    let model_bytes: &[u8] = include_bytes!(concat!(
        env!("PRX_MODELS_PATH"),
        "/potion-retrieval-32M.safetensors"
    ));
    if model_bytes.is_empty() {
        return;
    }

    let tensors = match safetensors::SafeTensors::deserialize(model_bytes) {
        Ok(t) => t,
        Err(_) => return,
    };

    let embedding_tensor = tensors
        .tensor("embeddings")
        .or_else(|_| tensors.tensor("model.embeddings"))
        .or_else(|_| {
            tensors
                .names()
                .into_iter()
                .find(|n| n.contains("embed"))
                .ok_or(safetensors::SafeTensorError::InvalidOffset(
                    "no embedding".into(),
                ))
                .and_then(|name| tensors.tensor(name))
        });

    let embedding_tensor = match embedding_tensor {
        Ok(t) => t,
        Err(_) => return,
    };

    let shape = embedding_tensor.shape();
    if shape.len() != 2 {
        return;
    }
    let (vocab_size, dim) = (shape[0], shape[1]);

    let data = embedding_tensor.data();
    let weights = match embedding_tensor.dtype() {
        safetensors::Dtype::F32 => {
            let floats: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            ndarray::Array2::from_shape_vec((vocab_size, dim), floats).ok()
        }
        safetensors::Dtype::F16 => {
            let floats: Vec<f32> = data
                .chunks_exact(2)
                .map(|c| half::f16::from_le_bytes([c[0], c[1]]).to_f32())
                .collect();
            ndarray::Array2::from_shape_vec((vocab_size, dim), floats).ok()
        }
        _ => None,
    };

    let weights = match weights {
        Some(w) => w,
        None => return,
    };

    let vocab = load_model2vec_vocab(vocab_size).unwrap_or_default();
    if vocab.is_empty() {
        return;
    }

    let index = DenseIndex::new(vocab, weights);

    for entry in entries.iter_mut() {
        let file_path = root.join(&entry.path);
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let ext = parsing::extension_from_path(&file_path);
        let chunks = chunking::chunk_file(&content, &entry.path, ext);
        if chunks.is_empty() {
            continue;
        }

        let chunk_refs: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let mut file_index = DenseIndex::new(index.vocab().clone(), index.weights().clone());
        file_index.index_chunks(&chunk_refs);

        let results = file_index.search(query, 1);
        if let Some((_, score)) = results.first() {
            entry.relevance = Some(*score);
        }
    }
}

fn load_model2vec_vocab(_expected_size: usize) -> Option<HashMap<String, usize>> {
    let tokenizer_bytes: &[u8] = include_bytes!(concat!(
        env!("PRX_MODELS_PATH"),
        "/model2vec_tokenizer.json"
    ));
    if tokenizer_bytes.is_empty() {
        return None;
    }
    let tokenizer_json: serde_json::Value = serde_json::from_slice(tokenizer_bytes).ok()?;
    let vocab_obj = tokenizer_json.get("model")?.get("vocab")?.as_object()?;
    let mut vocab = HashMap::with_capacity(vocab_obj.len());
    for (token, id_val) in vocab_obj {
        if let Some(id) = id_val.as_u64() {
            vocab.insert(token.clone(), id as usize);
        }
    }
    Some(vocab)
}

fn get_changed_files(root: &Path, git_ref: &str) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", &format!("{git_ref}..HEAD")])
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(
        stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
    )
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    if let Some(ext_pattern) = pattern.strip_prefix("*.") {
        return file_name.ends_with(&format!(".{ext_pattern}"));
    }
    file_name.contains(pattern)
}

enum TreeNode {
    File(FileInfo),
    Dir(BTreeMap<String, TreeNode>),
}

struct FileInfo {
    lines: usize,
    bytes: usize,
    language: Option<String>,
    symbols: Option<usize>,
}

impl TreeNode {
    fn to_json(&self) -> serde_json::Value {
        match self {
            TreeNode::File(info) => {
                let mut map = serde_json::Map::new();
                map.insert("lines".to_string(), serde_json::json!(info.lines));
                map.insert("bytes".to_string(), serde_json::json!(info.bytes));
                if let Some(ref lang) = info.language {
                    map.insert("language".to_string(), serde_json::json!(lang));
                }
                if let Some(syms) = info.symbols {
                    map.insert("symbols".to_string(), serde_json::json!(syms));
                }
                serde_json::Value::Object(map)
            }
            TreeNode::Dir(children) => {
                let map: serde_json::Map<String, serde_json::Value> = children
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect();
                serde_json::Value::Object(map)
            }
        }
    }
}

fn build_tree(entries: &[FileEntry]) -> serde_json::Value {
    let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
    for entry in entries {
        let parts: Vec<&str> = entry.path.split('/').collect();
        insert_into_tree(&mut root, &parts, entry);
    }
    TreeNode::Dir(root).to_json()
}

fn insert_into_tree(tree: &mut BTreeMap<String, TreeNode>, parts: &[&str], entry: &FileEntry) {
    if parts.len() == 1 {
        tree.insert(
            parts[0].to_string(),
            TreeNode::File(FileInfo {
                lines: entry.lines,
                bytes: entry.bytes,
                language: entry.language.clone(),
                symbols: entry.symbols,
            }),
        );
        return;
    }

    let dir = parts[0];
    let subtree = tree
        .entry(format!("{dir}/"))
        .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));

    if let TreeNode::Dir(children) = subtree {
        insert_into_tree(children, &parts[1..], entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::write(
            dir.path().join("src/main.rs"),
            "fn main() {}\nfn helper() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/lib.py"),
            "def foo(): pass\ndef bar(): pass\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("tests/test.rs"), "fn test_it() {}\n").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Hello\n").unwrap();
        dir
    }

    fn find_args(path: &str) -> FindArgs {
        FindArgs {
            path: path.to_string(),
            pattern: None,
            depth: None,
            related_to: None,
            changed_since: None,
            outline: false,
            tree: false,
            flat: false,
            budget: None,
        }
    }

    #[test]
    fn finds_all_files() {
        let dir = make_test_dir();
        let result = run(find_args(dir.path().to_str().unwrap())).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        assert_eq!(out.stats.total_files, 4);
        assert!(out.flat.is_some());
        assert!(out.tree.is_some());
    }

    #[test]
    fn pattern_filters() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.pattern = Some("*.rs".to_string());
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        let flat = out.flat.unwrap();
        assert_eq!(flat.len(), 2);
        for f in &flat {
            assert!(f.path.ends_with(".rs"));
        }
    }

    #[test]
    fn depth_filters() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.depth = Some(1);
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        let flat = out.flat.unwrap();
        for f in &flat {
            assert!(
                !f.path.contains('/'),
                "depth=1 should exclude nested: {}",
                f.path
            );
        }
    }

    #[test]
    fn tree_only() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.tree = true;
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        assert!(out.tree.is_some());
        assert!(out.flat.is_none());
    }

    #[test]
    fn flat_only() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.flat = true;
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        assert!(out.flat.is_some());
        assert!(out.tree.is_none());
    }

    #[test]
    fn entries_have_language() {
        let dir = make_test_dir();
        let result = run(find_args(dir.path().to_str().unwrap())).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        let flat = out.flat.unwrap();
        let rs = flat.iter().find(|f| f.path.ends_with(".rs")).unwrap();
        assert_eq!(rs.language.as_deref(), Some("rust"));
    }

    #[test]
    fn budget_limits() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.budget = Some(5);
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        assert!(out.stats.returned < out.stats.total_files);
    }

    #[test]
    fn outline_includes_symbol_count() {
        let dir = make_test_dir();
        let mut args = find_args(dir.path().to_str().unwrap());
        args.outline = true;
        let result = run(args).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        let flat = out.flat.unwrap();
        let rs = flat
            .iter()
            .find(|f| f.path.replace('\\', "/") == "src/main.rs")
            .expect("should find src/main.rs");
        assert!(rs.symbols.is_some());
        assert!(rs.symbols.unwrap() >= 2);
    }

    #[test]
    fn nonexistent_path_errors() {
        let err = run(find_args("/nonexistent/zzz")).unwrap_err();
        assert!(matches!(err, AgError::FileNotFound { .. }));
    }

    #[test]
    fn tree_has_directory_structure() {
        let dir = make_test_dir();
        let result = run(find_args(dir.path().to_str().unwrap())).unwrap();
        let out: FindOutput = serde_json::from_value(result).unwrap();
        let tree = out.tree.unwrap();
        let has_src = tree.get("src/").is_some() || tree.get("src\\").is_some();
        assert!(has_src, "tree should have src dir: {tree}");
    }

    #[test]
    fn glob_matches_extension() {
        assert!(glob_matches("*.rs", "main.rs"));
        assert!(glob_matches("*.rs", "src/lib.rs"));
        assert!(!glob_matches("*.rs", "lib.py"));
    }
}
