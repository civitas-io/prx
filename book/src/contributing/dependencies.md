# Dependencies

This page documents all dependencies, their versions, and why each is needed. Update this page when upgrading any crate.

Verified May 2026.

## MSRV Policy

Minimum Supported Rust Version: **1.85** (Rust edition 2024).

The MSRV is set in `Cargo.toml`. It's tested in CI on every commit. Don't use language features or standard library APIs introduced after 1.85 without bumping the MSRV and updating this page.

## Core Dependencies

| Crate | Version | Purpose |
|---|---|---|
| clap | 4.6 | CLI framework with derive macros and multicall support |
| tree-sitter | 0.25 | AST parsing for chunking, outline, snap, structural search |
| ast-grep-core | 0.42 | Structural pattern search (the `--structural` mode) |
| safetensors | 0.7 | Load embedding model weights (zero-copy mmap) |
| ndarray | 0.17 | Dense matrix operations for embedding inference |
| sprs | 0.11 | Sparse matrices for BM25 scoring (CSC format) |
| tokenizers | 0.23 | cl100k_base token counting for `--budget` enforcement |
| similar | 3.1 | Diff computation for `prx diff` |
| bloomfilter | 3.0 | Bloom filter for `prx exists` O(1) checks |
| serde | 1.0 | Serialization framework |
| serde_json | 1.0 | JSON output |
| xxhash-rust | 0.8 | Content hashing (xxh3 feature) |
| ignore | 0.4 | .gitignore-aware file walking (from ripgrep) |
| regex | 1.0 | Literal search and identifier extraction |
| thiserror | 2.0 | Typed library errors |
| anyhow | 1.0 | CLI error handling |

## Optional Dependencies

These are only linked when the corresponding feature is enabled.

| Crate | Version | Feature | Purpose |
|---|---|---|---|
| rmcp | 1.x | `mcp` | MCP server (official Anthropic Rust SDK) |
| tokio | 1.x | `mcp`, `watch` | Async runtime (only linked for MCP and file watching) |
| notify | 9.0-rc | `watch` | File watching for `prx index --watch` |

The core binary without `mcp` or `watch` is fully synchronous. No async runtime is linked.

## Dev Dependencies

| Crate | Version | Purpose |
|---|---|---|
| assert_cmd | 2.2 | CLI integration testing |
| predicates | 3.x | Assertion helpers for assert_cmd |
| tempfile | 3.x | Temp directories for tests |
| criterion | 0.8 | Benchmarking |

## Tree-sitter Grammar Crates

All grammar crates must be compatible with tree-sitter 0.26.x via the `LanguageFn` API. Where upstream crates don't support 0.26, we use compatible forks.

**Original grammars (13):**

| Crate | Version | Language | Notes |
|---|---|---|---|
| tree-sitter-rust | 0.24 | Rust | `LANGUAGE` const |
| tree-sitter-python | 0.25 | Python | `LANGUAGE` const |
| tree-sitter-javascript | 0.25 | JavaScript | `LANGUAGE` const |
| tree-sitter-typescript | 0.23 | TypeScript, TSX | `LANGUAGE_TYPESCRIPT`, `LANGUAGE_TSX` |
| tree-sitter-go | 0.25 | Go | `LANGUAGE` const |
| tree-sitter-java | 0.23 | Java | `LANGUAGE` const |
| tree-sitter-c | 0.24 | C | `LANGUAGE` const |
| tree-sitter-cpp | 0.23 | C++ | `LANGUAGE` const |
| tree-sitter-ruby | 0.23 | Ruby | `LANGUAGE` const |
| tree-sitter-bash | 0.25 | Bash | `LANGUAGE` const |
| tree-sitter-json | 0.24 | JSON | `LANGUAGE` const |
| tree-sitter-html | 0.23 | HTML | `LANGUAGE` const |
| tree-sitter-css | 0.25 | CSS | `LANGUAGE` const |

**v0.5.10 additions (14):**

| Crate | Version | Language | Notes |
|---|---|---|---|
| tree-sitter-kotlin-sg | 0.4 | Kotlin | `-sg` fork for 0.26. `LANGUAGE` const |
| tree-sitter-swift | 0.7 | Swift | `LANGUAGE` const |
| tree-sitter-c-sharp | 0.23 | C# | `LANGUAGE` const |
| tree-sitter-php | 0.24 | PHP | `LANGUAGE_PHP` const |
| tree-sitter-elixir | 0.3 | Elixir | `LANGUAGE` const |
| tree-sitter-yaml | 0.7 | YAML | `LANGUAGE` const |
| tree-sitter-toml-ng | 0.7 | TOML | `-ng` fork for 0.26. `LANGUAGE` const |
| tree-sitter-md | 0.5 | Markdown | `LANGUAGE` const |
| tree-sitter-containerfile | 0.8 | Dockerfile | `LANGUAGE` const |
| tree-sitter-hcl | 1.1 | HCL/Terraform | `LANGUAGE` const |
| tree-sitter-sequel | 0.3 | SQL | `LANGUAGE` const |
| tree-sitter-make | 1.1 | Makefile | `LANGUAGE` const |

**Standard access pattern (all crates except PHP and TypeScript):**

```rust
use tree_sitter_rust::LANGUAGE;
let lang: tree_sitter::Language = LANGUAGE.into();
parser.set_language(&lang)?;
```

**TypeScript (two languages):**

```rust
use tree_sitter_typescript::{LANGUAGE_TYPESCRIPT, LANGUAGE_TSX};
// Use LANGUAGE_TYPESCRIPT for .ts files
// Use LANGUAGE_TSX for .tsx files
```

**PHP (different const name):**

```rust
use tree_sitter_php::LANGUAGE_PHP;
let lang: tree_sitter::Language = LANGUAGE_PHP.into();
parser.set_language(&lang)?;
```

## Why These Choices

**clap over structopt:** clap 4.x includes derive macros natively. structopt is deprecated.

**tree-sitter 0.26:** Modern `LanguageFn` API, broad ecosystem adoption. Compatible forks used where upstream crates lag behind.

**safetensors over manual deserialization:** Zero-copy mmap, standard format, maintained by HuggingFace.

**ndarray over nalgebra:** ndarray is the standard for numerical computing in Rust. nalgebra is better for linear algebra but ndarray's array slicing is more natural for embedding operations.

**sprs over manual sparse matrix:** sprs is the standard Rust sparse matrix crate. CSC format is optimal for column-wise BM25 queries.

**ignore over walkdir:** ignore is from ripgrep and handles .gitignore correctly. walkdir doesn't understand .gitignore.

**similar over diff:** similar is pure Rust and handles both line-level and character-level diffs. The `diff` crate is older and less maintained.

**xxhash-rust over blake3:** xxh3 is faster for content hashing where cryptographic security isn't needed. blake3 is better for security-sensitive hashing.

**thiserror + anyhow over custom error types:** thiserror generates boilerplate for typed errors. anyhow is ergonomic for CLI error propagation. Using both is the standard Rust pattern.

## Evaluating New Dependencies

Before adding a dependency:

1. Check if an existing dependency already provides the functionality.
2. Check the crate's maintenance status (last commit, open issues, downloads).
3. Check the MSRV — it must be <= 1.85.
4. Check for security advisories via `cargo audit`.
5. Check license compatibility (Apache 2.0 or MIT preferred).
6. Add a comment in `Cargo.toml` explaining why the crate is needed.

Run `cargo deny check` after adding any dependency. This checks for license compliance, duplicate dependencies, and security advisories.
