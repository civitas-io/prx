# prx (Praxis)

[![CI](https://github.com/civitas-io/prx/actions/workflows/ci.yml/badge.svg)](https://github.com/civitas-io/prx/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)](#platform-support)

**AI coding agents waste [30-93% of tokens on exploration](https://arxiv.org/pdf/2604.22750). prx fixes this at the source.**

prx is a single Rust binary that replaces the five Unix tools AI coding agents use most — `grep`, `cat`, `find`, `sed`, `diff` — with structured JSON output, token budgets, and embedded semantic search. One call. Full answer. No re-parsing.

> *prx is shorter than grep, cat, find, sed, and diff combined.*

---

## The Problem

Every AI coding agent runs some version of this loop:

```
1. grep "authenticate" src/          → file paths, line numbers
2. cat src/auth/handler.ts           → entire file (6,500 tokens)
3. grep "authenticate" src/ -A 5     → same noise, wider context
```

**11,300 tokens consumed. ~800 useful.** That's 93% waste, per loop, compounding across a session. The tools aren't broken for humans — they're wrong for agents.

[SWE-bench research (arxiv 2604.22750)](https://arxiv.org/pdf/2604.22750) puts total exploration waste at 30–93% of agent token budgets. 50% of file reads are re-reads of files the agent already loaded.

---

## The Fix

```bash
prx search "authenticate" src/
```

```json
{
  "tokens": 487,
  "data": {
    "matches": [
      {
        "file": "src/auth/handler.ts",
        "line": 42,
        "context_name": "handleLogin",
        "snippet": "async handleLogin(req: Request)...",
        "relevance": 0.94
      }
    ],
    "total_matches": 23,
    "returned": 3,
    "budget_used": 487
  }
}
```

**487 tokens. Ranked results. Metadata included. Done.**

---

## What Makes prx Different

**Not a wrapper.** Tools like RTK, squeez, and LeanCTX compress output from existing tools. prx replaces the tools — no shell spawning, no re-parsing, no post-hoc compression.

**Not search-only.** Semble, Hypergrep, and FileSift solve retrieval well. Your agent still needs other tools to read, edit, and diff. prx covers the full loop.

**Not Python.** No runtime dependencies, no package manager, no internet at runtime. One static binary, 48MB, works in containers and sandboxes.

**Embedded semantic model.** A 32M-parameter retrieval-optimized embedding model (potion-retrieval-32M, PCA-reduced to 256 dims) is compiled directly into the binary. Semantic search runs on CPU in milliseconds — no model server, no FAISS, no setup.

---

## Token Savings

Measured across real agent sessions on production codebases (200+ calls, 36,000+ tokens saved):

| Feature | Scenario | Savings |
|---|---|---|
| `--if-changed` (hit) | Re-reading an unchanged file | **99%** |
| `--mode diff` | File with local changes | **98–99%** |
| `--mode entropy` | Generated code (50+ fields) | **86%** |
| `prx run` | Passing test suites | **95–99%** |
| `--skeleton` | Full file → signatures only | **~90%** |
| `prx search` | vs grep + follow-up reads | **35%** |

<p align="center">
  <img src="docs/assets/token-savings.svg" alt="Token savings per command" width="720"/>
</p>

```bash
prx stats --compare     # see per-command savings in your own sessions
prx bench .             # benchmark prx vs grep+cat on this repo
```

---

## Commands

| Command | Replaces | What it does |
|---|---|---|
| `prx search` | grep, rg | Hybrid search: literal + semantic + structural. Ranked, token-budgeted. |
| `prx read` | cat, head, tail | Structured file reading. `--if-changed` cache, `--skeleton`, `--mode`. |
| `prx find` | find, ls, tree | Codebase mapping. Tree + flat output, inline metadata, semantic scoring. |
| `prx edit` | sed, awk | Safe edits. Literal matching, dry-run default, tree-sitter syntax validation. |
| `prx diff` | diff, git diff | Semantic diffs. Function-level attribution, natural language summaries. |
| `prx run` | — | Structured test/build/lint output. 9 parsers. 95–99% token savings. |
| `prx exists` | grep -q | O(1) bloom filter existence check. Sub-millisecond, near-zero tokens. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx index` | — | Persistent search index. 6x faster repeated searches. |
| `prx mcp` | — | MCP server over stdio for direct agent integration. |
| `prx batch` | xargs | Parallel JSONL batch execution. |
| `prx init` | — | Auto-detect agent frameworks, generate integration configs. |
| `prx stats` | — | Token savings dashboard with `--compare`. |
| `prx bench` | — | Synthetic benchmark: prx vs grep+cat side-by-side. |

---

## Quick Start

```bash
# Search by meaning, not just text
prx search "authentication flow" src/

# Get file structure without reading the whole file (~10% of tokens)
prx read src/auth.ts --skeleton

# Read the function you need, not the whole file
prx read src/auth.ts --lines 42 --snap function

# Skip re-reading files that haven't changed (~50 bytes vs full content)
prx read src/auth.ts --if-changed a3f9b2c1...

# Safe editing with preview before applying
prx edit src/auth.ts --find "old_api()" --replace "new_api()"

# Run tests, get only failures and summary (95%+ savings)
prx run cargo test

# Check existence before spending tokens on a full search
prx exists "redis" src/
```

> Full command reference with benchmarks: [USAGE.md](docs/USAGE.md)

---

## How Search Works

prx combines three retrieval methods into a single ranked result:

- **Literal** — regex matching at ripgrep speed
- **Semantic** — 32M-parameter retrieval-optimized embedding model (Model2Vec potion-retrieval-32M, PCA-reduced to 256 dims, float16, embedded in the binary; runs on CPU in milliseconds, no server required)
- **Structural** — AST pattern matching via tree-sitter, e.g. `fn $NAME($$$) { $$$ }` to find all function definitions

Results are fused via Reciprocal Rank Fusion and reranked through a 6-stage pipeline: definition boost → identifier stem matching → file coherence → **import graph proximity** (files in the dependency neighborhood of top results) → noise penalties → saturation decay.

Search quality is measured via NDCG@10 on labeled datasets — see [Search Quality Tracking](#search-quality-tracking) and [`docs/design/SEARCH-QUALITY.md`](docs/design/SEARCH-QUALITY.md) for methodology and improvement roadmap.

```bash
prx search "authentication flow" src/          # semantic (auto-detected)
prx search --literal "authenticate(" src/      # exact match, ripgrep-speed
prx search --structural 'fn $NAME($$$) { $$$ }' src/   # AST pattern matching
```

---

## prx run — Structured Command Output

Unix test runners produce thousands of tokens of noise that agents don't need.

```
running 164 tests
test test_one ... ok
test test_two ... ok
[... 162 more lines ...]
test result: ok. 164 passed; 0 failed
```

prx run parses this and returns only what matters:

```json
{
  "passed": 164,
  "failed": 0,
  "duration_ms": 2341,
  "failures": []
}
```

Supports 9 parsers: `cargo test`, `cargo build`, `cargo clippy`, `pytest`, `go test`, `jest`, `vitest`, `tsc`, `eslint`, plus a fallback for unknown commands.

---

## Agent Integration

### MCP Server

```json
{
  "mcpServers": {
    "prx": {
      "command": "prx",
      "args": ["mcp"]
    }
  }
}
```

Works with Claude Code, Cursor, Codex, OpenCode, and any MCP-compatible agent.

### AGENTS.md / Framework Configs

```bash
prx init                        # auto-detect frameworks, generate all configs
prx init --agents-md            # append usage snippet to AGENTS.md
prx init --agent claude-code    # generate dedicated Claude Code sub-agent
```

### Three Integration Tiers

| Tier | How | Best for |
|---|---|---|
| **CLI on PATH** | `prx search ...` | All agents, CI, scripts |
| **MCP server** | `prx mcp` | Top-level agent integration |
| **Agent definition** | `prx init --agent claude-code` | Dedicated sub-agent |

### For AI Agents

If you are an AI coding agent evaluating prx, read [`skills/agents.md`](skills/agents.md) — it contains installation instructions, per-command examples, and measured token savings written for agent consumption.

---

## Reliability

prx never breaks your agent's workflow. On any internal error, prx silently falls back to the equivalent Unix command (`grep`/`cat`/`find`) and returns results in the same JSON envelope with `"fallback": true`. Errors are logged to `~/.prx/errors.jsonl`.

---

## Install

### Prebuilt Binary (recommended)

```bash
# Linux / macOS
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv prx /usr/local/bin/
prx --version
```

Download from [GitHub Releases](https://github.com/civitas-io/prx/releases).

### Build from Source

Requires Rust ≥ 1.85 and a C compiler (for tree-sitter grammars).

```bash
git clone https://github.com/civitas-io/prx.git
cd prx
make setup    # downloads model weights (~35MB), converts to float16, builds, tests
```

First run takes ~2 minutes. Model weights are embedded into the binary at compile time — no downloads at runtime.

```bash
make build      # debug build
make release    # optimized release build (~48MB)
make check      # fmt + clippy + all tests
```

See [CONTRIBUTING](docs/CONTRIBUTING.md) for the full developer guide.

---

## Platform Support

| Platform | Status |
|---|---|
| Linux x86_64 | Supported |
| Linux aarch64 | Supported |
| macOS Apple Silicon | Supported |
| Windows x86_64 | Supported |

Single static binary. No runtime dependencies. No internet required after build.

---

## Current Status

| | |
|---|---|
| Commands | 14 |
| Tests | 402 (319 unit + 75 E2E + 8 MCP) |
| Languages | 14 (tree-sitter grammars) |
| Import graph | 7 languages (Rust, Python, JS/TS, Go, Java, C/C++, Ruby) |
| Release binary | ~49 MB (float16 model embedded) |
| CI | GitHub Actions (Linux x86_64, Linux aarch64, macOS arm64, Windows) |
| Telemetry | Real-world token savings via `prx stats --compare` |

See [ROADMAP](docs/vision/ROADMAP.md) for what's next.

---

## Search Quality Tracking

NDCG@10 measured on two labeled datasets: prx's own codebase (50 queries,
145 files) and an external production codebase (49 queries, 11k files).
Methodology and ground truth in [`docs/design/SEARCH-QUALITY.md`](docs/design/SEARCH-QUALITY.md).

| Version | prx (self) | External | Semantic | Symbol | Architecture | Notes |
|---|---|---|---|---|---|---|
| v0.2.0 | 0.719 | 0.410 | 0.417 | 0.239 | 0.562 | Baseline measurement |
| v0.3.0 | 0.723 | 0.486 | 0.506 | 0.238 | 0.634 | +chunk headers, persistent dense index, symbol boost, synonym expansion, overlap |
| v0.3.0+ | 0.730 | 0.520 | 0.551 | 0.238 | 0.620 | +potion-retrieval-32M model (PCA→256), evaluated 3 models |

External scores use a 49-query dataset on an 11k-file Python/TypeScript
codebase (not written by the prx authors). This is the honest number — self-eval
inflates scores by ~40% due to labeling bias.

**Model evaluation (v0.3.0):** Evaluated 3 embedding models head-to-head:

| Model | Type | Params | Fiddler NDCG@10 | Binary | Index (11k files) | Verdict |
|---|---|---|---|---|---|---|
| potion-code-16M | Static (Model2Vec) | 16M | 0.486 | 48MB | 33s | baseline |
| CodeMalt | Static (Model2Vec) | 7.6M | 0.470 (-3%) | 31MB | 44s | rejected — smaller vocab hurts |
| **potion-retrieval-32M** | **Static (Model2Vec)** | **32M** | **0.520 (+7%)** | **49MB** | **39s** | **selected** |
| all-MiniLM-L6-v2 | Transformer (Candle) | 33M | not tested | 137MB | ~46min (CPU) | rejected — Metal missing LayerNorm, CPU too slow |

For comparison: Semble reports 0.854 on their own benchmark. ripgrep scores
~0.13 on the same benchmark. Direct comparison requires running both tools on
the same dataset (planned).

---

## Contributing

See [CONTRIBUTING](docs/CONTRIBUTING.md) for setup, development workflow, and how to add new commands, languages, and run parsers.

## License

Apache 2.0

---

Part of the [Civitas](https://github.com/civitas-io) ecosystem — open infrastructure for AI agent tooling.
