# prx Roadmap

## v0.1.0 — RELEASED

All phases complete. Released at https://github.com/civitas-io/prx/releases/tag/v0.1.0

### Phase 0 — Foundation [DONE]

| Deliverable | Status |
|---|---|
| Project scaffold (Cargo, CI, clippy/fmt) | Done |
| Tree-sitter integration (14 grammars, chunking, AST parsing) | Done |
| Model2Vec inference (pure Rust, safetensors + ndarray, float16) | Done |
| BM25 implementation (compound identifier tokenization, CSC sparse matrix) | Done |
| JSON/JSONL output framework | Done |
| Token counting (cl100k_base, fast + exact modes) | Done |
| Content hashing (xxh3) | Done |
| File walking (ignore crate, .prxignore) | Done |

### Phase 1 — Core Tools [DONE]

| Command | Status |
|---|---|
| `prx search` (literal + semantic + structural, RRF fusion, 5-stage reranking) | Done |
| `prx read` (--lines, --snap, --skeleton, --outline, --hash, --budget) | Done |
| `prx find` (tree+flat, --pattern, --depth, --changed-since, --related-to) | Done |
| `prx exists` (bloom filter O(1)) | Done |
| `prx outline` (file + directory mode) | Done |
| Search auto-detection (literal vs semantic vs structural) | Done |
| Continuation tokens for pagination | Done |
| Budget enforcement | Done |

### Phase 2 — Edit, Diff, Integration [DONE]

| Command | Status |
|---|---|
| `prx edit` (literal/regex, dry-run, --apply, --in-function, syntax validation) | Done |
| `prx diff` (git diff, function attribution, semantic notes, --stat-only) | Done |
| `prx run` (9 parsers: cargo test/build/clippy, pytest, go test, jest/vitest, tsc, eslint) | Done |
| `prx index` (persistent to .prx/index/, --rebuild, --stats, --watch) | Done |
| `prx batch` (JSONL stdin dispatch) | Done |
| `prx stats` (token savings dashboard, PRX_STATS_FILE env) | Done |
| `prx init` (AGENTS.md snippet, cursor/codex/opencode/claude-code configs) | Done |
| `prx mcp` (MCP server over stdio, 6 tools) | Done |

### Phase 3 — Polish, Benchmark, Release [DONE]

| Area | Status |
|---|---|
| Cross-platform CI (Linux, macOS, Windows) | Done |
| Float16 model conversion (77MB -> 48MB binary) | Done |
| Model2Vec vocabulary loading (real tokenizer, 61,826 tokens) | Done |
| GitHub Actions release pipeline (5 targets) | Done |
| Apache 2.0 license | Done |
| Documentation (21 docs, ~5,000 lines) | Done |
| 300 tests (256 unit + 44 E2E), 84% coverage | Done |

---

## v0.1.0 Stats

| Metric | Value |
|---|---|
| Commands | 13 |
| Tests | 300 |
| Coverage | 84% |
| Languages | 14 (tree-sitter grammars) |
| Release binary | ~48 MB |
| Tool parsers (prx run) | 9 |
| Repository | https://github.com/civitas-io/prx |

---

## v0.1.1 — Reliability [DONE]

| Item | Status |
|---|---|
| Graceful fallback | Done — catch_unwind + fallback to grep/cat/find on internal errors |
| Error logging | Done — `~/.prx/errors.jsonl` captures every fallback |
| Real-world telemetry | Done — `prx stats --compare` shows per-command savings |
| Synthetic benchmarks | Done — `prx bench` runs side-by-side comparisons |
| Pre-commit hook | Done — mirrors CI checks (fmt + clippy + tests) |

## v0.2.0 — Context Intelligence

Informed by LeanCTX research. Adopt the best techniques, keep the prx philosophy
(native structured output, not post-hoc compression).

### Session & Caching

| Item | Priority | Description | Inspired by |
|---|---|---|---|
| Session cache | High | Track file hashes per session. Re-reads of unchanged files return ~13-token cache-hit response instead of full content. Eliminates 50% of file read tokens (SWE-bench data). | LeanCTX `full` mode cache |
| File reference IDs | Medium | Assign sequential IDs (F1, F2...) to files in a session. Accept `F1` as path alias in subsequent commands. Saves ~10 tokens per reference. | LeanCTX structured headers |

### Read Modes

| Item | Priority | Description | Inspired by |
|---|---|---|---|
| `--mode aggressive` | High | Strip comments + whitespace, keep all functional code. 20-40% savings on verbose files. | LeanCTX aggressive mode |
| `--mode diff` | High | Only return lines changed since last read (via hash comparison with session cache). 80-97% savings on re-reads. | LeanCTX diff mode |
| `--mode entropy` | Medium | Shannon entropy scoring + Jaccard similarity dedup. Filters repetitive low-information lines. 60-85% savings on generated files (schemas, protobuf, OpenAPI specs). | LeanCTX entropy mode |
| Auto mode for read | Medium | Auto-select best read mode based on file size, type, and cache state. | LeanCTX auto/smart_read |

### Search Improvements

| Item | Priority | Description | Inspired by |
|---|---|---|---|
| Graph proximity boost | High | Build lightweight import graph from `use`/`import`/`require` statements. Boost search results that are in the dependency neighborhood of top results. | LeanCTX graph-aware RRF |
| MMR diversity | Low | Maximal Marginal Relevance in reranking to reduce redundant results from same cluster. Principled alternative to our saturation decay. | LeanCTX reranking |

### Distribution

| Item | Priority | Description |
|---|---|---|
| `cargo publish` | High | Publish to crates.io for `cargo install prx` |
| Homebrew formula | High | `brew install civitas-io/tap/prx` |
| Benchmarks (NDCG@10) | High | Head-to-head quality measurement vs ripgrep, Semble |
| More run parsers | Medium | bun test, deno test, dotnet test, ruff |
| Additional grammars | Medium | Kotlin, Swift, C#, PHP, Elixir |

## v0.3.0 — Project Intelligence

| Item | Priority | Description |
|---|---|---|
| `prx context` | High | Assemble context packages ("everything about module X") — combine search + read + outline into one call |
| `prx impact` | High | Reverse dependency analysis ("what breaks if I change X?") using the import graph |
| `prx deps` | Medium | Import and dependency graph visualization |
| `prx blame` | Medium | Structured git blame per function |
| `prx test` | Medium | Test discovery related to functions/files |
| Bayesian mode predictor | Low | Learn optimal read mode per file signature over time |
| Information bottleneck filter | Low | Task-conditioned line filtering for task-driven reads |
| Custom embeddings | Low | Support for user-provided or fine-tuned models |

---

## Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All breaking
changes are documented in CHANGELOG.md with migration guides. JSON output includes
a `version` field for programmatic detection.
