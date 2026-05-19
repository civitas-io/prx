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

| Item | Priority | Status | Description |
|---|---|---|---|
| `--if-changed HASH` | High | **Done** | Stateless conditional read. Agent passes previous hash, gets 48-token stub if unchanged. 99% reduction on re-reads. |
| File reference IDs | Medium | Planned | Assign sequential IDs (F1, F2...) to files in a session. Accept `F1` as path alias. |

### Read Modes

| Item | Priority | Status | Description |
|---|---|---|---|
| `--mode aggressive` | High | **Done** | Tree-sitter comment stripping + blank line collapse. 1-19% savings (real-world: tested on fiddler). |
| `--mode diff` | High | **Done** | Changed lines vs git HEAD only. 80-97% savings on modified files. |
| `--mode entropy` | Medium | **Done** | Pattern-based repetitive line filter. 5-87% savings (86% on generated structs). |
| Auto mode for read | Medium | Planned | Auto-select best read mode based on file size, type, and cache state. |

### Search Improvements

| Item | Priority | Status | Description |
|---|---|---|---|
| Graph proximity boost | High | **Done** | Import graph from 7 languages via regex. BFS 2-hop neighborhood. 0.25x additive boost with hop decay. Persisted to imports.bin. |
| MMR diversity | Low | Planned | Maximal Marginal Relevance in reranking. |

### Distribution

| Item | Priority | Status | Description |
|---|---|---|---|
| `cargo publish` | High | Planned | Publish to crates.io for `cargo install prx` |
| Homebrew formula | High | Planned | `brew install civitas-io/tap/prx` |
| Benchmarks (NDCG@10) | High | Planned | Head-to-head quality measurement vs ripgrep, Semble |
| More run parsers | Medium | Planned | bun test, deno test, dotnet test, ruff |
| Additional grammars | Medium | Planned | Kotlin, Swift, C#, PHP, Elixir |

### v0.2.0 Stats

| Metric | Value |
|---|---|
| Tests | 353 (304 unit + 49 E2E) |
| New modules | 3 (imports.rs, graph.rs, proximity.rs) |
| New features | 5 (--if-changed, 3 read modes, proximity boost) |
| LOC added | ~1,400 |

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
