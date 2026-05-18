# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-18

Initial release. 13 commands, 300 tests, 84% coverage.

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

[0.1.0]: https://github.com/civitas-io/prx/releases/tag/v0.1.0
