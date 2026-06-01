use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::chunking::{self, Chunk};
use crate::hash;
use crate::index::sparse::{self, SparseIndex};
use crate::output::AgError;
use crate::parsing;
use crate::walk::{self, WalkOpts};

/// Memory-mapped embeddings file. Zero-copy view into the OS page cache,
/// keeps the index warm across repeated `prx search` invocations.
pub struct MmapEmbeddings {
    mmap: memmap2::Mmap,
    n_chunks: usize,
    dim: usize,
}

impl MmapEmbeddings {
    /// Open and validate a memory-mapped embeddings file.
    ///
    /// The caller must ensure the file is not modified while this struct exists.
    /// prx only writes embeddings.bin during `prx index`, which is exclusive,
    /// and the file is read-only thereafter.
    pub fn open(path: &Path, n_chunks: usize, dim: usize) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        // SAFETY: Index files are read-only after creation. `prx index` is the sole
        // writer and completes before any reader accesses the file. Concurrent
        // `prx search` invocations only read.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        let expected = n_chunks
            .checked_mul(dim)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "embeddings.bin: size overflow in n_chunks * dim * 4",
                )
            })?;
        if mmap.len() != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "embeddings.bin: expected {expected} bytes, got {}",
                    mmap.len()
                ),
            ));
        }
        // Validate alignment and castability up front so `view()` cannot panic.
        // bytemuck::try_cast_slice returns Err if alignment is wrong.
        bytemuck::try_cast_slice::<u8, f32>(&mmap).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("embeddings.bin: cast failed: {e}"),
            )
        })?;
        Ok(Self {
            mmap,
            n_chunks,
            dim,
        })
    }

    /// Zero-copy 2D view over the mmap'd region.
    pub fn view(&self) -> ndarray::ArrayView2<'_, f32> {
        let floats: &[f32] = bytemuck::cast_slice(&self.mmap);
        ndarray::ArrayView2::from_shape((self.n_chunks, self.dim), floats)
            .expect("shape validated at open()")
    }
}

/// Embeddings backing storage: memory-mapped (preferred) or owned in-memory.
pub enum Embeddings {
    Mmap(MmapEmbeddings),
    Owned(ndarray::Array2<f32>),
}

impl Embeddings {
    /// Zero-copy 2D view over the embeddings.
    pub fn view(&self) -> ndarray::ArrayView2<'_, f32> {
        match self {
            Self::Mmap(m) => m.view(),
            Self::Owned(a) => a.view(),
        }
    }

    /// Number of embedding rows (chunks).
    pub fn nrows(&self) -> usize {
        match self {
            Self::Mmap(m) => m.n_chunks,
            Self::Owned(a) => a.nrows(),
        }
    }
}

const INDEX_DIR: &str = ".prx/index";
const META_FILE: &str = "meta.json";
const CHUNKS_FILE: &str = "chunks.bin";
const BM25_FILE: &str = "bm25.bin";
const EMBEDDINGS_FILE: &str = "embeddings.bin";
const EMBEDDING_HASHES_FILE: &str = "embedding_hashes.bin";

#[derive(Serialize, Deserialize)]
pub struct IndexMeta {
    pub version: String,
    pub timestamp: u64,
    pub file_count: usize,
    pub chunk_count: usize,
    pub file_hashes: HashMap<String, String>,
    #[serde(default)]
    pub embeddings_dim: usize,
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
    pub files_changed: usize,
    pub files_unchanged: usize,
    pub warnings: Vec<String>,
}

fn load_existing_index(root: &Path) -> Option<(IndexMeta, Vec<SerializedChunk>)> {
    let index_dir = root.join(INDEX_DIR);
    let meta_str = std::fs::read_to_string(index_dir.join(META_FILE)).ok()?;
    let meta: IndexMeta = serde_json::from_str(&meta_str).ok()?;

    if meta.version != env!("CARGO_PKG_VERSION") {
        return None;
    }

    let chunks_bin = std::fs::read(index_dir.join(CHUNKS_FILE)).ok()?;
    let chunks: Vec<SerializedChunk> = postcard::from_bytes(&chunks_bin).ok()?;
    Some((meta, chunks))
}

pub fn build_and_save(root: &Path) -> Result<IndexStats, AgError> {
    let entries = walk::walk(root, &WalkOpts::default());

    let existing = load_existing_index(root);

    struct FileResult {
        rel_path: String,
        hash: String,
        chunks: Vec<Chunk>,
        language: Option<String>,
        was_changed: bool,
    }

    let existing_hashes: HashMap<&str, &str> = existing
        .as_ref()
        .map(|(meta, _)| {
            meta.file_hashes
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let existing_chunks: Option<&Vec<SerializedChunk>> =
        existing.as_ref().map(|(_, chunks)| chunks);

    let file_results: Vec<FileResult> = entries
        .par_iter()
        .filter_map(|entry| {
            let content = std::fs::read_to_string(&entry.path).ok()?;
            let rel_path = entry
                .path
                .strip_prefix(root)
                .unwrap_or(&entry.path)
                .to_string_lossy()
                .to_string();
            let current_hash = hash::hash_bytes(content.as_bytes());
            let ext = parsing::extension_from_path(&entry.path);
            let language = ext
                .and_then(parsing::languages::language_name_for_extension)
                .map(|s| s.to_string());

            let reuse = existing_hashes.get(rel_path.as_str()).and_then(|old_hash| {
                if *old_hash == current_hash.as_str() {
                    existing_chunks.and_then(|old| {
                        let reused: Vec<SerializedChunk> = old
                            .iter()
                            .filter(|c| c.file_path == rel_path)
                            .cloned()
                            .collect();
                        if reused.is_empty() {
                            None
                        } else {
                            Some(reused)
                        }
                    })
                } else {
                    None
                }
            });

            let (chunks, was_changed) = if let Some(reused) = reuse {
                (reused.iter().map(Chunk::from).collect(), false)
            } else {
                (chunking::chunk_file(&content, &rel_path, ext), true)
            };

            Some(FileResult {
                rel_path,
                hash: current_hash,
                chunks,
                language,
                was_changed,
            })
        })
        .collect();

    let mut all_chunks: Vec<Chunk> = Vec::new();
    let mut file_hashes: HashMap<String, String> = HashMap::new();
    let mut lang_counts: HashMap<String, usize> = HashMap::new();
    let mut files_changed: usize = 0;
    let mut files_unchanged: usize = 0;

    for result in file_results {
        if let Some(lang) = result.language {
            *lang_counts.entry(lang).or_insert(0) += 1;
        }
        file_hashes.insert(result.rel_path, result.hash);
        if result.was_changed {
            files_changed += 1;
        } else {
            files_unchanged += 1;
        }
        all_chunks.extend(result.chunks);
    }

    let serialized_chunks: Vec<SerializedChunk> =
        all_chunks.par_iter().map(SerializedChunk::from).collect();

    let enriched_texts: Vec<String> = all_chunks
        .par_iter()
        .map(|c| sparse::enrich_for_bm25(&c.content, &c.file_path))
        .collect();

    let mut warnings: Vec<String> = Vec::new();
    let (embeddings_dim, emb_warning) =
        compute_and_save_embeddings(&enriched_texts, &root.join(INDEX_DIR));
    if let Some(w) = emb_warning {
        warnings.push(w);
    }

    let meta = IndexMeta {
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        file_count: entries.len(),
        chunk_count: all_chunks.len(),
        file_hashes,
        embeddings_dim,
    };

    let index_dir = root.join(INDEX_DIR);
    std::fs::create_dir_all(&index_dir).map_err(AgError::Io)?;

    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| AgError::Internal {
        message: format!("serialize meta: {e}"),
    })?;
    std::fs::write(index_dir.join(META_FILE), meta_json).map_err(AgError::Io)?;

    let chunks_bin = postcard::to_allocvec(&serialized_chunks).map_err(|e| AgError::Internal {
        message: format!("serialize chunks: {e}"),
    })?;
    std::fs::write(index_dir.join(CHUNKS_FILE), chunks_bin).map_err(AgError::Io)?;

    let bm25_data = SerializedBm25 { enriched_texts };
    let bm25_bin = postcard::to_allocvec(&bm25_data).map_err(|e| AgError::Internal {
        message: format!("serialize bm25: {e}"),
    })?;
    std::fs::write(index_dir.join(BM25_FILE), bm25_bin).map_err(AgError::Io)?;

    let file_paths: Vec<String> = {
        let mut paths: Vec<String> = all_chunks.iter().map(|c| c.file_path.clone()).collect();
        paths.sort();
        paths.dedup();
        paths
    };

    let chunk_texts: Vec<String> = all_chunks.iter().map(|c| c.content.clone()).collect();

    rayon::join(
        || {
            let g = crate::search::graph::ImportGraph::build_full(&file_paths, |path| {
                std::fs::read_to_string(root.join(path)).ok()
            });
            let _ = g.save(&index_dir);
        },
        || {
            let s = crate::search::symbols::SymbolIndex::build(
                &file_paths,
                |path| std::fs::read_to_string(root.join(path)).ok(),
                &chunk_texts,
            );
            let _ = s.save(&index_dir);
        },
    );

    Ok(IndexStats {
        files: entries.len(),
        chunks: all_chunks.len(),
        languages: lang_counts,
        files_changed,
        files_unchanged,
        warnings,
    })
}

fn compute_and_save_embeddings(
    enriched_texts: &[String],
    index_dir: &Path,
) -> (usize, Option<String>) {
    let Some(model) = crate::index::dense::load_model() else {
        return (
            0,
            Some(
                "embedding model failed to load; search will use BM25 only (no semantic search)"
                    .to_string(),
            ),
        );
    };

    let dim = model.dim();
    let current_hashes: Vec<String> = enriched_texts
        .iter()
        .map(|t| hash::hash_bytes(t.as_bytes()))
        .collect();

    let (old_hashes, old_embeddings) = load_embedding_cache(index_dir, dim);
    let old_lookup: HashMap<&str, usize> = old_hashes
        .iter()
        .enumerate()
        .map(|(i, h)| (h.as_str(), i))
        .collect();

    let _ = model.embed_text("warmup");

    let embeddings: Vec<(ndarray::Array1<f32>, bool)> = current_hashes
        .par_iter()
        .enumerate()
        .map(|(i, h)| {
            if let Some(&old_idx) = old_lookup.get(h.as_str()) {
                if let Some(old_emb) = old_embeddings.as_ref() {
                    if old_idx < old_emb.nrows() {
                        return (old_emb.row(old_idx).to_owned(), false);
                    }
                }
            }
            (model.embed_text(&enriched_texts[i]), true)
        })
        .collect();

    let mut result = ndarray::Array2::zeros((enriched_texts.len(), dim));
    let mut embedded_count = 0usize;
    for (i, (emb, was_computed)) in embeddings.into_iter().enumerate() {
        result.row_mut(i).assign(&emb);
        if was_computed {
            embedded_count += 1;
        }
    }

    let _ = std::fs::create_dir_all(index_dir);
    let raw: Vec<u8> = result.iter().flat_map(|f| f.to_le_bytes()).collect();
    let _ = std::fs::write(index_dir.join(EMBEDDINGS_FILE), raw);

    let hashes_bin = postcard::to_allocvec(&current_hashes).unwrap_or_default();
    let _ = std::fs::write(index_dir.join(EMBEDDING_HASHES_FILE), hashes_bin);

    if embedded_count < enriched_texts.len() {
        eprintln!(
            "embeddings: {embedded_count}/{} recomputed, {} reused",
            enriched_texts.len(),
            enriched_texts.len() - embedded_count
        );
    }

    (dim, None)
}

fn load_embedding_cache(
    index_dir: &Path,
    dim: usize,
) -> (Vec<String>, Option<ndarray::Array2<f32>>) {
    let hashes = std::fs::read(index_dir.join(EMBEDDING_HASHES_FILE))
        .ok()
        .and_then(|bytes| postcard::from_bytes::<Vec<String>>(&bytes).ok())
        .unwrap_or_default();

    let embeddings = std::fs::read(index_dir.join(EMBEDDINGS_FILE))
        .ok()
        .and_then(|bytes| {
            if dim == 0 || bytes.len() % (dim * 4) != 0 {
                return None;
            }
            let n = bytes.len() / (dim * 4);
            let floats: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            ndarray::Array2::from_shape_vec((n, dim), floats).ok()
        });

    (hashes, embeddings)
}

pub fn load_embeddings(root: &Path) -> Option<Embeddings> {
    let index_dir = root.join(INDEX_DIR);
    let meta_str = std::fs::read_to_string(index_dir.join(META_FILE)).ok()?;
    let meta: IndexMeta = serde_json::from_str(&meta_str).ok()?;

    if meta.embeddings_dim == 0 || meta.chunk_count == 0 {
        return None;
    }

    let emb_path = index_dir.join(EMBEDDINGS_FILE);

    match MmapEmbeddings::open(&emb_path, meta.chunk_count, meta.embeddings_dim) {
        Ok(m) => Some(Embeddings::Mmap(m)),
        Err(_) => {
            let raw = std::fs::read(&emb_path).ok()?;
            let expected = meta.chunk_count * meta.embeddings_dim * 4;
            if raw.len() != expected {
                return None;
            }
            let floats: Vec<f32> = raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let arr =
                ndarray::Array2::from_shape_vec((meta.chunk_count, meta.embeddings_dim), floats)
                    .ok()?;
            Some(Embeddings::Owned(arr))
        }
    }
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
        postcard::from_bytes(&chunks_bin).map_err(|e| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: format!("invalid chunks.bin: {e}"),
        })?;

    let bm25_bin =
        std::fs::read(index_dir.join(BM25_FILE)).map_err(|_| AgError::IndexCorrupted {
            path: index_dir.to_string_lossy().to_string(),
            reason: "bm25.bin not found".to_string(),
        })?;
    let bm25_data: SerializedBm25 =
        postcard::from_bytes(&bm25_bin).map_err(|e| AgError::IndexCorrupted {
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

    let current_files = walk::walk(root, &WalkOpts::default());
    if current_files.len() != meta.file_hashes.len() {
        return false;
    }

    for entry in &current_files {
        let rel = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string();
        if !meta.file_hashes.contains_key(&rel) {
            return false;
        }
    }

    true
}

pub fn load_symbols(root: &Path) -> Option<crate::search::symbols::SymbolIndex> {
    let index_dir = root.join(INDEX_DIR);
    crate::search::symbols::SymbolIndex::load(&index_dir).ok()
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

    #[test]
    fn incremental_skips_unchanged_files() {
        let dir = make_test_dir();
        let stats1 = build_and_save(dir.path()).unwrap();
        assert!(stats1.files_changed >= 2);
        assert_eq!(stats1.files_unchanged, 0);

        let stats2 = build_and_save(dir.path()).unwrap();
        assert_eq!(stats2.files_unchanged, stats1.files);
        assert_eq!(stats2.files_changed, 0);
        assert_eq!(stats2.chunks, stats1.chunks);
    }

    #[test]
    fn incremental_rechunks_changed_file() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"changed\");\n}\nfn extra() {}\n",
        )
        .unwrap();

        let stats = build_and_save(dir.path()).unwrap();
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.files_unchanged, 1);
    }

    #[test]
    fn incremental_handles_new_file() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        std::fs::write(dir.path().join("new.rs"), "fn new_fn() {}\n").unwrap();

        let stats = build_and_save(dir.path()).unwrap();
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.files_unchanged, 2);
        assert_eq!(stats.files, 3);
    }

    #[test]
    fn incremental_handles_deleted_file() {
        let dir = make_test_dir();
        let stats1 = build_and_save(dir.path()).unwrap();

        std::fs::remove_file(dir.path().join("lib.py")).unwrap();

        let stats2 = build_and_save(dir.path()).unwrap();
        assert_eq!(stats2.files, stats1.files - 1);
        assert!(stats2.chunks < stats1.chunks);
    }

    #[test]
    fn incremental_search_works_after_update() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        std::fs::write(
            dir.path().join("main.rs"),
            "fn unique_searchable_term() {}\n",
        )
        .unwrap();

        build_and_save(dir.path()).unwrap();
        let (chunks, bm25) = load(dir.path()).unwrap();

        let has_new_content = chunks
            .iter()
            .any(|c| c.content.contains("unique_searchable_term"));
        assert!(has_new_content);

        let results = bm25.query("unique_searchable_term", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn is_invalid_after_new_file_added() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();
        assert!(is_valid(dir.path()));

        std::fs::write(dir.path().join("new.rs"), "fn new_fn() {}\n").unwrap();
        assert!(!is_valid(dir.path()));
    }

    #[test]
    fn is_invalid_after_file_deleted() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();
        assert!(is_valid(dir.path()));

        std::fs::remove_file(dir.path().join("lib.py")).unwrap();
        assert!(!is_valid(dir.path()));
    }

    #[test]
    fn is_invalid_after_file_swapped() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();
        assert!(is_valid(dir.path()));

        std::fs::remove_file(dir.path().join("lib.py")).unwrap();
        std::fs::write(dir.path().join("other.rs"), "fn other() {}\n").unwrap();
        assert!(!is_valid(dir.path()));
    }

    #[test]
    fn incremental_embeddings_reuse_cache() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        let index_dir = dir.path().join(".prx").join("index");
        let hashes_before: Vec<String> =
            postcard::from_bytes(&std::fs::read(index_dir.join("embedding_hashes.bin")).unwrap())
                .unwrap();
        let emb_before = std::fs::read(index_dir.join("embeddings.bin")).unwrap();

        assert!(!hashes_before.is_empty());
        assert!(!emb_before.is_empty());

        build_and_save(dir.path()).unwrap();

        let hashes_after: Vec<String> =
            postcard::from_bytes(&std::fs::read(index_dir.join("embedding_hashes.bin")).unwrap())
                .unwrap();
        let emb_after = std::fs::read(index_dir.join("embeddings.bin")).unwrap();

        assert_eq!(hashes_before, hashes_after);
        assert_eq!(emb_before, emb_after);
    }

    #[test]
    fn incremental_embeddings_update_on_change() {
        let dir = make_test_dir();
        build_and_save(dir.path()).unwrap();

        let index_dir = dir.path().join(".prx").join("index");
        let hashes_before: Vec<String> =
            postcard::from_bytes(&std::fs::read(index_dir.join("embedding_hashes.bin")).unwrap())
                .unwrap();

        std::fs::write(
            dir.path().join("main.rs"),
            "fn totally_different_content() {\n    new_stuff();\n}\n",
        )
        .unwrap();

        build_and_save(dir.path()).unwrap();

        let hashes_after: Vec<String> =
            postcard::from_bytes(&std::fs::read(index_dir.join("embedding_hashes.bin")).unwrap())
                .unwrap();

        assert_ne!(hashes_before, hashes_after);
    }
}
