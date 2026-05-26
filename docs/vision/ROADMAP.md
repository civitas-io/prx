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

## v0.3.0 — Reliability & Search Quality

### Reliability & Testing [DONE]

| Item | Priority | Status | Description |
|---|---|---|---|
| MCP server E2E tests | **High** | **Done** | 8 E2E tests covering initialize, tools/list, tools/call for all 6 MCP tools, plus invalid tool error handling. |
| Incremental indexing | **High** | **Done** | Skip unchanged files via hash comparison. Reports files_changed/files_unchanged. Walker now excludes `.prx/` directory. |
| Real criterion benchmarks | **High** | **Done** | 5 search benchmarks (BM25 build/query, literal search, index build, incremental noop) + 3 chunking benchmarks (Rust/Python/plaintext at multiple sizes). |
| NDCG@10 measurement | **High** | **Done** | 50-query labeled dataset on prx (NDCG@10=0.639) + 49-query dataset on external codebase (NDCG@10=0.451). See `docs/design/SEARCH-QUALITY.md`. |
| Structural search validation | Medium | **Done** | Warns when pattern compiles but matches 0 files, or when pattern fails to compile for all languages. |

### Search Quality — Closing the Gap

Measured NDCG@10: 0.639 (self), 0.451 (external). Target: 0.70+ on unfamiliar
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

**Repository matrix:**

| Size | LOC | Repos (examples) | Languages |
|---|---|---|---|
| Small | 1K-10K | fastify/fastify-cli, BurntSushi/ripgrep | JS, Rust |
| Medium | 10K-100K | pallets/flask, golang/go (stdlib subset) | Python, Go |
| Large | 100K-500K | django/django, rust-lang/cargo | Python, Rust |

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
| pytest-cov | `pytest --cov`, `coverage report` | Planned |
| go-cover | `go test -cover` | Planned |
| jest-cov | `jest --coverage`, `c8` | Planned |

Total parsers: 19 implemented (9 original + 10 new), 3 coverage parsers planned.

### Project Intelligence

| Item | Priority | Description |
|---|---|---|
| `prx context` | High | Assemble context packages ("everything about module X") — search + read + outline in one call |
| `prx impact` | High | Reverse dependency analysis ("what breaks if I change X?") using the import graph |
| `prx deps` | Medium | Import and dependency graph visualization |
| `prx blame` | Medium | Structured git blame per function (collapse same-SHA runs) |
| `prx test` | Medium | Test discovery related to functions/files |

### Intelligence Features

| Item | Priority | Description |
|---|---|---|
| Auto-detect `--json` flags | High | For tools that support structured output (kubectl, terraform, npm, aws, gcloud), auto-add the JSON flag and parse natively |
| Generic log noise filter | High | Dedupe repeated lines, keep error context windows — shared by kubectl-logs, docker-logs, journalctl, CI output |
| Bayesian mode predictor | Low | Learn optimal read mode per file signature over time |
| Information bottleneck filter | Low | Task-conditioned line filtering for task-driven reads |
| Custom embeddings | Low | Support for user-provided or fine-tuned models |

## v0.5.0 — Distribution & Ecosystem

| Item | Priority | Description |
|---|---|---|
| `cargo publish` | High | Publish to crates.io for `cargo install prx` |
| Homebrew formula | High | `brew install civitas-io/tap/prx` |
| npm wrapper | Medium | `npx prx` for JS/TS agents |
| pip wrapper | Medium | `pip install prx` for Python agents |
| Additional grammars | Medium | Kotlin, Swift, C#, PHP, Elixir |

---

## Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All breaking
changes are documented in CHANGELOG.md with migration guides. JSON output includes
a `version` field for programmatic detection.
