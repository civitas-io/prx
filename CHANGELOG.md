# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.13] - 2026-06-01

### Fixed

- **C1: Search result ordering** — `saturation_decay` now decays all chunks before
  truncating to top_k, preventing decayed low-score chunks from evicting higher-scoring
  chunks from different files.
- **C2: Diff function attribution** — `find_changed_functions` now uses
  `change.new_index()` instead of manual line counting, fixing off-by-N errors
  when deletions interleave with insertions.
- **C3: Language table consistency** — `walk.rs` now delegates to
  `language_name_for_extension()` as the single source of truth. Fixes `.jsx`
  reporting `"jsx"` in some paths and `"javascript"` in others.
- **C4: Dead CLI flags removed** — `search --context`, `search --exists`, and
  `read --meta` were silently accepted with no effect. Removed from the CLI.
- **C5: Workspace root consistency** — unified `find_workspace_root` into
  `workspace.rs`. Both `context` and `impact` commands now use the same logic.
- **C6: Mmap overflow protection** — `MmapEmbeddings::open` now uses `checked_mul`
  for the `n_chunks * dim * 4` size calculation, returning `InvalidData` instead
  of panicking on corrupt index metadata.
- **C7: Go coverage reporting** — `go_cover` summary now honestly labels its
  output as "unweighted" average across packages.
- **C8: JS/TS outline noise** — `lexical_declaration` no longer classified as a
  symbol. Top-level `const x = 5` is filtered out; `const fn = () => {}` still
  emitted via the arrow_function handler.

## [0.5.12] - 2026-06-01

### Fixed

- **Documentation sweep** — updated all docs to reflect 27 languages (was 15),
  tree-sitter 0.26 (was 0.25), 20 import families (was 10). Covers: roadmap,
  architecture overview, search architecture, platform reference, contributing
  guide, architecture SVGs, and HANDOFF.md.
- Marked v0.5.10 and v0.5.11 as DONE in roadmap.

## [0.5.11] - 2026-06-01

### Fixed

- **Windows CI linker failure** — replaced `tree-sitter-markdown-updated` (CRT mismatch
  on MSVC) with `tree-sitter-md` which uses the proper `LanguageFn` API.
- **Dockerfile grammar crate** — replaced `tree-sitter-dockerfile-updated` with
  `tree-sitter-containerfile` for consistent `LanguageFn` API across all grammars.
- **Homebrew formula** — updated version and URLs to v0.5.11.

## [0.5.10] - 2026-06-01

### Added

**Programming languages (5):**
- **Kotlin** — `.kt`, `.kts`. Import extraction, symbol outline, snapping.
  Uses `tree-sitter-kotlin-sg` fork for tree-sitter 0.26 compatibility.
- **Swift** — `.swift`. Imports, outline (functions, classes, structs, enums, protocols), snapping.
- **C#** — `.cs`. `using` directives, outline (classes, interfaces, structs, enums, methods, namespaces), snapping.
- **PHP** — `.php`. `use` declarations + `require`/`include`, outline (functions, classes, interfaces, traits, enums), snapping.
- **Elixir** — `.ex`, `.exs`. `import`/`alias`/`use`/`require`, outline (`defmodule`, `def`/`defp`, `defprotocol`), snapping.

**Config/scripting languages (7):**
- **YAML** — `.yml`, `.yaml`. Parsing, chunking, skeleton support.
- **TOML** — `.toml`. Uses `tree-sitter-toml-ng` fork for 0.26 compat.
- **Markdown** — `.md`. Uses `tree-sitter-markdown-updated` fork.
- **Dockerfile** — `Dockerfile`. Uses `tree-sitter-dockerfile-updated` fork.
- **HCL/Terraform** — `.tf`, `.hcl`. Outline extracts resource/variable/module blocks with labels.
- **SQL** — `.sql`. Uses `tree-sitter-sequel` fork with LanguageFn API.
- **Makefile** — `Makefile`, `.mk`. Outline extracts rules and variable assignments.

**Totals:**
- Supported languages: **27** (was 15)
- Tests: **454 unit + 80 E2E = 534** (was 442 + 80 = 522)

## [0.5.9] - 2026-05-31

### Changed

- **crates.io ready** — `cargo publish --dry-run` passes. Build script
  downloads models to `OUT_DIR` instead of source tree. Published crate
  is 171 KB compressed (models downloaded at build time via `build.rs`).
- **Cargo.toml exclude** — models/, benchmarks/, book/, docs/, .prx/,
  .github/ excluded from published crate.

### To publish

```bash
cargo login <your-token>
cargo publish
```

## [0.5.8] - 2026-05-30

### Added

- **mdBook documentation site** — 33 pages across 7 sections (User Guide,
  Commands, Architecture, Performance, Reference, Contributing, Vision).
  Deployed to `civitas-io.github.io/prx/` via GitHub Pages.
- **`deploy-docs.yml`** — GitHub Actions workflow for automatic docs
  deployment on push to main.

### Changed

- **Internal docs reorganized** — sprint-specific and internal design docs
  moved to `docs/internal/`. CLAUDE.md removed (redundant with AGENTS.md).
- **Makefile simplified** — removed stale `setup`, `models`, `coverage`
  targets. Added `docs` target for mdBook build.

## [0.5.7] - 2026-05-29

### Added

- **Public benchmark suite** — 200 labeled queries across 8 public repos
  (flask, ripgrep, fastify, cargo, kafka, django, terraform, vscode).
  6 languages, 3 size tiers. Measured NDCG@10 with ground-truth relevance.
- **`benchmark.yml` CI workflow** — runs NDCG benchmark on release tags.
  Clones all 8 repos at pinned SHAs, indexes, benchmarks, fails on
  regression > 0.05. Results uploaded as artifacts.
- **Versioned baseline results** — `benchmarks/results/v0.5.7-baseline.json`
  with per-repo scores, category breakdowns, and miss counts.
- **v0.6.0 Model Tiering milestone** — design doc for code-specific
  Model2Vec models with download-on-demand. Based on benchmark findings
  showing semantic search degradation at scale.

### Benchmark Results

| Tier | Repos | Avg NDCG@10 |
|---|---|---|
| Small (<3K files) | flask, ripgrep, fastify | 0.545 |
| Medium (3-10K files) | cargo, kafka, django | 0.332 |
| Large (10K+ files) | terraform, vscode | 0.248 |

## [0.5.6] - 2026-05-29

### Changed

- **Memory-mapped embeddings** — `embeddings.bin` loaded via `memmap2` with
  zero-copy `bytemuck::cast_slice`. OS page cache keeps 54 MB of embeddings
  warm across queries. Falls back to owned allocation if mmap fails.
- **`bench-ndcg` load-once** — index loaded once, queries run N times.
  50-query benchmark: 12.76s → 0.23s (55x speedup).
- **`bench-ndcg --plain`** — human-readable table output with NDCG scores,
  per-category breakdown, and miss list.
- **Test helpers module** — shared `ag()`, `test_dir()`, `parse_json()` in
  `tests/helpers/mod.rs`. 5 new E2E tests covering semantic search with
  index, alpha override, run modes, and context budgets.

### Stats

- 442 unit + 80 E2E + 8 MCP = 530 tests (was 525).

## [0.5.5] - 2026-05-29

### Changed

- **Parallel indexing with rayon** — all 5 stages of `prx index` now run
  in parallel. File read/hash/chunk, BM25 enrichment, embedding computation,
  import graph, and symbol index all use `par_iter` or `rayon::join`.
  BLAS thread limits set at process start to prevent oversubscription.

### Performance

- **7.6x speedup** on large codebases (11K files, 55K chunks):
  410s → 54s on 10-core Mac (944% CPU utilization).
- **3x speedup** on small codebases (258 files, 910 chunks):
  1.2s → 0.4s.
- Embedding computation (94% of indexing time) parallelized with
  shared `&model` reference — no Arc, no Mutex, no cloning.

## [0.5.4] - 2026-05-29

### Changed

- **`define_regex!` macro** — reduces 3-line `LazyLock<Regex>` statics to
  1-line macro calls across all 21 runner parsers.
- **`ParsedResult::new()` constructor** — replaces 7-field struct literals
  in 8 parsers where `warnings` and `tail` are empty defaults.
- **Shared `workspace` module** — extracted `relative_path()` and
  `is_test_file()` from duplicated copies in `context.rs` and `impact.rs`
  into `src/workspace.rs`.

### Stats

- Net -159 lines across 27 files. No behavior changes. All 442 tests pass.

## [0.5.3] - 2026-05-28

### Added

- **`prx bench-ndcg`** — Rust-native NDCG benchmark runner. Calls
  search::run() directly without spawning external processes. Outputs
  structured JSON with per-query scores, category breakdown, and misses.
  Suitable for CI regression gate.

## [0.5.2] - 2026-05-27

### Changed

- **Self-contained build** — `cargo build` now works without `make models`
  or Python. The build script (`build.rs`) downloads model weights from
  HuggingFace, verifies SHA-256 hashes, and converts F32→F16 in pure Rust.
  Set `PRX_MODELS_DIR` for offline/air-gapped builds.
- **Migrated bincode → postcard** — replaced unmaintained `bincode`
  (RUSTSEC-2025-0141) with `postcard` for all index serialization.
  Existing `.prx/index/` directories will auto-rebuild on version mismatch.

## [0.5.1] - 2026-05-27

### Fixed

- **Renamed model file** — `potion-code-16M.safetensors` renamed to
  `potion-retrieval-32M.safetensors` to match the actual model (upgraded
  in v0.3.0 but file never renamed). Updated `include_bytes!` references,
  download script, and all documentation.

### NDCG (v0.5.0 tree-sitter imports)

- External NDCG@10: 0.494 (stable, no regression from v0.4.0)
- prx self-benchmark: NDCG@10 = 0.673 (stable, was 0.681 in v0.4.0)
- 9 complete misses unchanged (all semantic, unrelated to imports)

## [0.5.0] - 2026-05-27

Tree-sitter imports & auto-JSON release. Import extraction rewritten from
regex to AST queries, extended to all languages with import concepts.

### Added

- **`prx run --auto-json`** — auto-injects `--json`/`-o json` flags for
  tools that support structured output (kubectl, terraform, npm, eslint,
  mypy). Existing JSON detection in parsers handles the output side.
- **Import extraction for bash, CSS, HTML** — `source`/`.` commands,
  `@import` rules, `<script src>`/`<link href>` attributes.
- **Tree-sitter import forms** — multi-path `use` (Rust), multiline
  imports (Python), re-exports and dynamic `import()` (JS/TS), type
  imports (TS).

### Changed

- **Import extraction rewritten from regex to tree-sitter** — all 10
  language families now use AST queries instead of line-by-line regex
  matching. Captures forms that regex cannot: multi-line imports, aliased
  imports, re-exports, dynamic `import()` calls.

### Stats

| Metric | v0.4.5 | v0.5.0 |
|---|---|---|
| Tests | 421 | 435 |
| Import languages | 7 (regex) | 10 (tree-sitter) |
| Import forms captured | basic only | multi-line, aliased, re-export, dynamic |

## [0.4.5] - 2026-05-27

### Fixed

- **Documentation consistency** — softened "tree-sitter for all structural
  awareness" claim to reflect that import extraction uses regex. Added build
  prerequisites (Python 3, network) to README. Added context/impact to
  architecture docs. Added import graph proximity stage to search pipeline docs.

## [0.4.4] - 2026-05-27

### Changed

- **Incremental embeddings** — `prx index` now caches per-chunk content
  hashes alongside embeddings. On re-index, only chunks whose content
  changed are re-embedded; unchanged chunks reuse cached embeddings.
  For a 1-file change in an 11k-file repo, this reduces embedding time
  from ~300s (full re-embed) to seconds.

## [0.4.3] - 2026-05-27

### Fixed

- **Import resolution no longer bails on common filenames** — previously,
  `resolve_import` gave up (returned no edge) when a name matched >3 files.
  Common names like `index.ts`, `utils.py`, `mod.rs` triggered this in large
  repos, making the import graph sparser as repos grew. Now uses directory
  proximity to pick the closest 1-2 candidates instead of giving up. Threshold
  raised from 3 to 5, with proximity-based disambiguation above that.

## [0.4.2] - 2026-05-27

### Fixed

- **Embedding model failure now warns instead of silently degrading** —
  when `load_model()` fails during indexing, `prx index` output now includes
  a `warnings` field explaining that search will use BM25 only. Previously,
  `embeddings_dim: 0` was written silently and search quality halved with
  no signal to the user.

## [0.4.1] - 2026-05-27

### Fixed

- **`is_valid` now detects new files** — previously, `prx index` reported
  "up_to_date" when new files were added after indexing, because `is_valid()`
  only checked that previously-indexed files hadn't changed. Now walks the
  tree to detect both new and deleted files.

## [0.4.0] - 2026-05-26

Project Intelligence & Run Parsers release. Symbol index for search quality,
two new commands (context, impact), 13 new run parsers, security CI, and
JSON output detection.

### Added

- **Symbol index** — maps symbol names to definition locations with reference
  counts at index time. For symbol queries, boosts definition chunks directly
  instead of relying on BM25. Symbol NDCG@10: 0.263 → 0.619 (+135%). 4
  previously-complete-miss symbol queries recovered.
- **`prx context`** — assembles a context package for a module in one call:
  stats, documentation, entrypoints ranked by reference count, file skeletons,
  and 1-hop import graph edges. Replaces outline + find + cat README + grep.
- **`prx impact`** — reverse dependency analysis. Walks the import graph
  backwards to find what depends on a file. Supports `--symbol` narrowing,
  `--hops` control, fan-in protection, test file filtering.
- **13 new run parsers** — mypy, dotnet, git-log, docker-build, npm-ls,
  terraform, kubectl, kubectl-logs, mvn, gradle, pytest-cov, go-cover,
  jest-coverage. Total: 22 parsers.
- **JSON output detection** — kubectl, terraform, npm-ls, and eslint parsers
  auto-detect JSON responses when user passes `--json`/`-o json` and parse
  structurally instead of regex-matching text.
- **Security CI** — `cargo-deny` runs on every push/PR checking advisories
  (RustSec), license compliance, source origin, and dependency bans.
- **`deny.toml`** — security policy configuration.
- **`benchmarks/repos.json`** — 8 public repos pinned by SHA for NDCG
  regression testing (flask, ripgrep, fastify, cargo, django, kafka,
  terraform, vscode).
- **`docs/design/RUN-PARSERS.md`** — design doc for the parser system.
- **`is_symbol_query`** now detects snake_case identifiers (e.g. `feature_impact`).
- Symbol queries routed to hybrid search instead of literal search.

### Changed

- **NDCG measurement corrected** — previous scores were inflated by a
  deduplication bug. All docs updated with corrected numbers.
- **`skills/agents.md`** rewritten with tool replacement table, new commands,
  recommended workflow.

### Stats

| Metric | v0.3.0 | v0.4.0 |
|---|---|---|
| Commands | 14 | 16 |
| Tests | 372 | 413 unit + 75 E2E + 8 MCP |
| Run parsers | 9 | 22 |
| Index files | 5 | 6 (+symbols.bin) |
| NDCG@10 (self) | 0.639 | 0.681 |
| NDCG@10 (external) | 0.451 | 0.494 |
| Symbol NDCG@10 | 0.263 | 0.619 |

## [0.3.0] - 2026-05-25

Reliability & Search Quality release. NDCG measurement infrastructure, incremental
indexing, persistent dense index, and search ranking improvements.

### Added

- **MCP server E2E tests** — 8 tests covering JSON-RPC initialize, tools/list,
  tools/call for all 6 MCP tools, and invalid tool error handling.
- **Incremental indexing** — `prx index` skips unchanged files by comparing content
  hashes from the previous index. Reports `files_changed`/`files_unchanged` in output.
  Walker now excludes `.prx/` directory.
- **Real criterion benchmarks** — `benches/search.rs` (BM25 build/query, literal search,
  persistent index build, incremental no-op) and `benches/chunking.rs` (Rust/Python/plaintext
  at 10/50/100/500 functions).
- **NDCG@10 measurement** — labeled relevance datasets for prx (50 queries) and an external
  11k-file Python/TypeScript codebase (49 queries). Automated NDCG harness in `tests/ndcg.rs`.
  Results tracked per-release in README.
- **Structural search validation** — warns when a pattern compiles but matches 0 files,
  or when a pattern fails to compile for all languages. Warning surfaced in search output.
- **Persistent dense index** — chunk embeddings computed at index time and stored as
  `embeddings.bin`. At query time, semantic retrieval runs independently of BM25 before
  RRF fusion. Unlocks semantic recall for queries where BM25 fails.
- **Chunk header enrichment** — BM25 enrichment prepends `[lang] file_path stem_tokens`
  to each chunk. Split identifiers (camelCase/snake_case) indexed as separate terms.
- **Synonym expansion** — 18-pair static dictionary (auth→authentication, db→database,
  k8s→kubernetes, etc.) applied to BM25 queries for natural language searches.
- **Chunk overlap** — 200-byte overlap between adjacent chunks, snapped to line boundaries.
- **Configurable reranker** — `RerankConfig` struct enables selective stage toggling for
  ablation testing.
- `docs/design/SEARCH-QUALITY.md` — full NDCG analysis, failure mode diagnosis, improvement
  roadmap, and symbol graph feasibility assessment.
- `benchmarks/ndcg_dataset.json` — 50 labeled queries for prx codebase.
- `benchmarks/ndcg_dataset_external.json` — 49 labeled queries for external codebase.

### Changed

- **Symbol-query ranking** — definition boost increased from 3x to 12x for symbol queries
  (single PascalCase/snake_case tokens). Import-heavy chunks penalized at 0.2x.
- **Alpha tuning** — symbol queries now use alpha=0.1 (near-pure BM25, was 0.3).
  Natural language queries use alpha=0.6 (was 0.5). Queries containing synonyms use 0.5.
- **Reranker weights** — definition boost 3→4 (NL), stem match 1.0→1.5, file coherence
  0.2→0.15, import penalty 0.3→0.2.
- **Definition detection** — improved pattern matching for Python/TypeScript class and
  function definitions (requires space or paren after keyword).
- **Model loading** — extracted `load_model()` to `index/dense.rs` as a public function,
  shared between index-time embedding and query-time fallback.
- `is_symbol_query()` made public for use by ranking pipeline.

### Stats

| Metric | v0.2.0 | v0.3.0 |
|---|---|---|
| Tests | 353 (304 unit + 49 E2E) | 372 (315 unit + 49 E2E + 8 MCP) |
| NDCG@10 (self) | — | 0.639 |
| NDCG@10 (external) | — | 0.451 |
| Benchmarks | 2 stubs | 8 real (search + chunking) |
| Index files | 4 (meta, chunks, bm25, imports) | 5 (+embeddings.bin) |

Note: v0.2.0 NDCG scores omitted — measured with a buggy script that did not
deduplicate files across chunks, producing inflated results. Corrected
methodology applied from v0.3.0 onward.

## [0.2.0] - 2026-05-19

Context Intelligence release. Conditional reads, read modes, and import graph proximity boost.

### Added

- **`--if-changed HASH`** flag for `prx read` — stateless conditional read. Agent passes
  the `meta.hash` from a previous response; if the file is unchanged, prx returns a
  ~50-byte cached stub instead of full content. 99% token savings on re-reads.
- **`--mode aggressive`** — strips comments using tree-sitter (14 grammars) and collapses
  blank lines. Preserves strings containing comment-like syntax. 1-19% savings depending
  on comment density.
- **`--mode diff`** — returns only lines changed vs git HEAD. Falls back to full content
  for untracked files. 98-99% savings on files with local modifications.
- **`--mode entropy`** — pattern-based repetitive line filter. Normalizes digits, allows
  3 occurrences of each pattern, suppresses the rest. Up to 86% savings on generated code.
- **Import graph proximity boost** for search — extracts `import`/`use`/`require` statements
  from 7 languages (Rust, Python, JS/TS, Go, Java, C/C++, Ruby) via regex. Files within
  2 hops of top-ranked results get a 0.25x additive boost with hop decay. Graph persisted
  to `.prx/index/imports.bin`.
- `docs/USAGE.md` — full command reference with real-world benchmarks.
- `skills/agents.md` — agent-facing skill guide: what prx is, how to use it, how to
  install as MCP server / CLI skill / agent definition.
- Token savings SVG chart in README with per-feature measurements.

### Changed

- Search reranking pipeline now has 6 stages (was 5): added import graph proximity
  between stem matching and noise penalties.
- `prx index --rebuild` now builds and persists the import graph alongside chunks and BM25.
- Telemetry baseline estimation improved for search, find, exists, and diff commands.
- E2E tests now route `PRX_STATS_FILE` and `PRX_ERRORS_FILE` to `/dev/null` (no more
  test pollution in real telemetry).
- GitHub Actions: `softprops/action-gh-release` v2 → v3 (node24).

### Fixed

- `--mode diff` now correctly runs `git` from the file's parent directory, fixing
  incorrect results when prx is called with absolute paths from a different CWD.

### Stats

| Metric | v0.1.0 | v0.2.0 |
|---|---|---|
| Tests | 304 (260 unit + 44 E2E) | 353 (304 unit + 49 E2E) |
| Modules | 29 | 32 (+imports.rs, graph.rs, proximity.rs) |
| LOC (src/) | ~8,200 | ~9,600 |
| Reranking stages | 5 | 6 (+ proximity) |
| Import graph languages | — | 7 |

## [0.1.0] - 2026-05-19

Initial release. 14 commands, 304 tests.

### Reliability

- Graceful fallback: on internal errors (panics, parse errors), prx silently falls back to grep/cat/find and returns results with `fallback: true` in the envelope. User errors (file not found) are returned normally.
- Error logging: every fallback logs to `~/.prx/errors.jsonl` for debugging
- Pre-commit hook: mirrors CI checks (fmt + clippy + tests)

### Telemetry & Benchmarks

- Real-world telemetry: every command logs `actual_bytes` vs `baseline_bytes` to `~/.prx/stats.jsonl`
- `prx stats --compare`: per-command savings breakdown from real usage
- `prx bench`: synthetic benchmark runner comparing prx vs grep+cat side-by-side

### Commands

- `prx search` — hybrid search: literal + semantic (Model2Vec) + structural (ast-grep). RRF fusion with adaptive alpha. 5-stage reranking pipeline.
- `prx read` — structured file reading with --skeleton, --snap, --outline, --hash, --budget.
- `prx find` — codebase mapping with tree+flat output, --pattern, --depth, --changed-since, --related-to.
- `prx edit` — find-replace with dry-run default, --apply, --in-function scoping, syntax validation.
- `prx diff` — git diff with semantic summaries, function attribution, --stat-only.
- `prx run` — structured command runner with 9 tool parsers (cargo test/build/clippy, pytest, go test, jest/vitest, tsc, eslint). 95-99% token savings on test output.
- `prx index` — persistent search index with validation, --rebuild, --stats, --watch.
- `prx outline` — symbol table for files and directories with --kind filter.
- `prx exists` — bloom filter O(1) existence check.
- `prx batch` — JSONL batch execution from stdin.
- `prx stats` — token savings dashboard.
- `prx init` — auto-detect agent frameworks, generate MCP configs and AGENTS.md snippets.
- `prx mcp` — MCP server over stdio exposing 6 tools.

### Infrastructure

- 14 tree-sitter language grammars (Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, Bash, JSON, HTML, CSS, TSX)
- Model2Vec potion-code-16M embedded in binary (float16, 31MB)
- Real vocabulary loading (61,826 tokens via HuggingFace tokenizer)
- cl100k_base tokenizer for --budget enforcement
- Persistent index serialization to .prx/index/
- Content hashing (xxh3) for change detection
- BM25 with compound identifier tokenization (camelCase/snake_case splitting)
- Cross-platform: Linux x86_64 + aarch64, macOS arm64 + Intel, Windows x86_64
- GitHub Actions CI (lint, test, build) + release pipeline (5 targets)
- Apache 2.0 license

### Documentation

- 21 documentation files (~5,000 lines)
- AGENTS.md with Karpathy coding guidelines
- PRD, roadmap, architecture, CLI spec, output schema, benchmarks plan, implementation plan, testing plan, crate reference, competitive landscape, platform audit, contributing guide

[0.3.0]: https://github.com/civitas-io/prx/releases/tag/v0.3.0
[0.2.0]: https://github.com/civitas-io/prx/releases/tag/v0.2.0
[0.1.0]: https://github.com/civitas-io/prx/releases/tag/v0.1.0
