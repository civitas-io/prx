HANDOFF CONTEXT
===============

PROJECT
-------
- Name: prx (Praxis)
- Repository: https://github.com/civitas-io/prx
- Language: Rust (edition 2024)
- License: Apache 2.0
- Status: All commands implemented, CI deployed, ready for v0.1.0

WHAT IT IS
----------
A single Rust binary providing agent-native replacements for core Unix tools
(grep, cat, find, sed, diff). 13 subcommands, all returning structured JSON
with token budgets, content hashing, and semantic search. Embedded 16M-parameter
Model2Vec model for semantic code search. No runtime dependencies, no internet.

CURRENT STATE
-------------
- 300 tests (256 unit + 44 E2E), all passing
- 84% coverage
- Clippy clean, fmt clean
- CI: GitHub Actions (Linux, macOS, Windows)
- Release binary: ~77 MB (includes 61 MB embedded model)
- Repository: pushed to civitas-io/prx

COMMANDS IMPLEMENTED
--------------------
search (literal + semantic + structural, RRF fusion, reranking)
read (--lines, --snap, --skeleton, --outline, --hash, --budget)
find (tree+flat, --pattern, --depth, --changed-since, --related-to)
edit (literal/regex, dry-run, --apply, --in-function, syntax validation)
diff (git diff, function attribution, semantic notes, --stat-only)
run (9 parsers: cargo test/build/clippy, pytest, go test, jest/vitest, tsc, eslint)
index (persistent to .prx/index/, --rebuild, --stats, --watch)
outline (file + directory, --kind filter)
exists (bloom filter O(1))
batch (JSONL stdin dispatch)
stats (token savings dashboard, PRX_STATS_FILE env)
init (AGENTS.md snippet, cursor/codex/opencode/claude-code configs)
mcp (MCP server over stdio via rmcp, 6 tools exposed)

REMAINING FOR v0.1.0
---------------------
- Fix Model2Vec vocabulary loading (currently uses dummy vocab)
- Float16 model conversion (reduces binary from 77 MB to ~45 MB)
- Homebrew formula
- Benchmarks (NDCG@10, token efficiency measurements)

KEY FILES
---------
- src/commands/search.rs - search with literal/semantic/structural modes
- src/commands/run.rs - structured command runner
- src/runner/ - 9 tool-specific output parsers
- src/index/persist.rs - persistent index serialization
- src/commands/mcp.rs - MCP server (rmcp)
- src/ranking/ - 5-stage reranking pipeline
- src/chunking/treesitter.rs - AST-aware code chunking

IMPORTANT DECISIONS
-------------------
- Renamed from "ag" to "prx" (Praxis) to avoid conflict with The Silver Searcher
- tree-sitter 0.26 compatible with all grammar crates (via tree-sitter-language bridge)
- Model weights embedded via include_bytes! (no downloads, works air-gapped)
- Pure Rust Model2Vec inference (no ONNX Runtime)
- Custom BM25 with sprs (not tantivy)
- Three-tier integration: CLI + MCP + agent definitions
- Stats path configurable via PRX_STATS_FILE env var
- License: Apache 2.0 (matching civitas-io org convention)

TO CONTINUE
-----------
1. Open new session in /Users/jeryn/workspace/projects/prx
2. Paste this context
3. Request: "Continue from handoff. [Your next task]"
