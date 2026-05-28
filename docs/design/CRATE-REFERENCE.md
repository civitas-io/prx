# Crate Reference

Exact versions, API patterns, and compatibility notes for every dependency.
Verified May 2026. Update this document when upgrading any crate.

---

## Version Constraint: tree-sitter 0.25.8

The original plan specified tree-sitter 0.26, but grammar crate compatibility
analysis revealed that only 1 of 15 grammar crates (tree-sitter-cpp) supports
0.26.x. All others require 0.24.x or 0.25.x.

**Decision: use tree-sitter 0.25.8** for broad grammar compatibility.

---

## Core Dependencies

| Crate | Version | MSRV | Purpose |
|---|---|---|---|
| clap | 4.6 | 1.74 | CLI framework with derive + multicall |
| tree-sitter | 0.25 | - | AST parsing |
| ast-grep-core | 0.42 | - | Structural pattern search |
| safetensors | 0.7 | - | Load embedding weights |
| ndarray | 0.17 | 1.64 | Dense matrix operations |
| sprs | 0.11 | - | Sparse matrices for BM25 |
| tokenizers | 0.23 | - | cl100k_base token counting |
| similar | 3.1 | - | Diff computation |
| bloomfilter | 3.0 | - | Bloom filter for exists |
| serde | 1.0 | - | Serialization |
| serde_json | 1.0 | - | JSON output |
| xxhash-rust | 0.8 | - | Content hashing (xxh3) |
| ignore | 0.4 | - | .gitignore-aware file walking |
| regex | 1.0 | - | Literal search + identifier extraction |
| thiserror | 2.0 | - | Typed library errors |
| anyhow | 1.0 | - | CLI error handling |
| rmcp | 1.x | - | MCP server (optional) |
| tokio | 1.x | - | Async runtime (optional, MCP/watch) |
| notify | 9.0-rc | - | File watching (optional) |

## Dev Dependencies

| Crate | Version | Purpose |
|---|---|---|
| assert_cmd | 2.2 | CLI integration testing |
| predicates | 3.x | Assertion helpers |
| tempfile | 3.x | Temp directories for tests |
| criterion | 0.8 | Benchmarking |

---

## Tree-sitter Grammar Crates

All crates must be compatible with tree-sitter 0.25.x.

| Crate | Version | Language Access | Notes |
|---|---|---|---|
| tree-sitter-rust | 0.24 | `LANGUAGE` const | `LANGUAGE.into()` for Language |
| tree-sitter-python | 0.25 | `LANGUAGE` const | expression_statement is supertype in 0.25 |
| tree-sitter-javascript | 0.25 | `LANGUAGE` const | |
| tree-sitter-typescript | 0.23 | `LANGUAGE_TYPESCRIPT`, `LANGUAGE_TSX` | Two separate Language objects |
| tree-sitter-go | 0.25 | `LANGUAGE` const | |
| tree-sitter-java | 0.23 | `LANGUAGE` const | |
| tree-sitter-c | 0.24 | `LANGUAGE` const | |
| tree-sitter-cpp | 0.23 | `LANGUAGE` const | Also compat with 0.26 |
| tree-sitter-ruby | 0.23 | `LANGUAGE` const | |
| tree-sitter-bash | 0.25 | `LANGUAGE` const | |
| tree-sitter-json | 0.24 | `LANGUAGE` const | |
| tree-sitter-toml | 0.20 | `language()` function | NOT a const, call as fn |
| tree-sitter-yaml | 0.7 | check source | May need verification |
| tree-sitter-html | 0.23 | `LANGUAGE` const | |
| tree-sitter-css | 0.25 | `LANGUAGE` const | |

### Language Access Pattern

Standard (14 crates):
```rust
use tree_sitter_rust::LANGUAGE;
let lang: tree_sitter::Language = LANGUAGE.into();
parser.set_language(&lang)?;
```

TypeScript (special — two languages):
```rust
use tree_sitter_typescript::{LANGUAGE_TYPESCRIPT, LANGUAGE_TSX};
// Use LANGUAGE_TYPESCRIPT for .ts files
// Use LANGUAGE_TSX for .tsx files
```

TOML (special — function, not const):
```rust
let lang = tree_sitter_toml::language();
parser.set_language(&lang)?;
```

---

## API Quick Reference

### clap (multicall busybox)

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "prx", version, about)]
#[command(multicall = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Search(SearchArgs),
    Read(ReadArgs),
    // ...
}
```

### tree-sitter (parse + query)

```rust
use tree_sitter::{Parser, Query, QueryCursor};

let mut parser = Parser::new();
parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
let tree = parser.parse(source, None).unwrap();

let query = Query::new(
    &tree_sitter_rust::LANGUAGE.into(),
    "(function_item name: (identifier) @fn_name)"
)?;
let mut cursor = QueryCursor::new();
for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
    for cap in m.captures {
        let name = cap.node.utf8_text(source.as_bytes())?;
    }
}
```

### safetensors (load from bytes)

```rust
use safetensors::SafeTensors;

static MODEL_BYTES: &[u8] = include_bytes!("../models/potion-retrieval-32M.safetensors");
let tensors = SafeTensors::deserialize(MODEL_BYTES)?;
let weight = tensors.tensor("weight")?;
let shape = weight.shape();  // &[usize]
let data = weight.data();    // &[u8]
```

### ndarray (embedding math)

```rust
use ndarray::{Array1, Array2};

// Mean pool
let mut sum = Array1::<f32>::zeros(256);
for token_idx in token_ids {
    sum += &weights.row(token_idx);
}
sum /= count as f32;

// L2 normalize
let norm = sum.dot(&sum).sqrt();
if norm > 0.0 { sum /= norm; }

// Cosine similarity (vectors already normalized)
let sim = query_vec.dot(&chunk_vec);
```

### sprs (BM25 sparse matrix)

```rust
use sprs::{CsMat, TriMat};

// Build via triplets
let mut tri = TriMat::new((n_chunks, n_terms));
tri.add_triplet(chunk_idx, term_idx, bm25_score);
let mat: CsMat<f32> = tri.to_csc();

// Query: sum columns for query terms
let mut scores = vec![0.0f32; n_chunks];
for term_idx in query_term_indices {
    let col = mat.outer_view(term_idx).unwrap();
    for (row, &val) in col.iter() {
        scores[row] += val;
    }
}
```

### tokenizers (cl100k_base)

```rust
use tokenizers::Tokenizer;

static TOKENIZER_BYTES: &[u8] = include_bytes!("../models/cl100k_base.json");
let tokenizer = Tokenizer::from_bytes(TOKENIZER_BYTES)?;
let encoding = tokenizer.encode(text, false)?;
let token_count = encoding.get_ids().len();
```

### similar (diff)

```rust
use similar::{TextDiff, ChangeTag};

let diff = TextDiff::from_lines(old, new);
for change in diff.iter_all_changes() {
    match change.tag() {
        ChangeTag::Delete => { /* removed line */ }
        ChangeTag::Insert => { /* added line */ }
        ChangeTag::Equal  => { /* unchanged */ }
    }
}
```

### bloomfilter

```rust
use bloomfilter::Bloom;

let mut bloom = Bloom::new_for_fp_rate(50_000, 0.02);
bloom.set(&"authenticate");
assert!(bloom.check(&"authenticate"));    // true
assert!(!bloom.check(&"nonexistent"));    // false (probably)
```

### ignore (file walking)

```rust
use ignore::WalkBuilder;

let walker = WalkBuilder::new(path)
    .hidden(false)
    .add_custom_ignore_filename(".prxignore")
    .build();
for entry in walker.flatten() {
    if entry.file_type().map_or(false, |ft| ft.is_file()) {
        // process file
    }
}
```

### xxhash-rust (content hash)

```rust
use xxhash_rust::xxh3::xxh3_128;

let hash = xxh3_128(file_bytes);
let hex = format!("{:032x}", hash);
```

### rmcp (MCP server)

```rust
use rmcp::{ServerHandler, tool, serve_server};

struct AgServer { /* index cache */ }

#[tool(description = "Search codebase")]
async fn search(&self, query: String, path: Option<String>) -> String {
    // reuse CLI handler
}

#[tokio::main]
async fn main() {
    let server = AgServer::new();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    serve_server(server, (stdin, stdout)).await.unwrap();
}
```

### notify (file watching)

```rust
use notify::{recommended_watcher, RecursiveMode, Watcher};
use std::sync::mpsc;

let (tx, rx) = mpsc::channel();
let mut watcher = recommended_watcher(tx)?;
watcher.watch(Path::new("."), RecursiveMode::Recursive)?;
for event in rx { /* handle file changes */ }
```

### thiserror + anyhow

```rust
// Library errors (thiserror)
#[derive(thiserror::Error, Debug)]
pub enum AgError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// CLI errors (anyhow)
fn main() -> anyhow::Result<()> {
    let result = do_work().context("failed to process")?;
    Ok(())
}
```

### assert_cmd (integration tests)

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_search() {
    Command::cargo_bin("prx").unwrap()
        .args(["search", "--literal", "fn main", "tests/fixtures/"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"ok\""));
}
```

### criterion (benchmarks)

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_chunking(c: &mut Criterion) {
    let source = include_str!("../tests/fixtures/sample.py");
    c.bench_function("chunk_python", |b| {
        b.iter(|| chunk_file(black_box(source), "sample.py", Some("python")))
    });
}

criterion_group!(benches, bench_chunking);
criterion_main!(benches);
```
