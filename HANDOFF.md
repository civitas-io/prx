HANDOFF CONTEXT
===============

PROJECT
-------
- Name: prx (Praxis) — agent-native Unix tools
- Repository: https://github.com/civitas-io/prx
- Language: Rust (edition 2024, MSRV 1.85)
- License: Apache 2.0
- Status: v0.1.0 released

CURRENT STATE
-------------
- 13 commands, all implemented and working
- 300 tests (256 unit + 44 E2E), all passing
- 84% coverage (target: 80%)
- Clippy clean, fmt clean
- CI: GitHub Actions (Linux, macOS, Windows)
- Release binary: ~48 MB (float16 model embedded)
- v0.1.0 tagged and pushed, release pipeline building binaries

COMMANDS
--------
search    - literal + semantic + structural, RRF fusion, 5-stage reranking
read      - --lines, --snap, --skeleton, --outline, --hash, --budget
find      - tree+flat, --pattern, --depth, --changed-since, --related-to
edit      - literal/regex, dry-run default, --apply, --in-function, syntax check
diff      - git diff, function attribution, semantic notes, --stat-only
run       - 9 parsers (cargo test/build/clippy, pytest, go, jest/vitest, tsc, eslint)
index     - persistent .prx/index/, --rebuild, --stats, --watch
outline   - file + directory, --kind filter
exists    - bloom filter O(1)
batch     - JSONL stdin dispatch
stats     - token savings dashboard (PRX_STATS_FILE env)
init      - auto-detect frameworks, generate configs
mcp       - MCP server over stdio (rmcp, 6 tools)

NEXT (v0.2.0)
-------------
- Benchmarks (NDCG@10, token efficiency, latency profiling)
- cargo publish to crates.io
- Homebrew formula
- More run parsers (bun test, deno test, dotnet test, ruff)
- Additional language grammars (Kotlin, Swift, C#, PHP, Elixir)

KEY DECISIONS
-------------
- Named "prx" (Praxis) to avoid conflict with "ag" (The Silver Searcher)
- tree-sitter 0.26 confirmed compatible with all grammar crates
- Model weights embedded via include_bytes! (float16, 31MB, no downloads)
- Pure Rust Model2Vec inference (no ONNX Runtime)
- Custom BM25 with sprs (not tantivy)
- Three-tier integration: CLI + MCP + agent definitions
- Windows paths normalized to forward slashes
- Stats path configurable via PRX_STATS_FILE env var

TO CONTINUE
-----------
1. Open new session in /Users/jeryn/workspace/projects/prx
2. Paste this context
3. Request: "Continue from handoff. [Your next task]"
