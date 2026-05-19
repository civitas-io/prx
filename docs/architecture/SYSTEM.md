# ag System Architecture

## Overview

`prx` is a single Rust binary with a busybox-style architecture. All subcommands share common infrastructure (tree-sitter parsing, token counting, JSON output, content hashing) but each command is a self-contained module. The binary can be invoked directly as `prx <subcommand>` or via hardlinks named after each subcommand.

---

## Binary Architecture

- Single binary via `clap::Command::multicall(true)` — invokable as `prx search` or as a hardlink `ag-search`
- Subcommand dispatch via a Rust enum (`Commands::Search`, `Commands::Read`, `Commands::Find`, `Commands::Edit`, `Commands::Diff`, etc.)
- Each command lives in `src/commands/` as its own module (`search.rs`, `read.rs`, `find.rs`, `edit.rs`, `diff.rs`, etc.)
- Shared infrastructure lives in the `src/` root modules, imported by any command that needs it

---

## Shared Infrastructure

### Tree-sitter Parsing (`src/parsing/`)

- AST parsing for 15 languages (v0.1), extensible via additional grammar crates
- Structural queries for function and class boundary detection
- Syntax validation before processing
- Language grammars compiled directly into the binary — no runtime grammar loading

### Token Counting (`src/tokens.rs`)

- Uses the `tokenizers` crate
- Provides token counts used for `--budget` enforcement across all commands
- Commands select results greedily until the token budget is exhausted

### JSON Output (`src/output.rs`)

- Standardized output envelope with a `version` field
- Structured errors written to stdout (never stderr)
- All agent-consumable output goes through this module

### Content Hashing (`src/hash.rs`)

- xxh3 for fast content-addressable hashing
- Used for cache invalidation and change detection

### File Walking (`src/walk.rs`)

- Built on the `ignore` crate
- Respects `.gitignore`, `.prxignore`, and standard ignore patterns
- Used by search, find, and index commands

---

## Search Subsystem (`src/search/`)

Hybrid retrieval engine implemented in pure Rust. Supports three retrieval modes that can be combined.

### Retrieval Modes

- **Literal** — regex matching against raw chunk text
- **Semantic** — dense vector search via Model2Vec embeddings
- **Structural** — AST pattern matching via ast-grep patterns

### Chunking (`src/chunking/`)

- Tree-sitter-based chunking with a 1500-character target size
- Syntax-aware boundaries (won't split mid-function or mid-class)
- No overlap between chunks

### Dense Index (`src/index/dense.rs`)

- Model2Vec static embeddings using the `potion-code-16M` model
- Model weights embedded in the binary via `include_bytes!`
- 256-dimensional float16 vectors
- Inference pipeline: tokenize input, lookup token embeddings, mean pool, L2 normalize

### Sparse Index (`src/index/sparse.rs`)

- BM25 scoring with compound identifier tokenization
- Tokenizer splits `camelCase` and `snake_case` identifiers into component terms
- Pre-computed scores stored in a CSC sparse matrix

### Fusion (`src/search/fusion.rs`)

- Reciprocal Rank Fusion (RRF) with `k=60`
- Adaptive alpha weighting: 0.3 for symbol-like queries, 0.5 for natural language queries

### Ranking (`src/ranking/`)

Reranking pipeline applied after fusion:

- **Definition boost** — 3x score multiplier for definition sites vs. usage sites
- **Identifier stem matching** — boosts results where query terms appear in identifiers
- **File coherence boost** — up to 0.2x max_score added when multiple chunks from the same file rank highly
- **Noise penalties** — test files (0.3x), compatibility shims (0.3x), example files (0.3x), `.d.ts` declaration files (0.7x)
- **File saturation decay** — each additional chunk from the same file is penalized by 0.5^(n-1) to prevent one file from dominating results

---

## Index Management (`src/index/`)

- **In-memory by default** — index is built on demand at query time, fast enough for most repositories
- **Persistent index** — `prx index .` writes the index to `.prx/index/` for large repos or repeated queries
- **File watching** — optional `--watch` flag uses the `notify` crate to keep the persistent index current
- **Bloom filter** — O(1) existence checks before full index lookup

---

## MCP Server (`src/commands/mcp.rs`)

- Compiled in by default (controlled by the `mcp` Cargo feature)
- Exposes all `prx` tools as MCP tools over stdio transport
- Uses the `rmcp` crate (official Anthropic Rust SDK)
- Async runtime via `tokio`, only linked when the `mcp` feature is enabled

---

## Data Flow

Typical path for a search query:

1. CLI parses args, dispatches to `Commands::Search`
2. File walker discovers files, respecting `.gitignore`
3. Tree-sitter chunks each file (1500-char, syntax-aware boundaries)
4. If semantic mode: embed chunks via Model2Vec (lookup + mean pool + normalize)
5. If semantic mode: embed query, run cosine similarity search against chunk vectors
6. If literal mode: regex match against chunk text
7. BM25 scores computed (if hybrid or sparse mode)
8. RRF fusion combines scores from active retrievers
9. Reranking pipeline applies boosts and penalties
10. Budget enforcement selects top results greedily until token limit is reached
11. Results serialized as JSON and written to stdout

---

## Error Handling

- All errors written to stdout as structured JSON:
  ```
  {"error": "...", "code": "...", "suggestion": "..."}
  ```
- stderr is never used for agent-consumable output
- Exit codes: `0` for success, `1` for errors, `2` for usage errors

---

## Feature Flags

Defined in `Cargo.toml`:

| Feature | Dependencies | Purpose |
|---------|-------------|---------|
| `default` | `["mcp"]` | Includes MCP server by default |
| `mcp` | `rmcp`, `tokio` | MCP stdio server |
| `watch` | `notify`, `tokio` | File watching for persistent index |

Features that pull in `tokio` only link the async runtime when those features are active. The core binary without `mcp` or `watch` is fully synchronous.
