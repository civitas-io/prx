use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::chunking::{self, Chunk};
use crate::hash;
use crate::index::sparse::{self, SparseIndex};
use crate::output::AgError;
use crate::parsing;
use crate::walk::{self, WalkOpts};

const INDEX_DIR: &str = ".prx/index";
const META_FILE: &str = "meta.json";
const CHUNKS_FILE: &str = "chunks.bin";
const BM25_FILE: &str = "bm25.bin";

#[derive(Serialize, Deserialize)]
pub struct IndexMeta {
    pub version: String,
    pub timestamp: u64,
    pub file_count: usize,
    pub chunk_count: usize,
    pub file_hashes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializedChunk {
    pub content: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: Option<String>,
}

impl From<&Chunk> for SerializedChunk {
    fn from(c: &Chunk) -> Self {
        Self {
            content: c.content.clone(),
            file_path: c.file_path.clone(),
            start_line: c.start_line,
            end_line: c.end_line,
            language: c.language.clone(),
        }
    }
}

impl From<&SerializedChunk> for Chunk {
    fn from(s: &SerializedChunk) -> Self {
        Self {
            content: s.content.clone(),
            file_path: s.file_path.clone(),
            start_line: s.start_line,
            end_line: s.end_line,
            start_byte: 0,
            end_byte: s.content.len(),
            language: s.language.clone(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializedBm25 {
    pub enriched_texts: Vec<String>,
}

pub struct IndexStats {
    pub files: usize,
    pub chunks: usize,
    pub languages: HashMap<String, usize>,
}

pub fn build_and_save(root: &Path) -> Result<IndexStats, AgError> {
    let entries = walk::walk(root, &WalkOpts::default());
    let mut all_chunks = Vec::new();
    let mut file_hashes = HashMap::new();
    let mut lang_counts: HashMap<String, usize> = HashMap::new();

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string();

        file_hashes.insert(rel_path.clone(), hash::hash_bytes(content.as_bytes()));

        let ext = parsing::extension_from_path(&entry.path);
        if let Some(lang) = ext.and_then(parsing::languages::language_name_for_extension) {
            *lang_counts.entry(lang.to_string()).or_insert(0) += 1;
        }

        let chunks = chunking::chunk_file(&content, &rel_path, ext);
        all_chunks.extend(chunks);
    }

    let serialized_chunks: Vec<SerializedChunk> =
        all_chunks.iter().map(SerializedChunk::from).collect();

    let enriched_texts: Vec<String> = all_chunks
        .iter()
        .map(|c| sparse::enrich_for_bm25(&c.content, &c.file_path))
        .collect();

    let meta = IndexMeta {
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        file_count: entries.len(),
        chunk_count: all_chunks.len(),
        file_hashes,
    };

    let index_dir = root.join(INDEX_DIR);
    std::fs::create_dir_all(&index_dir).map_err(AgError::Io)?;

    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| AgError::Internal {
        message: format!("serialize meta: {e}"),
    })?;
    std::fs::write(index_dir.join(META_FILE), meta_json).map_err(AgError::Io)?;

    let chunks_bin = bincode::serialize(&serialized_chunks).map_err(|e| AgError::Internal {
        message: format!("serialize chunks: {e}"),
    })?;
    std::fs::write(index_dir.join(CHUNKS_FILE), chunks_bin).map_err(AgError::Io)?;

    let bm25_data = SerializedBm25 { enriched_texts };
    let bm25_bin = bincode::serialize(&bm25_data).map_err(|e| AgError::Internal {
        message: format!("serialize bm25: {e}"),
    })?;
    std::fs::write(index_dir.join(BM25_FILE), bm25_bin).map_err(AgError::Io)?;

    let file_paths: Vec<String> = {
        let mut paths: Vec<String> = all_chunks.iter().map(|c| c.file_path.clone()).collect();
        paths.sort();
        paths.dedup();
        paths
    };
    let import_graph = crate::search::graph::ImportGraph::build_full(&file_paths, |path| {
        std::fs::read_to_string(root.join(path)).ok()
    });
    let _ = import_graph.save(&index_dir);

    Ok(IndexStats {
        files: entries.len(),
        chunks: all_chunks.len(),
        languages: lang_counts,
    })
}

pub fn load(root: &Path) -> Result<(Vec<Chunk>, SparseIndex), AgError> {
    let index_dir = root.join(INDEX_DIR);

    let meta_str = std::fs::read_to_string(index_dir.join(META_FILE)).map_err(|_| {
        AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: "meta.json not found".to_string(),
        }
    })?;
    let meta: IndexMeta = serde_json::from_str(&meta_str).map_err(|e| AgError::IndexCorrupted {
        path: index_dir.to_string_lossy().to_string(),
        reason: format!("invalid meta.json: {e}"),
    })?;

    if meta.version != env!("CARGO_PKG_VERSION") {
        return Err(AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: format!(
                "version mismatch: index={}, binary={}",
                meta.version,
                env!("CARGO_PKG_VERSION")
            ),
        });
    }

    let chunks_bin =
        std::fs::read(index_dir.join(CHUNKS_FILE)).map_err(|_| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: "chunks.bin not found".to_string(),
        })?;
    let serialized_chunks: Vec<SerializedChunk> =
        bincode::deserialize(&chunks_bin).map_err(|e| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: format!("invalid chunks.bin: {e}"),
        })?;

    let bm25_bin =
        std::fs::read(index_dir.join(BM25_FILE)).map_err(|_| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: "bm25.bin not found".to_string(),
        })?;
    let bm25_data: SerializedBm25 =
        bincode::deserialize(&bm25_bin).map_err(|e| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: format!("invalid bm25.bin: {e}"),
        })?;

    let chunks: Vec<Chunk> = serialized_chunks.iter().map(Chunk::from).collect();
    let bm25_index = SparseIndex::build(&bm25_data.enriched_texts);

    Ok((chunks, bm25_index))
}

pub fn is_valid(root: &Path) -> bool {
    let index_dir = root.join(INDEX_DIR);
    let meta_path = index_dir.join(META_FILE);

    let meta_str = match std::fs::read_to_string(&meta_path) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let meta: IndexMeta = match serde_json::from_str(&meta_str) {
        Ok(m) => m,
        Err(_) => return false,
    };

    if meta.version != env!("CARGO_PKG_VERSION") {
        return false;
    }

    for (rel_path, expected_hash) in &meta.file_hashes {
        let full_path = root.join(rel_path);
        match std::fs::read(&full_path) {
            Ok(content) => {
                if hash::hash_bytes(&content) != *expected_hash {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }

    true
}

pub fn index_path(root: &Path) -> PathBuf {
    root.join(INDEX_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("lib.py"),
            "def greet(name):\n    print(f\"Hello {name}\")\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn build_and_load_roundtrip() {
        let dir = make_test_dir();
        let stats = build_and_save(dir.path()).unwrap();
        assert!(stats.files >= 2);
        assert!(stats.chunks >= 2);

        let (chunks, bm25) = load(dir.path()).unwrap();
        assert_eq!(chunks.len(), stats.chunks);

        let results = bm25.query("main", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn is_valid_after_build() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();
        assert!(is_valid(dir.path()));
    }

    #[test]
    fn is_invalid_after_file_change() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        std::fs::write(dir.path().join("main.rs"), "fn main() { changed() }").unwrap();
        assert!(!is_valid(dir.path()));
    }

    #[test]
    fn is_invalid_when_no_index() {
        let dir = TempDir::new().unwrap();
        assert!(!is_valid(dir.path()));
    }

    #[test]
    fn stats_has_languages() {
        let dir = make_test_dir();
        let stats = build_and_save(dir.path()).unwrap();
        assert!(stats.languages.contains_key("rust"));
        assert!(stats.languages.contains_key("python"));
    }
}
