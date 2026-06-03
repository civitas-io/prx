# Roadmap

## v0.1.0 — RELEASED

All phases complete. Released at https://github.com/civitas-io/prx/releases/tag/v0.1.0

### Phase 0 — Foundation

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

### Phase 1 — Core Tools

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

### Phase 2 — Edit, Diff, Integration

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

### Phase 3 — Polish, Benchmark, Release

| Area | Status |
|---|---|
| Cross-platform CI (Linux, macOS, Windows) | Done |
| Float16 model conversion (77MB → 48MB binary) | Done |
| Model2Vec vocabulary loading (real tokenizer, 61,826 tokens) | Done |
| GitHub Actions release pipeline (5 targets) | Done |
| Apache 2.0 license | Done |
| Documentation (21 docs, ~5,000 lines) | Done |
| 300 tests (256 unit + 44 E2E), 84% coverage | Done |

### v0.1.0 Stats

| Metric | Value |
|---|---|
| Commands | 13 |
| Tests | 300 |
| Coverage | 84% |
| Languages | 14 (tree-sitter grammars) |
| Release binary | ~49 MB |
| Tool parsers (prx run) | 9 |

---

## v0.1.1 — Reliability — RELEASED

| Item | Status |
|---|---|
| Graceful fallback (catch_unwind + fallback to grep/cat/find on internal errors) | Done |
| Error logging (`~/.prx/errors.jsonl` captures every fallback) | Done |
| Real-world telemetry (`prx stats --compare` shows per-command savings) | Done |
| Synthetic benchmarks (`prx bench` runs side-by-side comparisons) | Done |
| Pre-commit hook (mirrors CI checks: fmt + clippy + tests) | Done |

---

## v0.2.0 — Context Intelligence — RELEASED

### Session and Caching

| Item | Status | Description |
|---|---|---|
| `--if-changed HASH` | Done | Stateless conditional read. Agent passes previous hash, gets 48-token stub if unchanged. 99% reduction on re-reads. |
| File reference IDs | Planned | Assign sequential IDs (F1, F2...) to files in a session. Accept `F1` as path alias. |

### Read Modes

| Item | Status | Description |
|---|---|---|
| `--mode aggressive` | Done | Tree-sitter comment stripping + blank line collapse. 1-19% savings. |
| `--mode diff` | Done | Changed lines vs git HEAD only. 80-97% savings on modified files. |
| `--mode entropy` | Done | Pattern-based repetitive line filter. 5-87% savings (86% on generated structs). |
| Auto mode for read | Planned | Auto-select best read mode based on file size, type, and cache state. |

### Search Improvements

| Item | Status | Description |
|---|---|---|
| Graph proximity boost | Done | Import graph from 7 languages via regex. BFS 2-hop neighborhood. 0.25x additive boost with hop decay. Persisted to imports.bin. |
| MMR diversity | Planned | Maximal Marginal Relevance in reranking. |

### v0.2.0 Stats

| Metric | Value |
|---|---|
| Tests | 353 (304 unit + 49 E2E) |
| New modules | 3 (imports.rs, graph.rs, proximity.rs) |
| New features | 5 (--if-changed, 3 read modes, proximity boost) |

---

## v0.3.0 — Reliability and Search Quality — RELEASED

### Reliability

| Item | Status | Description |
|---|---|---|
| MCP server E2E tests | Done | 8 E2E tests covering initialize, tools/list, tools/call for all 6 MCP tools. |
| Incremental indexing | Done | Skip unchanged files via hash comparison. Reports files_changed/files_unchanged. |
| Real criterion benchmarks | Done | 5 search benchmarks + 3 chunking benchmarks. |
| NDCG@10 measurement | Done | 50-query labeled dataset on prx (NDCG@10=0.639) + 49-query dataset on external production codebase (NDCG@10=0.451). |
| Structural search validation | Done | Warns when pattern compiles but matches 0 files, or when pattern fails to compile for all languages. |

### Search Quality

Measured NDCG@10: 0.639 (self), 0.451 (external production codebase). Target: 0.70+ on unfamiliar codebases.

| Item | Status | Description |
|---|---|---|
| Symbol-query ranking overhaul | Done | 12x definition boost for symbol queries, import-line penalty (0.2x), improved definition detection for Python/TS. |
| Chunk header enrichment | Done | BM25 enrichment now prepends `[lang] file_path stem_tokens` to each chunk. |
| Persistent dense index | Done | Embeddings computed at index time, stored as `embeddings.bin`. |
| Sharper mode detection | Done | Symbol queries: alpha=0.1 (near-pure BM25). NL queries: alpha=0.6. Static synonym dict (18 pairs). |
| Reranker weight tuning | Done | Definition boost 3→4 (NL), 8→12 (symbol). Stem match 1.0→1.5. |
| Chunk overlap | Done | 200-byte overlap between chunks, snapped to line boundaries. |
| Embedding model upgrade | Done | Evaluated 3 models: potion-retrieval-32M selected (+7% NDCG). |
| Symbol index | Done | Map each symbol to definition location + reference count. Symbol NDCG: 0.263 → 0.619. |

---

## v0.4.0 — Run Parsers and Project Intelligence — RELEASED

### Run Parsers

10 new parsers implemented. Total: 22 parsers.

| Parser | Tool | Status |
|---|---|---|
| terraform | `plan`, `apply` | Done |
| kubectl | `describe`, `get` | Done |
| kubectl-logs | `logs` (+ docker logs) | Done |
| docker-build | `build` | Done |
| mvn | `test`, `build` | Done |
| gradle | `build`, `test` | Done |
| dotnet | `test`, `build` | Done |
| mypy | type check | Done |
| npm-ls | `npm list` | Done |
| git-log | `log` | Done |
| pytest-cov | `pytest --cov`, `coverage report` | Done |
| go-cover | `go test -cover` | Done |
| jest-cov | `jest --coverage`, `c8` | Done |

### Project Intelligence

| Item | Status | Description |
|---|---|---|
| `prx context` | Done | Assemble context packages — search + read + outline in one call |
| `prx impact` | Done | Reverse dependency analysis using the import graph |

### Security CI

| Item | Status |
|---|---|
| `cargo audit` in CI | Done |
| `cargo deny` in CI | Done |

---

## v0.5.x — Current Development

### v0.5.0 — Features

| Item | Status | Description |
|---|---|---|
| `prx run --auto-json` | Done | Auto-inject `--json` flags for tools with structured output. |
| Tree-sitter import extraction | Done | Replace regex imports with tree-sitter AST queries. |
| Import language coverage | Done | bash, CSS, HTML import extraction added. |

### v0.5.1 — Build and Security

| Item | Status | Description |
|---|---|---|
| Self-contained build (`build.rs`) | Done | `cargo build` works without `make models` or Python. SHA-256 pinned artifacts. |
| Migrate off bincode | Done | Replace bincode (RUSTSEC-2025-0141) with postcard for all index serialization. |

### v0.5.4 — Lean-Down Refactoring

| Item | Status | Description |
|---|---|---|
| `define_regex!` macro | Done | Reduce 3-line `LazyLock<Regex>` statics to 1-line macro calls across 22 parsers. ~130 lines saved. |
| `ParsedResult::new()` constructor | Done | Replace 10-line struct literals with 1-line constructor calls across 22 parsers. ~200 lines saved. |
| Extract `src/workspace.rs` | Done | Deduplicate `find_workspace_root()`, `relative_path()`, `is_test_file()`. ~73 lines saved. |

### v0.5.5 — Index Performance and Test Coverage (Current)

| Item | Priority | Status | Description |
|---|---|---|---|
| Parallel embedding (rayon) | High | Done | Embed chunks in parallel during indexing. ~300s → ~100s on 4-core for 55k chunks. |
| Parallel chunking | High | Done | Parse and chunk files in parallel during indexing. |
| Parallel import extraction | Medium | Done | Extract imports per-file in parallel during `ImportGraph::build_full`. |
| E2E coverage for search.rs | High | In progress | Cover hybrid/semantic search paths (47.6% → 80%+). |
| E2E coverage for mcp.rs | High | In progress | Cover remaining MCP tool paths (51.4% → 80%+). |
| E2E coverage for run.rs | Medium | Planned | Cover external command execution paths (63.1% → 80%+). |
| E2E coverage for init.rs | Medium | Planned | Cover config generation paths (59.8% → 80%+). |
| Test helpers (`tests/helpers/`) | Medium | Planned | Extract `run_prx()`, `test_dir()` helpers. ~300 lines saved. |

---

## v0.5.6 — Memory-Mapped Index

| Item | Priority | Description |
|---|---|---|
| Memory-mapped index files | High | Use mmap instead of read-to-vec for chunks.bin, bm25.bin, embeddings.bin. OS handles caching — index stays in memory across queries. |
| `bench-ndcg --plain` | Medium | Human-readable table output for terminal use. |
| `bench-ndcg` load-once | Medium | Load index once, query N times. |

---

## v0.5.7 — Public Benchmark Suite

| Item | Priority | Description |
|---|---|---|
| Query generation for 8 pinned repos | High | 25 labeled queries per repo (flask, ripgrep, fastify, cargo, django, kafka, terraform, vscode). 200 total queries across 6 languages, 3 size tiers. |
| `benchmark.yml` CI workflow | High | Clone repos at pinned SHAs, build index, run NDCG, compare to baseline, fail on regression >0.05. |
| Results dashboard | Medium | `benchmarks/results/` with per-release JSON. |
| Expand to 40-50 queries per repo | Medium | 25 queries gives ±0.05-0.08 standard error. 40-50 narrows to ±0.03, enabling tighter CI gate. |

**Repository matrix:**

| Size | Repo | Language | LOC |
|---|---|---|---|
| Small | `pallets/flask` | Python | 15K |
| Small | `BurntSushi/ripgrep` | Rust | 25K |
| Small | `fastify/fastify` | TypeScript | 15K |
| Medium | `rust-lang/cargo` | Rust | 150K |
| Medium | `django/django` | Python | 300K |
| Medium | `apache/kafka` | Java | 500K |
| Large | `hashicorp/terraform` | Go | 2M |
| Large | `microsoft/vscode` | TypeScript | 1M |

---

## v0.5.8 — Documentation Site [DONE]

| Item | Priority | Status |
|---|---|---|
| Documentation site (mdBook) | **High** | **Done** — 33 pages at `civitas-io.github.io/prx/`. |
| deploy-docs.yml workflow | **High** | **Done** — auto-deploy on push to main. |
| Docs cleanup | Medium | **Done** — book/ is single source of truth, docs/ archived. |

## v0.5.9 — Distribution [DONE]

| Item | Priority | Status |
|---|---|---|
| `cargo publish` | **High** | **Done** — [crates.io/crates/prx](https://crates.io/crates/prx). `cargo install prx`. |
| Homebrew formula | **High** | **Done** — `brew install civitas-io/tap/prx`. Tap: [civitas-io/homebrew-tap](https://github.com/civitas-io/homebrew-tap). |
| build.rs OUT_DIR fix | **High** | **Done** — models download to OUT_DIR, crate is 171 KB compressed. |
| npm wrapper | Medium | Deferred — `npx prx` for JS/TS agents. |
| pip wrapper | Medium | Deferred — `pip install prx` for Python agents. |

## v0.5.10 — Additional Grammars [DONE]

Added 12 new tree-sitter grammars, bringing the total to 27 languages compiled into the binary. Import extraction expanded from 10 to 20 language families.

Languages added: Kotlin (tree-sitter-kotlin-sg), Swift, C#, PHP, Elixir, YAML, TOML (tree-sitter-toml-ng), Markdown (tree-sitter-md), Dockerfile (tree-sitter-containerfile), HCL/Terraform, SQL (tree-sitter-sequel), Makefile.

All new grammars target tree-sitter 0.26.x via the `LanguageFn` API. Compatible forks used where upstream crates don't yet support 0.26.

## v0.5.11 — Fixes [DONE]

Windows CI fix: resolved grammar compilation failure on `windows-latest` GitHub Actions runner. Homebrew formula updated to v0.5.11.

## v0.5.12 — Documentation Sweep [DONE]

Updated all docs (roadmap, architecture, search, platforms, dependencies, SVGs) to reflect 27 languages, tree-sitter 0.26, 20 import families.

## v0.5.13 — Correctness Fixes

P0 bugs from code review. Each fix is independently testable.

| ID | Type | Effort | Description | Status |
|---|---|---|---|---|
| C1 | bug | S | `saturation_decay` truncates by pre-decay rank, evicting higher un-selected chunks | |
| C2 | bug | S | `diff` changed-function attribution off-by-N — ignores `Delete` tags | |
| C3 | bug | M | Three divergent extension→language tables (`languages.rs` × 2, `walk.rs`) | |
| C4 | bug | S | Dead CLI flags silently accepted: `search --context`, `search --exists`, `read --meta` | |
| C5 | bug | M | Two different `find_workspace_root` semantics in `context` vs `impact` | |
| C6 | bug | S | `mmap` size check can integer-overflow on corrupt meta → panic | |
| C7 | bug | S | `go_cover` reports unweighted mean across packages | |
| C8 | bug | S | JS/TS outline floods with noise — every top-level `const` emitted | |

## v0.5.14 — Performance (P1 Scale Fixes) [DONE]

Parallel embedding indexing via rayon. O(n) top-k selection replacing O(n log n) full sorts. Find tree builder O(n²) → O(n). Chunk line numbering O(n²) → O(n log n). BM25 df and symbol reference counting optimized.

---

## v0.6.0 — Codebase Health + Benchmark Infrastructure

### Model Tiering — Evaluated, Deferred

Benchmark data (v0.5.7) showed the builtin model degrades on large repos.
We hypothesized code-specific models would help. A comprehensive evaluation
(June 2026) tested 5 candidate models distilled to Model2Vec against the
builtin across 8 repos / 280 queries:

| Model | Dim | MEAN NDCG@10 | vs builtin |
|---|---|---|---|
| **builtin** (potion-retrieval-32M) | 64 | **0.384** | baseline |
| potion-code-16M | 256 | 0.377 | -1.8% |
| codexembed-400m (Salesforce) | 256 | 0.362 | -5.8% |
| codexembed-2b (Salesforce) | 256 | 0.360 | -6.4% |
| jina-code-v2 (Jina AI) | 256 | 0.352 | -8.3% |

**Result: builtin wins. No candidate cleared the 5% lift threshold.**
The general-retrieval training distribution (MS-MARCO) generalizes better
to mixed prx queries than narrow code-specific training. The 6-stage
reranking pipeline already encodes code structure, making code-aware
embeddings redundant. Model tiering infrastructure (--model flag, download,
ModelTier registry) is in place but models are deferred pending Tier 8
(task-specific training) in v0.7.0.

Full analysis: `docs/internal/SEARCH-QUALITY.md`

### Benchmark Infrastructure

| Item | Priority | Status | Description |
|---|---|---|---|
| `bench-ndcg --model-path` | **High** | Done | Load external Model2Vec model and re-embed chunks at bench time. |
| Expand benchmark to 45 queries per repo | **High** | Done | All 8 repos at 45 queries each. 360 total. Bootstrap 95% CIs. Hard-negative support. |
| Model evaluation scripts | **High** | Done | `scripts/distill_eval_models.py`, `scripts/run_model_eval.py`. |
| Measured savings baselines (P1-1) | Medium | Done | `prx bench --plain` produces measured table. README savings sourced from real runs. |

### Codebase Simplification (remaining P2)

| Item | Effort | Status | Description |
|---|---|---|---|
| P2-1: Budget helper | M | Done | `budget::retain_within()` — unified budget enforcement. |
| P2-3: Runner parser helpers | M | Done | Extract `ParsedResult::diagnostics()` and `try_json()`. |
| P2-4: Symbol-tree flattener | M | Done | Unify 4 recursive flatteners into one shared helper. |
| P2-5: Git utils module | S | Done | `git show` + path-relativization unified. |
| P2-6: Default derives | S | Done | `..Default::default()` on Args structs. |

---

## v0.7.0 — Search Quality

Target: +15-25% NDCG@10 lift (0.384 → 0.45-0.50) through pipeline improvements. The v0.6.0 model evaluation proved that off-the-shelf embedding swaps yield <2% gain — the leverage is in fusion, recall, chunking, and query understanding.

### Tier 5: Benchmark Hardening (prerequisite)

| Item | Priority | Effort | Description |
|---|---|---|---|
| Bootstrap CIs on mean NDCG@10 | **High** | S | Quantify noise so we can detect real lifts vs statistical artifacts. |
| Expand to 40-50 queries per repo | **High** | M | 280→360+ queries. Narrow standard error from ±0.04 to ±0.03. |
| Per-language / per-category CI gate | **High** | S | Prevent silent regressions in specific query types or languages. |
| Hard-negative queries | Medium | S | Queries that should retrieve nothing — catches over-retrieval. |

### Tier 6: Learned-to-Rank Fusion

| Item | Priority | Effort | Description |
|---|---|---|---|
| Extract reranker features per query-chunk pair | **High** | M | BM25 score, cosine, def_match, stem_match, coherence, proximity, is_symbol, chunk_length, language, file_depth, import_distance. |
| Train LightGBM with LambdaMART (NDCG objective) | **High** | M | Replace hand-tuned weights with learned fusion. Cross-validated on benchmark. |
| Ship as ~100KB JSON, pure-Rust forest eval | **High** | M | Zero new runtime deps. Tree evaluation in `ranking/`. |

### Tier 7: Multi-Field BM25 + Dual Vectors

| Item | Priority | Effort | Description |
|---|---|---|---|
| BM25F: weighted fields (identifiers, path, signatures, body, comments) | **High** | L | Stop diluting identifier IDF with path tokens. |
| Dual dense vectors per chunk (signature vs body) | Medium | M | Recover signal lost when mean-pooling 1500-char chunks. Score = max(sig_cos, body_cos). |
| AST-extract signatures as separate mini-chunks | Medium | M | Compounds with BM25F — high-IDF function/class names get their own index entries. |

### Tier 8: Task-Specific Static Embeddings

| Item | Priority | Effort | Description |
|---|---|---|---|
| Mine training pairs from indexed corpora | Medium | L | Docstrings, commit messages, file names → chunk pairs. |
| Train Model2Vec via tokenlearn on (query, code) pairs | Medium | L | Skip transformer distillation. Train directly for prx query distribution. |
| Validate on held-out human-labeled queries | Medium | M | Never train on eval set. Guard against synthetic overfitting. |

### Tier 9: Cross-Encoder Reranker

| Item | Priority | Effort | Description |
|---|---|---|---|
| Integrate ~30M cross-encoder for top-20 reranking | Low | L | Highest single-step gain (+5-10%) but adds download-on-demand model. |
| Download-on-first-use to `~/.prx/models/reranker/` | Low | M | Keep binary at 49MB. |

**Ceiling:** Tiers 5-7 target 0.45-0.50. Tier 8 may reach 0.50-0.55. Tier 9 needed for 0.55-0.60.

---

## v0.6.1 — Agent Primitives

New commands and modes that reduce multi-step agent workflows to single calls.

| Item | Effort | Description |
|---|---|---|
| Persistent `exists` | M | Wire bloom filter to persisted index instead of rebuilding from scratch every call. |
| `--no-fallback` strict mode | M | Retrieval-sensitive agents fail loud instead of silently degrading to plain matching. |
| Surface fallback flag | S | Make it obvious in output when a fallback occurred. |
| `prx context` git-aware budgeting | S | Prioritize recently-changed files in the module package. |
| NDCG regression gate in CI | M | Search quality can't silently regress between releases. Tighten threshold from 0.05 to 0.02. |

---

## v0.6.2 — Advanced Agent Features

Higher-level primitives that compose existing infrastructure into new capabilities.

| Item | Effort | Description |
|---|---|---|
| `prx explain <symbol>` | L | One call for definition + callers + tests. Agents currently stitch search→read→impact. |
| Cross-file rename | L | Multi-edit transaction built on import graph. AST-validated, multi-file. |

---

## v0.6.3 — Distribution Automation [DONE]

| Item | Effort | Status | Description |
|---|---|---|---|
| Homebrew SHA automation | M | Done | `update-homebrew` job in release.yml auto-pushes formula to homebrew-tap. |
| `cargo publish` automation | S | Done | `publish-crate` job in release.yml auto-publishes to crates.io. |
| npm wrapper | M | Deferred | `npx prx` for JS/TS agents. No demand yet. |
| pip wrapper | M | Deferred | `pip install prx` for Python agents. No demand yet. |

---

## Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All breaking changes are documented in CHANGELOG.md with migration guides. JSON output includes a `version` field for programmatic detection.
