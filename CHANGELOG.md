# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - 2026-05-27

### Fixed

- **Renamed model file** — `potion-code-16M.safetensors` renamed to
  `potion-retrieval-32M.safetensors` to match the actual model (upgraded
  in v0.3.0 but file never renamed). Updated `include_bytes!` references,
  download script, and all documentation.

### NDCG (v0.5.0 tree-sitter imports)

- Fiddler NDCG@10: 0.494 (stable, no regression from v0.4.0)
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
- `benchmarks/ndcg_dataset_fiddler.json` — 49 labeled queries for external codebase.

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
