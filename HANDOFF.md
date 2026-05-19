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
- 14 commands, all implemented
- 304 tests (260 unit + 44 E2E), all passing
- Clippy clean, fmt clean, pre-commit hook active
- CI: GitHub Actions (Linux x86_64 + aarch64, macOS arm64, Windows x86_64)
- Release binary: ~48 MB (float16 model embedded)
- Graceful fallback: on internal errors, falls back to grep/cat/find
- Real-world telemetry: prx stats --compare
- Installed locally at /Users/jeryn/.cargo/bin/prx
- Workspace AGENTS.md at /Users/jeryn/workspace/projects/AGENTS.md

COMMANDS (14)
-------------
search, read, find, edit, diff, run, index, outline, exists,
batch, stats, init, mcp, bench

KEY FEATURES
------------
- Hybrid search: literal + semantic (Model2Vec, float16) + structural (ast-grep)
- RRF fusion (k=60) with 5-stage reranking pipeline
- 9 test/build/lint parsers in prx run (95-99% token savings)
- Persistent index with validation (6x faster repeated searches)
- Graceful fallback to Unix tools on internal errors
- Real-world telemetry with baseline estimation
- MCP server over stdio (rmcp)

NEXT (v0.2.0 — Context Intelligence)
-------------------------------------
Session cache (13-token re-reads)
Read modes: aggressive, diff, entropy, auto
Graph proximity boost for search
File reference IDs (F1/F2 aliases)
cargo publish, Homebrew formula
NDCG@10 benchmarks

KEY DECISIONS
-------------
- Named "prx" (Praxis) — Latin for practice/action, fits civitas-io theme
- Model weights embedded via include_bytes! (float16, 31MB)
- Pure Rust Model2Vec inference (no ONNX Runtime)
- tree-sitter 0.26 confirmed compatible with all grammar crates
- Three-tier integration: CLI + MCP + agent definitions
- Fallback only on internal errors (panics, parse, index), not user errors
- macOS Intel dropped from release builds (macos-13 runner stuck)

TO CONTINUE
-----------
1. Open new session in /Users/jeryn/workspace/projects/prx
2. Paste this context
3. Request: "Continue from handoff. [Your next task]"
