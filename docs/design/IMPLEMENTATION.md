# Implementation Plan

This plan maps directly to the Roadmap phases. Each step has a defined input, output, acceptance test, and references to the design docs. Implementation order is dependency-driven — lower layers first.

All design decisions are in SYSTEM-DESIGN.md. All CLI flags are in CLI.md. All output schemas are in OUTPUT.md. This plan does not duplicate those — it references them.

---

## Phase 0: Foundation (Weeks 1-2)

### Step 0.1: Project Scaffold

**Files:**
- `src/main.rs` — clap derive `Parser`, `multicall(true)`
- `src/lib.rs` — public API surface
- `src/commands/mod.rs` — `Commands` enum with all subcommands: `Search`, `Read`, `Find`, `Edit`, `Diff`, `Index`, `Outline`, `Exists`, `Batch`, `Stats`, `Mcp`, `Init`
- Each variant has its own args struct matching CLI.md
- `src/commands/{search,read,find,edit,diff,index,outline,exists,batch,stats,mcp,init}.rs` — empty handler stubs
- `.github/workflows/ci.yml` — `cargo fmt`, `clippy`, `test`, build matrix

**Acceptance:** `cargo build` succeeds. `prx --help` prints the full subcommand list.

---

### Step 0.2: Output Envelope (`src/output.rs`)

**Structs:**
```
Envelope<T: Serialize> { version, command, status, tokens, data: T }
ErrorEnvelope { version, command, status, error: ErrorDetail }
ErrorDetail { code, message, suggestion }
```

**Error type:**
```
AgError (thiserror): FileNotFound, ParseError, InvalidArgument,
                     IndexCorrupted, GitError, IoError, Internal
```

Each command handler returns `Result<Box<dyn Serialize>, AgError>`. `output.rs` serializes to JSON and computes token count post-serialization (`len / 4` fast mode). `--plain` bypasses JSON and writes human-readable text.

**Acceptance:** Unit test serializes a mock `CommandOutput` and verifies the JSON shape matches OUTPUT.md.

---

### Step 0.3: Content Hashing (`src/hash.rs`)

**Functions:**
```
hash_file(path: &Path) -> String   // hex-encoded xxh3_128
hash_bytes(data: &[u8]) -> String
```

Uses `xxhash_rust::xxh3::xxh3_128`.

**Acceptance:** Unit test with known input/output pair.

---

### Step 0.4: File Walking (`src/walk.rs`)

**Output type:**
```
WalkEntry { path, size, language: Option<String> }
```

**Function:**
```
walk(root: &Path, opts: &WalkOpts) -> Vec<WalkEntry>
```

Wraps `ignore::WalkBuilder`. Adds `.prxignore` via `WalkBuilder::add_ignore()`. Skips binary files (null byte in first 8KB). Skips files exceeding `PRX_MAX_FILE_SIZE` env var (default 1MB).

**Acceptance:** Integration test with a temp dir containing a `.gitignore`, a binary file, and a text file. Verify binary and ignored files are excluded.

---

### Step 0.5: Token Counting (`src/tokens.rs`)

**Functions:**
```
count_tokens_fast(text: &str) -> usize   // byte_count / 4
count_tokens_exact(text: &str) -> usize  // cl100k_base tokenizer
```

The `cl100k_base` `tokenizer.json` (~2MB) is embedded via `include_bytes!`. Lazy initialization: tokenizer loaded on first `--budget` call.

**Acceptance:** Unit test comparing fast vs exact on code samples. Verify within 20% of each other.

---

### Step 0.6: Tree-sitter Integration (`src/parsing/`)

**Files:**
- `languages.rs` — `HashMap<&str, fn() -> Language>` mapping extensions to grammars
- `mod.rs` — `create_parser(language: &str) -> Option<Parser>`
- `outline.rs`:
  ```
  Symbol { name, kind, lines: (usize, usize), signature, children: Vec<Symbol> }
  extract_symbols(source: &str, language: &str) -> Vec<Symbol>
  ```
- `snap.rs`:
  ```
  SnapTarget { Function, Class, Block }
  snap_to_structure(source: &str, language: &str,
                    line_range: (usize, usize),
                    target: SnapTarget) -> (usize, usize)
  ```
- `queries/` — per-language `.scm` files embedded via `include_str!`

Start with queries for: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++.

**Acceptance:** Unit tests for outline extraction on sample files in each language.

---

### Step 0.7: Chunking (`src/chunking/`)

**Types:**
```
Chunk { content, file_path, start_line, end_line, start_byte, end_byte, language }
```

**Function:**
```
chunk_file(source: &str, file_path: &str, language: Option<&str>) -> Vec<Chunk>
```

Tree-sitter path: parse AST, recursive `merge_nodes(root, 1500)`. Fallback path: line-based chunking for unsupported languages.

**Acceptance:** Unit test on a 3000-char Python file. Verify 2 chunks produced, neither splits a function body.

---

### Step 0.8: Model2Vec Embedding (`src/index/dense.rs`)

**Functions:**
```
load_model() -> (HashMap<String, usize>, Array2<f32>)
embed_text(text: &str,
           vocab: &HashMap<String, usize>,
           weights: &Array2<f32>) -> Array1<f32>
embed_chunks(chunks: &[Chunk], ...) -> Array2<f32>
```

`potion-code-16M.safetensors` embedded via `include_bytes!`. `load_model` deserializes safetensors, extracts `"embeddings"` tensor, converts float16 to float32, extracts vocabulary. `embed_text` tokenizes against vocabulary, looks up rows, mean pools, L2 normalizes.

**Acceptance:** Embed two semantically similar chunks. Verify cosine similarity > 0.5.

---

### Step 0.9: BM25 Index (`src/index/sparse.rs`)

**Tokenizer (`src/search/tokenize.rs`):**
```
extract_identifiers(text: &str) -> Vec<String>
split_identifier(token: &str) -> Vec<String>   // camelCase + snake_case
tokenize_for_bm25(text: &str) -> Vec<String>
```

**Enrichment:**
```
enrich_for_bm25(chunk: &Chunk) -> String   // append file stem 2x + dir components
```

**Index:**
- Term-document frequency matrix
- IDF computed per term
- BM25 scores with k1=1.5, b=0.75
- Stored as `sprs::CsMat<f32>` in CSC format

**Query:**
```
query_bm25(query_tokens: &[String], index: &CsMat<f32>) -> Vec<(usize, f32)>
```

Extracts columns for query terms, sums per row.

**Acceptance:** Index 10 chunks, query with a known identifier, verify top result is correct.

---

### Step 0.10: Literal Search (`prx search --literal`)

Walk files, match against raw content using the `regex` crate. Return matches with file, line, column, matched text, and surrounding context lines.

This is the Phase 0 milestone: `prx search --literal "pattern" src/` works end-to-end.

**Acceptance:** Search for a known pattern in a test fixture. Verify JSON output matches OUTPUT.md schema exactly.

---

## Phase 1: Core Tools (Weeks 3-5)

### Step 1.1: Hybrid Search (`prx search --semantic/hybrid`)

**Files:**
- `src/search/fusion.rs` — RRF: `1/(60+rank)` on each retriever, combined with adaptive alpha
- `src/ranking/weighting.rs` — alpha resolution: symbol detection regex, 0.3 vs 0.5

Over-fetch `top_k * 5` candidates before reranking.

**Acceptance:** Semantic search for "authentication" finds auth-related code even without a literal match.

---

### Step 1.2: Reranking Pipeline (`src/ranking/`)

**Files:**
- `boosting.rs` — file coherence boost, definition boost, stem matching
- `penalties.rs` — noise penalties (test files, compat, examples, re-export, `.d.ts`), saturation decay

Apply in the order documented in SYSTEM-DESIGN.md section 9.

**Acceptance:** Verify test files rank below source files for the same query.

---

### Step 1.3: Budget Enforcement

Greedy selection by descending score. Skip (don't truncate) chunks exceeding remaining budget. Continuation token: `base64(query_hash, position)`. `--continue TOKEN` resumes from position.

**Acceptance:** Search with `--budget 500`. Verify total tokens in response <= 500.

---

### Step 1.4: Structural Search (`prx search --structural`)

Use `ast_grep_core::Pattern` to parse metavariable patterns. Walk files, match against each file's AST. Return matched nodes with file, line, and matched text.

**Acceptance:** Pattern `fn $NAME($$$)` finds all function definitions in a Rust fixture.

---

### Step 1.5: Search Auto-Detection

- Fewer than 3 tokens or regex metacharacters: literal
- Contains `$VAR` metavariables: structural
- Otherwise: semantic/hybrid

**Acceptance:** Verify auto-detection on 10 test queries covering all three branches.

---

### Step 1.6: `prx read`

Full implementation per SYSTEM-DESIGN.md section 12. Flags: `--lines`, `--snap`, `--skeleton`, `--outline`, `--hash`, `--budget`, `--meta`. Outline included by default; suppress with `--quiet`.

**Acceptance:** Read a Python file with `--skeleton`. Verify only signatures returned, no bodies.

---

### Step 1.7: `prx find`

Full implementation per SYSTEM-DESIGN.md section 13. Flags: `--pattern`, `--depth`, `--changed-since`, `--related-to`, `--tree`, `--flat`, `--outline`.

**Acceptance:** Find `*.ts` files with `--depth 2`. Verify both tree and flat output formats.

---

### Step 1.8: `prx exists`

Bloom filter built from identifier tokens. Check query, return `{ exists, confidence }`.

**Acceptance:** Check a known-present and a known-absent pattern. Verify correct confidence values.

---

### Step 1.9: `prx outline`

Standalone wrapper around `parsing/outline.rs`. For directories: recurse with `--depth`.

**Acceptance:** Outline a Python file. Verify symbols match a hand-checked expected list.

---

## Phase 2: Edit, Diff, Integration (Weeks 6-8)

### Step 2.1: `prx edit`

Full implementation per SYSTEM-DESIGN.md section 14. Literal match default, `--regex` opt-in, `--dry-run` default. `--in-function` scoping via tree-sitter. Syntax validation before write. Multi-edit: `--find`/`--replace` multiple times.

**Acceptance:** Dry-run edit, verify before/after in output. Then `--apply` and verify file changed on disk.

---

### Step 2.2: `prx diff`

Full implementation per SYSTEM-DESIGN.md section 15. Uses `similar` crate for diff computation, tree-sitter for function attribution. Heuristic summary generation. Flags: `--stat-only`, `--functions`, `--budget`.

**Acceptance:** Make a change, run `prx diff`, verify summary describes the change accurately.

---

### Step 2.3: `prx mcp`

MCP server over stdio via `rmcp` crate. Exposes: `search`, `read`, `find`, `edit`, `diff`, `exists`, `outline`. Each tool reuses the CLI command handler via shared structs. In-memory index cache (LRU 10).

**Acceptance:** Start MCP server, send a JSON-RPC search request, verify well-formed response.

---

### Step 2.4: `prx index`

Persistent index to `.prx/index/`: `chunks.bin`, `dense.bin`, `sparse.bin`, `bloom.bin`, `meta.json`. `--watch` via `notify` crate. Index validation: version + file hash checks on load.

**Acceptance:** Build index, modify a file, re-run search, verify updated results appear.

---

### Step 2.5: `prx batch`

Read JSONL from stdin. Parallel execution via `std` threads. Results returned in input order.

**Acceptance:** Batch 3 commands (search + read + exists). Verify all 3 results present and ordered correctly.

---

### Step 2.6: `prx stats`

All other commands append to `~/.prx/stats.jsonl` (fire-and-forget). `prx stats` reads the file and aggregates by period.

**Acceptance:** Run 5 searches, verify `prx stats` shows 5 calls.

---

### Step 2.7: `prx init`

Auto-detect frameworks, generate configs from embedded templates. `--agents-md` appends a snippet to `AGENTS.md`.

**Acceptance:** Run `prx init` in a directory containing `.cursor/`. Verify `.cursor/mcp.json` written with correct content.

---

### Step 2.8: `prx run`

Structured command runner. See PRX-RUN.md and PRX-RUN-DESIGN.md for full spec.

**Files:**
- `src/commands/run.rs` — CLI args, orchestration
- `src/runner/mod.rs` — subprocess execution, tool detection, dispatch
- `src/runner/cargo_test.rs` — cargo test output parser
- `src/runner/cargo_build.rs` — cargo build/clippy output parser
- `src/runner/pytest.rs` — pytest output parser
- `src/runner/go_test.rs` — go test output parser
- `src/runner/jest.rs` — jest/npm test/vitest output parser
- `src/runner/tsc.rs` — TypeScript compiler output parser
- `src/runner/eslint.rs` — eslint output parser
- `src/runner/fallback.rs` — unknown command fallback

**Implementation order:**

1. Runner framework: spawn subprocess, capture stdout+stderr, timeout watchdog
2. Tool detection: match command string against known patterns
3. Fallback parser: exit code + last N lines
4. `cargo test` parser:
   - Summary: `test result: (ok|FAILED). N passed; M failed; I ignored`
   - Failures: extract between `---- name stdout ----` and `note:`
   - Skip `test name ... ok` lines entirely
5. `cargo build`/`clippy` parser:
   - Errors: `error[EXXXX]: message` + `  --> file:line:col`
   - Warnings: `warning[CODE]: message` + `  --> file:line:col`
   - Summary: `Finished` or `could not compile`
6. `pytest` parser:
   - Summary: `=== N passed, M failed in Xs ===`
   - Failures: `FAILED path::test - message`
7. `go test` parser:
   - Package: `ok|FAIL package Xs`
   - Failures: `--- FAIL: TestName` + indented message lines
8. `jest`/`vitest` parser (shared module — vitest is jest-compatible):
   - Summary: `Tests: N failed, M passed, K total`
   - Failures: `● test name` + expect/received block + `at` location
   - Vitest variant: `Tests N failed | M passed (K)`
9. `tsc` parser:
   - Errors: `file(line,col): error TSXXXX: message`
   - No summary line — count from individual errors
10. `eslint` parser:
    - Per-file header + indented `line:col error|warning message rule`
    - Summary: `✖ N problems (E errors, W warnings)`
11. Wire `prx run` into CLI dispatch, add to Commands enum
12. Integration test: `prx run cargo test` on own project

**Acceptance:** Run `prx run cargo test` on this project. Verify JSON output has correct pass/fail counts, failure details (if any), and token savings > 90%.

---

## Phase 3: Polish, Benchmark, Release (Weeks 9-12)

### Step 3.1: Cross-platform CI

GitHub Actions matrix: `linux x86_64`, `linux aarch64` (cross), `macos arm64`, `macos x86_64`, `windows msvc`. Each job runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo build --release`.

---

### Step 3.2: Benchmarks

Implement benchmarks per BENCHMARKS.md. NDCG@10 benchmark suite. Token efficiency measurement. Latency profiling with `hyperfine`.

---

### Step 3.3: Binary Optimization

LTO, strip, `codegen-units=1` (already in `Cargo.toml`). Verify binary size ~47MB with embedded model. Test float16 model precision vs float32.

---

### Step 3.4: Distribution

- GitHub releases with prebuilt binaries (linux, macos, windows)
- `cargo install prx`
- Homebrew formula
