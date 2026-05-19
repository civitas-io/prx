# Detailed System Design

Design decisions and internal behavior for every subsystem in ag. This document
is the implementation reference — it specifies what each module does, how it
does it, and why.

Settled decisions from this design session are marked with rationale. Do not
revisit without discussion.

---

## 1. Output Envelope (`src/output.rs`)

Every ag command returns a JSON envelope to stdout.

### Success Envelope

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": { ... }
}
```

### Error Envelope

```json
{
  "version": "0.2.0",
  "command": "read",
  "status": "error",
  "error": {
    "code": "file_not_found",
    "message": "File not found: src/auth.ts",
    "suggestion": "Use `prx find` to discover files."
  }
}
```

### Decisions

- **`version`**: compiled from `env!("CARGO_PKG_VERSION")`.
- **`tokens`**: estimated token count of the **entire JSON response** (envelope
  + data), not just the data payload. Computed post-serialization. Uses `len/4`
  when `--budget` is not specified, exact cl100k_base count when `--budget` is
  active (since budget enforcement needs precision).
- **Errors go to stdout.** stderr is for `RUST_LOG` debug logging only.
- **`--plain` mode**: bypasses the JSON envelope entirely. Separate formatting
  code path.
- **JSONL mode** (`prx batch`): one JSON envelope per line, no wrapping array.
- **Internally**: command handlers return `Result<CommandOutput, AgError>`.
  The output module serializes. Handlers never write to stdout directly.

---

## 2. Content Hashing (`src/hash.rs`)

- **Algorithm**: xxh3 128-bit via `xxhash-rust` crate.
- **Input**: raw file bytes.
- **Output**: hex-encoded 128-bit string.
- **No cache**: xxh3 runs at ~30 GB/s. Computing is cheaper than HashMap lookup
  for typical file sizes.
- **Appears in**: `prx read` (meta.hash), `prx search` (per-match file hash),
  `prx diff` (old_hash, new_hash), `prx edit` (before/after hash).

---

## 3. Token Counting (`src/tokens.rs`)

Two modes:

| Mode | When Used | Method |
|---|---|---|
| Fast | `--budget` not specified | `byte_count / 4` |
| Exact | `--budget` specified | cl100k_base tokenizer |

The cl100k_base tokenizer vocabulary (~2MB) is embedded via `include_bytes!`.
Loaded lazily on first `--budget` call. Reused for the process lifetime.

---

## 4. File Walking (`src/walk.rs`)

Wrapper around the `ignore` crate.

- Respects `.gitignore` (built into `ignore`)
- Respects `.prxignore` (custom file for ag-specific exclusions)
- Skips binary files (null byte in first 8KB)
- Skips files > 1MB (configurable via `PRX_MAX_FILE_SIZE` env var)
- Returns: file path, file size, detected language
- Language detection: by file extension only. Map in `src/parsing/languages.rs`.

---

## 5. Tree-sitter Parsing (`src/parsing/`)

### Module Structure

```
src/parsing/
├── mod.rs          -- Parser creation, language dispatch
├── languages.rs    -- Extension-to-Language map, grammar references
├── outline.rs      -- Symbol extraction via tree-sitter queries
└── snap.rs         -- Structural snapping (expand range to enclosing node)
```

### Parser Management

Tree-sitter parsers are not thread-safe. One parser per language, reused
within a thread. For parallel indexing, create per-thread parsers.

### Language Grammars

All compiled into the binary via `tree-sitter-*` crates. No runtime loading.

`languages.rs` maps extensions to `tree_sitter::Language`:

```
.rs  -> tree_sitter_rust::LANGUAGE
.py  -> tree_sitter_python::LANGUAGE
.ts  -> tree_sitter_typescript::language_typescript()
.tsx -> tree_sitter_typescript::language_tsx()
```

Returns `Option<Language>`. None triggers fallback (line-based chunking, no
structural features).

### Query Files

Per-language tree-sitter queries embedded via `include_str!`:

- `symbols.scm` per language: for outline extraction
- `definitions.scm` per language: for definition boost in search ranking

Queries are 10-30 lines each. Starting set covers the 15 shipped languages.

### Symbol Extraction (`outline.rs`)

Returns `Vec<Symbol>`:

```
Symbol {
    name: String,
    kind: SymbolKind,   // Function, Class, Method, Struct, Enum, Trait, ...
    lines: (usize, usize),  // 1-indexed, inclusive
    signature: String,  // first line of the definition
    children: Vec<Symbol>,
}
```

### Structural Snapping (`snap.rs`)

Given a line range and snap target (function/class/block):

1. Find the smallest named node containing the requested lines
2. Walk up the AST until reaching the target node type
3. Return the expanded line range

---

## 6. Chunking (`src/chunking/`)

### Chunk Type

```
Chunk {
    content: String,
    file_path: String,       // repo-relative
    start_line: usize,       // 1-indexed
    end_line: usize,         // 1-indexed, inclusive
    start_byte: usize,
    end_byte: usize,
    language: Option<String>,
}
```

### Algorithm

1. Parse with tree-sitter
2. Recursive `merge_nodes(root, 1500)`:
   - No children: return node as single boundary
   - Walk children, accumulate adjacent siblings while `bytes < 1500`
   - Single child exceeds 1500: recurse into that child
   - Emit accumulated groups as chunk boundaries
3. Convert byte boundaries to string slices + line numbers

Chunks are contiguous, non-overlapping, syntax-aware. A function is never
split unless it exceeds 1500 chars.

Fallback for unsupported languages: split on newlines at 1500-char boundary.

Target size: 1500 chars. Hardcoded for v0.1.

---

## 7. Dense Index — Model2Vec (`src/index/dense.rs`)

### Model Loading

Weights are a safetensors file embedded via `include_bytes!`.

At initialization:
1. Deserialize safetensors buffer (zero-copy)
2. Extract embedding matrix: `[62500, 256]`, float16
3. Convert to float32 `ndarray::Array2` for computation
4. Extract vocabulary: `HashMap<String, usize>` (token -> index)

The vocabulary is the Model2Vec tokenizer. No separate tokenizer file.

### Embedding a Chunk

1. Tokenize text against Model2Vec vocabulary (whitespace + subword lookup)
2. For each known token, fetch its 256-dim row from the embedding matrix
3. Sum all vectors, divide by count (mean pool)
4. L2-normalize

### Searching

1. Embed query via the same pipeline
2. Dot product against all chunk embeddings (cosine similarity, since
   vectors are normalized)
3. Return top-k by score

### Memory

For 5000 chunks: 5000 * 256 * 4 = ~5MB. Negligible.

---

## 8. Sparse Index — BM25 (`src/index/sparse.rs`)

### Identifier Tokenization (`src/search/tokenize.rs`)

1. Extract identifiers: regex `[a-zA-Z_][a-zA-Z0-9_]*`
2. Split compounds: snake_case on `_`, camelCase on case boundaries
3. Lowercase all
4. Return compound + sub-tokens: `"getHTTPResponse"` ->
   `["gethttpresponse", "get", "http", "response"]`

No stemming.

### Content Enrichment

Before BM25 indexing, each chunk is augmented with:
- File stem repeated 2x
- Last 3 directory path components

### BM25 Scoring

Robertson BM25 with k1=1.5, b=0.75.

Pre-computed at index time into CSC sparse matrix (rows=chunks, cols=terms).
Query time: extract columns for query terms, sum -> score per chunk.

Implementation: `sprs` crate (pure Rust sparse matrices).

---

## 9. Fusion and Ranking

### RRF Fusion (`src/search/fusion.rs`)

1. Semantic search -> top_k*5 candidates
2. BM25 search -> top_k*5 candidates
3. RRF: `score = 1/(60 + rank)` per list
4. Combined: `alpha * RRF(semantic) + (1-alpha) * RRF(bm25)`
5. Alpha: 0.3 for symbol queries, 0.5 for NL queries. Auto-detected via
   regex heuristic unless `--alpha` overrides.

### Reranking Pipeline (`src/ranking/`)

Applied in order:

1. **File coherence**: boost top chunk of multi-match files by
   `max_score * 0.2 * (file_sum / max_file_sum)`
2. **Definition boost**: 3x for definition sites. +1.5x if file stem matches.
3. **Stem matching**: prefix match query keywords against path components.
   Boost = `max_score * match_ratio` if >= 10% match.
4. **Noise penalties**: test 0.3x, compat 0.3x, examples 0.3x, re-export
   0.5x, .d.ts 0.7x. Multiplicative.
5. **Saturation decay**: during selection, nth chunk from same file scores
   `score * 0.5^(n-1)`.

### Budget Enforcement

Greedy selection by descending score. Skip (don't truncate) chunks exceeding
remaining budget. Continue token: base64-encoded `(query_hash, position)`.

---

## 10. Bloom Filter (`src/index/bloom.rs`)

- Built from all identifier tokens in the codebase
- 2% false positive rate, ~75KB for 50K tokens
- "No" from bloom = definitely absent (exact)
- "Yes" from bloom = probably present (probable)
- `--exact` flag: on "probable", confirm with literal search
- Built as byproduct of index construction

---

## 11. prx search (`src/commands/search.rs`)

1. Parse args
2. Auto-detect mode if not specified
3. `--exists`: bloom filter only, short-circuit
4. `--literal`: walk files, regex match, return
5. `--structural`: ast-grep pattern match, return
6. `--semantic`/hybrid:
   a. Walk files, chunk, build dense+sparse+bloom indexes
   b. Fusion, ranking, budget enforcement
   c. If `--context function|class`: expand results via snap.rs
   d. Serialize, envelope, stdout
7. Index caching: use `.prx/index/` if present and fresh

---

## 12. prx read (`src/commands/read.rs`)

1. Read file, compute hash, detect language
2. `--hash`: return hash only (cheapest call)
3. `--outline`: parse, extract symbols, return symbol table
4. `--skeleton`: parse, return first line of each symbol definition
5. `--lines START-END`: extract range
   - `--snap function|class|block`: expand to enclosing structure
6. No range: return full content
7. `--budget`: if over budget, return centered window + `truncated: true`
8. Always include: meta (language, lines, bytes, modified, hash) and outline
   (symbol table). Suppress outline with `--quiet`.

Decision: **outline is included by default** alongside content. One call = full
answer. Agent gets content + ToC + metadata + hash without a second call.

---

## 13. prx find (`src/commands/find.rs`)

1. Walk directory (ignore crate)
2. Apply filters: `--pattern`, `--depth`, `--changed-since`
3. Per file: path, size, line count, language
4. `--outline`: parse each file, count symbols (expensive, on-demand)
5. `--related-to QUERY`: embed query + compute file relevance scores
6. Build tree + flat output (default: both, `--tree`/`--flat` for one)
7. Budget: truncate flat by relevance, prune tree depth-first
8. `--changed-since`: git log/diff for git repos, mtime fallback

---

## 14. prx edit (`src/commands/edit.rs`)

1. Read file
2. `--in-function`/`--in-class`: parse, find scope, restrict to byte range
3. Find matches: literal (default) or regex (`--regex`)
4. `--all`: all matches. Default: first only.
5. Compute replacements: (line, before, after) tuples
6. `--syntax-check` (default true): apply in memory, re-parse, check errors
7. `--dry-run` (default): return changes without writing
8. `--apply`: write to disk, return changes as confirmation
9. Include hash before and after

Multi-edit: `--find`/`--replace` can be specified multiple times. All applied
atomically.

Safety: never writes to disk without `--apply`.

---

## 15. prx diff (`src/commands/diff.rs`)

1. Determine comparison: `--since REF`, `--staged`, or default HEAD
2. Parse diff (via git) or compute directly (via `similar` crate)
3. Per changed file:
   a. Parse old+new with tree-sitter
   b. Map hunks to enclosing functions via snap.rs
   c. Detect moves: identical block deleted+added elsewhere
4. Build heuristic summary: "Modified handleLogin: changed auth header
   access pattern."
5. Build semantic_notes: compare old/new symbol outlines. Report new/removed
   functions, signature changes, import changes.
6. `--stat-only`: summary + stats only (~30 tokens)
7. `--functions`: group hunks by enclosing function
8. Budget: summary first, then largest hunks, until budget exhausted
9. Non-git repos: error with suggestion

Decision: **summary is heuristic, not template-based.** Pattern-matching on the
structural diff produces natural language descriptions. Deterministic, no LLM.

---

## 16. prx index (`src/commands/index.rs`)

1. Walk files, chunk, embed, build BM25 matrix, build bloom
2. Write to `.prx/index/`:
   - `chunks.bin`, `dense.bin`, `sparse.bin`, `bloom.bin`, `meta.json`
3. `meta.json`: ag version, timestamp, file count, per-file content hashes
4. `--watch`: file watcher (notify crate), incremental re-index
5. `--stats`: print index statistics
6. Validation: before using cached index, check version + file hashes

Incremental re-index on file change:
1. Re-chunk changed file
2. Re-embed new chunks
3. Update BM25 matrix (remove old rows, insert new)
4. Update bloom filter
5. Write updated index

---

## 17. prx batch (`src/commands/batch.rs`)

- Read JSONL from stdin
- Each line: `{"cmd": "search", "query": "auth", "budget": 300}`
- Optional `id` field for request correlation
- Parse all lines first
- Execute in parallel (std threads, not tokio)
- Write JSONL to stdout, order matches input order
- Errors: per-line error envelopes, other commands continue

---

## 18. prx mcp (`src/commands/mcp.rs`)

- MCP server on stdio via `rmcp` crate
- Exposes: search, read, find, edit, diff, exists, outline
- **Each MCP tool reuses the CLI command handler.** MCP params deserialized
  into same struct as clap args. No duplicate logic.
- Index cache: in-memory, LRU 10 repos, file watcher for local paths
- Only subsystem requiring tokio

---

## 19. prx init (`src/commands/init.rs`)

- Auto-detect frameworks: check `which claude`, `~/.cursor/`, etc.
- Generate configs from templates embedded via `include_str!`
- Writes per framework:
  - Claude Code: `.claude/agents/ag-search.md` + `claude mcp add`
  - Cursor: `.cursor/mcp.json`
  - Codex: `~/.codex/config.toml`
  - OpenCode: `~/.opencode/config.json`
  - Any: append to `AGENTS.md`
- `--agents-md`: idempotent (checks if snippet exists before appending)

---

## 20. prx stats (`src/commands/stats.rs`)

- Reads `~/.prx/stats.jsonl`
- Each line: `{ts, command, mode, result_tokens, file_chars_avoided, latency_ms}`
- Aggregates: today, 7d, 30d, all time
- `--verbose`: per-command breakdown
- `--reset`: truncate file
- Stats writing is fire-and-forget. Failures do not block commands.

---

## Cross-Cutting: Threading Model

- **Default (no `--features mcp`)**: fully synchronous. Single-threaded.
  Commands run sequentially. Tree-sitter parsing is single-threaded (parsers
  aren't thread-safe).
- **With `prx batch`**: parallel execution via std scoped threads. Each command
  gets its own thread with its own tree-sitter parser.
- **With `prx mcp`**: tokio async runtime. Index building runs in
  `spawn_blocking`. MCP event loop is async.
- **Indexing**: for large repos (>1000 files), chunking and embedding can be
  parallelized per-file using `rayon` or scoped threads. Tree-sitter parsers
  created per-thread. Embedding is pure math (no shared state).

---

## Cross-Cutting: Error Types

```
AgError (thiserror)
├── FileNotFound { path }
├── ParseError { path, language, message }
├── InvalidArgument { flag, message }
├── IndexCorrupted { path, reason }
├── GitError { message }
├── IoError(std::io::Error)
└── Internal { message }
```

Each variant maps to an error `code` in the JSON envelope. The `suggestion`
field is populated per-variant with actionable guidance.

---

## Cross-Cutting: Configuration

No config file in v0.1. Behavior controlled by:

- CLI flags (primary)
- Environment variables (secondary):
  - `PRX_MAX_FILE_SIZE`: max file size to process (default: 1MB)
  - `PRX_CHUNK_SIZE`: chunk target in chars (default: 1500, future)
  - `RUST_LOG`: debug logging level
- `.prxignore`: custom ignore patterns (alongside .gitignore)

Config file (`.prx/config.toml`) deferred to v0.2.
