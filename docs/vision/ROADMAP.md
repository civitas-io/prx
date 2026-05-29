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
| Release binary | ~49 MB |
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
| `--mode aggressive` | High | **Done** | Tree-sitter comment stripping + blank line collapse. 1-19% savings (real-world: tested on external codebase). |
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

## v0.3.0 — Reliability & Search Quality

### Reliability & Testing [DONE]

| Item | Priority | Status | Description |
|---|---|---|---|
| MCP server E2E tests | **High** | **Done** | 8 E2E tests covering initialize, tools/list, tools/call for all 6 MCP tools, plus invalid tool error handling. |
| Incremental indexing | **High** | **Done** | Skip unchanged files via hash comparison. Reports files_changed/files_unchanged. Walker now excludes `.prx/` directory. |
| Real criterion benchmarks | **High** | **Done** | 5 search benchmarks (BM25 build/query, literal search, index build, incremental noop) + 3 chunking benchmarks (Rust/Python/plaintext at multiple sizes). |
| NDCG@10 measurement | **High** | **Done** | 50-query labeled dataset on prx (NDCG@10=0.639) + 49-query dataset on external production codebase (NDCG@10=0.451). See `docs/design/SEARCH-QUALITY.md`. |
| Structural search validation | Medium | **Done** | Warns when pattern compiles but matches 0 files, or when pattern fails to compile for all languages. |

### Search Quality — Closing the Gap

Measured NDCG@10: 0.639 (self), 0.451 (external production codebase). Target: 0.70+ on unfamiliar
codebases. Full analysis and plan in `docs/design/SEARCH-QUALITY.md`.

**Tier 1 — Structural fixes [DONE]:**

| Item | Status | Description |
|---|---|---|
| Symbol-query ranking overhaul | **Done** | 12x definition boost for symbol queries, import-line penalty (0.2x), improved definition detection for Python/TS. |
| Chunk header enrichment | **Done** | BM25 enrichment now prepends `[lang] file_path stem_tokens` to each chunk. Split identifiers indexed as separate tokens. |
| Persistent dense index | **Done** | Embeddings computed at index time, stored as `embeddings.bin`. Loaded at query time for independent semantic retrieval. |

**Tier 2 — Tune and expand [DONE]:**

| Item | Status | Description |
|---|---|---|
| Sharper mode detection | **Done** | Symbol queries: alpha=0.1 (near-pure BM25). NL queries: alpha=0.6. Static synonym dict (18 pairs: auth→authentication, db→database, k8s→kubernetes, etc). Synonym expansion applied to BM25 queries. |
| Reranker weight tuning | **Done** | Definition boost 3→4 (NL), 8→12 (symbol). Stem match 1.0→1.5. Coherence 0.2→0.15. Import penalty 0.3→0.2. Configurable RerankConfig added for ablation. |
| Chunk overlap | **Done** | 200-byte overlap between chunks, snapped to line boundaries. |

**Measured improvement:** 7 previously-missed queries recovered. Remaining
misses are symbol queries requiring Tier 4.

**Tier 3 — Model upgrade [DONE]:**

| Item | Status | Description |
|---|---|---|
| Upgrade embedding model | **Done** | Evaluated 3 models: CodeMalt (-3%, rejected), potion-retrieval-32M (+7%, selected), Candle all-MiniLM-L6-v2 (rejected — Metal missing LayerNorm, CPU 46min index). |

**Tier 4 — Symbol index [DONE]:**

| Item | Status | Description |
|---|---|---|
| Symbol index with reference counting | **Done** | Map each symbol to definition location + reference count at index time. Direct lookup for symbol queries. Symbol NDCG: 0.263 → 0.619. 4 symbol misses recovered. |

## v0.4.0 — Run Parsers & Project Intelligence

### Public Benchmark Suite (CI-integrated)

Automated NDCG@10 measurement against real public repositories, run as a
GitHub Actions workflow before every release. Repos pinned by commit SHA.

**Repository matrix (finalized):**

| Size | Repo | Language | LOC | License |
|---|---|---|---|---|
| Small | `pallets/flask` | Python | 15K | BSD-3 |
| Small | `BurntSushi/ripgrep` | Rust | 25K | MIT |
| Small | `fastify/fastify` | TypeScript | 15K | MIT |
| Medium | `rust-lang/cargo` | Rust | 150K | MIT/Apache-2.0 |
| Medium | `django/django` | Python | 300K | BSD-3 |
| Medium | `apache/kafka` | Java | 500K | Apache-2.0 |
| Large | `hashicorp/terraform` | Go | 2M | BSL-1.1 |
| Large | `microsoft/vscode` | TypeScript | 1M | MIT |

8 repos, 6 languages, 3 size tiers. Selected based on knowing and Vera
benchmark suites. All pinned by commit SHA in `benchmarks/repos.json`.

**Per-repo benchmark:**
- 20-30 queries per repo (semantic, symbol, architecture mix)
- Ground truth: Claude-annotated with human spot-check (10%)
- Queries + ground truth checked into `benchmarks/repos/`

**Metrics collected:**
- NDCG@5, NDCG@10 per query category
- Index time, query p50/p95 latency
- Token efficiency (prx result tokens vs grep+cat baseline)

**CI integration:**
- `benchmark.yml` workflow triggered on release tags and `workflow_dispatch`
- Clones repos at pinned SHAs, builds index, runs queries
- Compares against baseline thresholds (NDCG regression > 0.02 = fail)
- Uploads results as release artifacts

| Item | Priority | Description |
|---|---|---|
| Benchmark repo selection + SHA pinning | **High** | Select 6-9 repos across 3 sizes x 3 languages, pin commit SHAs in `benchmarks/repos.json` |
| Query + ground truth generation | **High** | 20-30 labeled queries per repo, Claude-annotated, human-verified subset |
| `benchmark.yml` workflow | **High** | GitHub Actions: clone repos, build index, run NDCG, compare to baseline, upload artifacts |
| Regression gate | High | Block release if NDCG drops > 0.02 from baseline on any repo size category |
| Results dashboard | Medium | `benchmarks/results/` with per-release JSON, referenced from README |

### Symbol Index (Search Quality Tier 4) [DONE]

| Item | Status | Description |
|---|---|---|
| Symbol index with reference counting | **Done** | Map each symbol to definition location + reference count at index time. Direct lookup for symbol queries. Symbol NDCG: 0.263 → 0.619 (+135%). 4 previously-complete-miss symbol queries recovered. |

### Run Parsers [DONE — 10 shipped, 3 planned]

10 new parsers implemented. Each extracts only failures, warnings, and
summaries — dropping progress bars, cache hits, dependency resolution,
and verbose defaults. Design: `docs/design/RUN-PARSERS.md`.

| Parser | Tool | Status |
|---|---|---|
| terraform | `plan`, `apply` | **Done** |
| kubectl | `describe`, `get` | **Done** |
| kubectl-logs | `logs` (+ docker logs) | **Done** |
| docker-build | `build` | **Done** |
| mvn | `test`, `build` | **Done** |
| gradle | `build`, `test` | **Done** |
| dotnet | `test`, `build` | **Done** |
| mypy | type check | **Done** |
| npm-ls | `npm list` | **Done** |
| git-log | `log` | **Done** |
| pytest-cov | `pytest --cov`, `coverage report` | **Done** |
| go-cover | `go test -cover` | **Done** |
| jest-cov | `jest --coverage`, `c8` | **Done** |

Total parsers: 22 (9 original + 10 infra/devops + 3 coverage).

### Project Intelligence

| Item | Priority | Description |
|---|---|---|
| `prx context` | High | Assemble context packages ("everything about module X") — search + read + outline in one call |
| `prx impact` | High | Reverse dependency analysis ("what breaks if I change X?") using the import graph |
| `prx deps` | Medium | Import and dependency graph visualization |
| `prx blame` | Medium | Structured git blame per function (collapse same-SHA runs) |
| `prx test` | Medium | Test discovery related to functions/files |

### Intelligence Features

| Item | Priority | Status | Description |
|---|---|---|---|
| JSON output detection | High | **Done** | When user passes `--json`/`-o json` themselves, detect JSON response and parse structurally instead of regex. kubectl, terraform, npm, eslint. |
| Generic log noise filter | High | **Done** | Shipped inline in kubectl_logs parser (dedup repeated lines, keep ERROR/WARN context). Shared module deferred until more log parsers added. |
| Bayesian mode predictor | Low | Deferred | Learn optimal read mode per file signature over time |
| Information bottleneck filter | Low | Deferred | Task-conditioned line filtering for task-driven reads |
| Custom embeddings | Low | Deferred | Support for user-provided or fine-tuned models |

### Security CI

| Item | Priority | Description |
|---|---|---|
| `cargo audit` in CI | **High** | Check dependencies against RustSec advisory database on every PR. Fast, zero config. |
| `cargo deny` in CI | **High** | License compliance, duplicate dep detection, advisory checks. Superset of audit. |
| Clippy restriction lints | Medium | Enable additional security-relevant clippy lints beyond `-D warnings`. |
| Index deserialization fuzzing | Low | Fuzz `bincode::deserialize` on symbols.bin, imports.bin, chunks.bin to catch panic paths. |
| Path traversal tests | Low | Verify `prx edit --apply` and `prx read` reject paths outside workspace root. |

## v0.4.x — Patch Releases (Correctness & Quality)

From independent code review. Detailed plan: `docs/design/PATCH-PLAN.md`.

| Release | Issue | Fix | Priority |
|---|---|---|---|
| v0.4.1 | `is_valid` ignores new files | Walk tree in `is_valid()` to detect files not in hash map | **P2 — correctness** |
| v0.4.2 | Silent embedding degradation | Warn when `load_model()` fails; surface in `prx index` output | **P2 — observability** |
| v0.4.3 | Import resolution bail-out at >3 | Replace bail-out with proximity-based disambiguation | **P1 — quality** |
| v0.4.4 | Full embedding rebuild every run | Incremental embeddings: hash per chunk, re-embed only changed | **P2 — perf** |
| v0.4.5 | Doc/code inconsistencies | Align doc claims with actual implementation | **P3 — docs** |

## v0.5.0 — Features (new capabilities)

Detailed plan: `docs/design/V050-PLAN.md`.

| Item | Priority | Description |
|---|---|---|
| `prx run --auto-json` | **High** | Auto-inject `--json` flags for tools with structured output. kubectl, terraform, npm, eslint, mypy. |
| Tree-sitter import extraction | **High** | Replace regex imports with tree-sitter AST queries. Captures multi-line, aliased, re-export, dynamic forms. |
| Import language coverage | Medium | **Done** — bash, CSS, HTML import extraction added. |

## v0.5.1 — Improvements (build & security)

| Item | Priority | Description |
|---|---|---|
| Self-contained build (`build.rs`) | **High** | `cargo build` works without `make models` or Python. SHA-256 pinned artifacts. Offline via `PRX_MODELS_DIR`. |
| Migrate off bincode | **High** | Replace bincode (RUSTSEC-2025-0141) with postcard for all index serialization. |

## v0.5.4 — Lean-Down Refactoring

Code review and reduction pass. No behavior changes, no new features.
Design: `docs/design/LEAN-DOWN.md`.

| Item | Priority | Description |
|---|---|---|
| `define_regex!` macro | **High** | Reduce 3-line `LazyLock<Regex>` statics to 1-line macro calls across 22 parsers. ~130 lines saved. |
| `ParsedResult::new()` constructor | **High** | Replace 10-line struct literals with 1-line constructor calls across 22 parsers. ~200 lines saved. |
| Extract `src/workspace.rs` | **High** | Deduplicate `find_workspace_root()`, `relative_path()`, `is_test_file()` from context.rs and impact.rs. ~73 lines saved. |
| ~~Test helpers (`tests/helpers/`)~~ | ~~Medium~~ | Deferred to v0.5.5. |
| ~~Large function review~~ | ~~Low~~ | Deferred to v0.5.5. |

## v0.5.5 — Index Performance & Test Coverage

| Item | Priority | Description |
|---|---|---|
| Parallel embedding (rayon) | **High** | Embed chunks in parallel during indexing. ~300s → ~100s on 4-core for 55k chunks. Each `embed_text` is independent. |
| Parallel chunking | **High** | Parse and chunk files in parallel during indexing. Each file is independent. Rayon `par_iter` over walk entries. |
| Parallel import extraction | Medium | Extract imports per-file in parallel during `ImportGraph::build_full`. Each file's imports are independent. |
| E2E coverage for search.rs | **High** | Cover hybrid/semantic search paths (47.6% → 80%+). Requires built index in test fixtures. |
| E2E coverage for mcp.rs | **High** | Cover remaining MCP tool paths (51.4% → 80%+). Extend `tests/mcp_e2e.rs`. |
| E2E coverage for run.rs | Medium | Cover external command execution paths (63.1% → 80%+). Extend `tests/e2e.rs`. |
| E2E coverage for init.rs | Medium | Cover config generation paths (59.8% → 80%+). Test each framework target. |
| Test helpers (`tests/helpers/`) | Medium | Extract `run_prx()`, `test_dir()` helpers to reduce e2e.rs boilerplate. ~300 lines saved. |
| Large function review | Low | Review `run()` functions over 100 lines for decomposition (readability only). |

## v0.5.6 — Memory-Mapped Index

| Item | Priority | Description |
|---|---|---|
| Memory-mapped index files | **High** | Use mmap instead of read-to-vec for chunks.bin, bm25.bin, embeddings.bin. OS handles caching — index stays in memory across queries. Critical for `prx bench-ndcg` performance (currently re-reads ~200MB per query). |
| `bench-ndcg --plain` | Medium | Human-readable table output for terminal use. |
| `bench-ndcg` load-once | Medium | Load index once, query N times. Depends on mmap or refactored search API. |

## v0.5.7 — Public Benchmark Suite

| Item | Priority | Description |
|---|---|---|
| Query generation for 8 pinned repos | **High** | 25 labeled queries per repo (flask, ripgrep, fastify, cargo, django, kafka, terraform, vscode). Done — 200 total queries across 6 languages, 3 size tiers. |
| `benchmark.yml` CI workflow | **High** | Clone repos at pinned SHAs, build index, run NDCG, compare to baseline, fail on regression >0.05 (relaxed until query count increases). |
| Results dashboard | Medium | `benchmarks/results/` with per-release JSON. |
| Expand to 40-50 queries per repo | Medium | 25 queries gives ±0.05-0.08 standard error — too noisy for 0.02 regression detection. 40-50 narrows to ±0.03, enabling tighter CI gate. Prioritize medium/large repos where misses are highest. |

## v0.5.8 — Distribution & Documentation

| Item | Priority | Description |
|---|---|---|
| Documentation site (mdBook) | **High** | mdBook-based docs at `civitas-io.github.io/prx/`. Organize existing 21 docs into SUMMARY.md, add `deploy-docs.yml` workflow for GitHub Pages. |
| `cargo publish` | **High** | `cargo install prx`. |
| Homebrew formula | High | `brew install civitas-io/tap/prx` |
| npm wrapper | Medium | `npx prx` for JS/TS agents |
| pip wrapper | Medium | `pip install prx` for Python agents |

## v0.5.9 — Additional Grammars

| Item | Priority | Description |
|---|---|---|
| Kotlin grammar | Medium | tree-sitter-kotlin + import/outline extraction |
| Swift grammar | Medium | tree-sitter-swift + import/outline extraction |
| C# grammar | Medium | tree-sitter-c-sharp + import/outline extraction |
| PHP grammar | Medium | tree-sitter-php + import/outline extraction |
| Elixir grammar | Medium | tree-sitter-elixir + import/outline extraction |

## v0.6.0 — Model Tiering (Code-Specific Embeddings)

Benchmark data (v0.5.7) shows the 32M general-purpose model works for small
codebases (NDCG@10 0.5-0.7) but degrades on medium (0.3-0.4) and large
(0.2-0.3). Code-specific models distilled via Model2Vec can close this gap
while keeping pure-Rust inference.

Research: `docs/design/MODEL-TIERING.md`. Baseline: `benchmarks/results/v0.5.7-baseline.json`.

| Item | Priority | Description |
|---|---|---|
| Distill code-specific Model2Vec models | **High** | Distill CodeSage-v2-Base (356M) and/or all-mpnet-base-v2 (109M) into Model2Vec format (256d, f16). ~30 sec distillation, ~8 MB output. Benchmark against 200-query suite. |
| `prx index --model` flag | **High** | Support `--model builtin` (default), `--model standard`, `--model large`. Download on first use to `~/.prx/models/`. |
| Repo analysis + model recommendation | **High** | After `prx index`, emit a hint if repo has >3K files: "For better semantic search, try `prx index --model standard`". |
| Model download infrastructure | **High** | SHA-256 pinned downloads from HuggingFace or GitHub Releases. Offline via `PRX_MODELS_DIR`. Progress bar. |
| Benchmark regression gate | Medium | CI workflow that runs 200-query NDCG suite against all 8 repos. Fail if regression > 0.02 on any size tier. |
| Evaluate Jina Code v3 distillation | Medium | Distill Jina Code v3 (570M) for the "large" tier. Expected ~30-60 MB, higher quality for 10K+ file repos. |

**Model tiers:**

| Tier | Model | Size | Target | NDCG@10 (expected) |
|---|---|---|---|---|
| `builtin` | potion-retrieval-32M (current) | 32 MB embedded | <3K files | 0.5-0.7 |
| `standard` | CodeSage-Base-M2V-256 | ~8 MB download | 3K-10K files | 0.5-0.6 (est.) |
| `large` | Jina-Code-v3-M2V-512 | ~30-60 MB download | 10K+ files | 0.4-0.5 (est.) |

---

## Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All breaking
changes are documented in CHANGELOG.md with migration guides. JSON output includes
a `version` field for programmatic detection.
