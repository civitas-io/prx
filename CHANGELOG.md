# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.2.0]: https://github.com/civitas-io/prx/releases/tag/v0.2.0
[0.1.0]: https://github.com/civitas-io/prx/releases/tag/v0.1.0
