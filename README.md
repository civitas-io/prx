# prx (Praxis)

[![CI](https://github.com/civitas-io/prx/actions/workflows/ci.yml/badge.svg)](https://github.com/civitas-io/prx/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)](#platform-support)

**AI coding agents burn most of their context window re-discovering code they've already seen. prx fixes that at the source.**

prx is a single Rust binary that replaces the Unix tools coding agents lean on most — `grep`, `cat`, `find`, `sed`, `diff` — with structured JSON output, hard token budgets, and an embedded semantic search model. One call returns a ranked, budgeted answer instead of a wall of text the agent has to read, parse, and re-read. No shell spawning, no post-hoc compression, no model server.

---

## The problem

Every coding agent runs some version of this loop:

```
1. grep "authenticate" src/          → file paths, line numbers
2. cat src/auth/handler.ts           → entire file (thousands of tokens)
3. grep "authenticate" src/ -A 5     → same noise, wider context
```

Most of those tokens are waste: whole files read to use ten lines, the same file loaded twice in a session, test logs dumped in full to find one failure. The tools aren't broken — they were built for humans reading a terminal, not for an agent paying for every token and working inside a fixed context window. That mismatch is the tax prx removes.

> The token-waste figures previously cited here are being re-sourced. Rather than quote a number we can't currently point you to a verifiable reference for, we let the per-command savings table below — measured on real sessions — speak for itself.

---

## The fix

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

Ranked results, metadata included, under a token budget you control. The agent gets the answer, not the haystack.

---

## What makes prx different

**It replaces the tools, it doesn't wrap them.** Compression tools shell out to `grep`/`cat` and squeeze the output afterward. prx does the search, reading, and diffing itself — no subprocess, no re-parsing, no lossy post-processing.

**It covers the whole loop, not just search.** Retrieval-only tools still leave your agent to read, edit, diff, and run tests with the old noisy tools. prx handles search, structured reads, safe edits, semantic diffs, and parsed test/build output behind one consistent JSON envelope.

**It has no runtime dependencies.** One static binary, ~49 MB, no Python, no package manager, no network at runtime. It runs in containers and sandboxes as-is.

**The semantic model is built in.** A 32M-parameter retrieval-optimized embedding model (potion-retrieval-32M, stored as float16) is compiled directly into the binary. Semantic search runs on CPU in milliseconds — no model server, no vector database, no setup step.

---

## Token savings

Measured across real agent sessions on production codebases. Run the numbers on your own repo with `prx stats --compare` and `prx bench .`.

| Feature | Scenario | Savings |
|---|---|---|
| `read --if-changed` (cache hit) | Re-reading an unchanged file | ~99% |
| `read --mode diff` | File with local changes | 98–99% |
| `read --skeleton` | Full file reduced to signatures | ~90% |
| `run` | Passing test suites | 95–99% |
| `read --mode entropy` | Generated / highly repetitive code | ~86% |
| `search` | vs grep + follow-up reads | ~35% |

<p align="center">
  <img src="docs/assets/token-savings.svg" alt="Token savings per command" width="720"/>
</p>

---

## Indexing performance

`prx index` builds a persistent search index — BM25, semantic embeddings, import graph, and symbol definitions — in a single parallel pass. All five stages run on all available CPU cores via rayon.

| Codebase | Files | Chunks | Time | CPU |
|---|---|---|---|---|
| Small Rust project (prx itself) | 260 | 910 | **0.4s** | — |
| Large production monorepo | 11,021 | 55,484 | **54s** | 944% (10 cores) |

The embedding stage (computing 256-dim vectors for 55K chunks) is 94% of the work. Parallelizing it alone turned a 7-minute indexing job into under a minute. On CI runners (4 cores), expect ~3-4x speedup. On workstations (8-16 cores), expect ~6-8x.

Incremental rebuilds skip unchanged files entirely — only modified or new files are re-chunked and re-embedded.

---

## The commands agents actually orchestrate around

Most tools stop at "better grep." The two commands below are why prx is useful for agents working inside a tight context window — they answer questions that would otherwise take a dozen `grep`/`cat` calls to reconstruct.

### `prx context` — understand a module in one call

```bash
prx context src/auth/
```

Returns a single structured package for a directory: summary stats, doc/README content, entrypoints, per-file **skeletons** (signatures without bodies), and the **import edges** connecting the files. Instead of the agent running `find`, then `cat README`, then `outline` on each file, then chasing imports by hand, it gets the whole mental model of a module in one budgeted response — ideal for the "load just enough to start a task" step in an agent loop.

### `prx impact` — know what breaks before you touch it

```bash
prx impact src/auth.ts
```

Reverse-dependency analysis built on prx's import graph: it answers "what depends on this file?" so an agent (or a human) can scope a refactor before making it. Edges are extracted from the AST (see [How search works](#how-search-works)); when an import name is ambiguous across many files, resolution falls back to a directory-proximity heuristic and returns the most likely candidates rather than guessing blindly. Treat its output as a high-quality map, not a formal proof of completeness.

---

## All commands

| Command | Replaces | What it does |
|---|---|---|
| `prx search` | grep, rg | Hybrid search: literal + semantic + structural. Ranked, token-budgeted. |
| `prx read` | cat, head, tail | Structured reading. `--if-changed` cache, `--skeleton`, `--mode`, `--snap`. |
| `prx find` | find, ls, tree | Codebase mapping. Tree or flat output, inline metadata, semantic scoring. |
| `prx edit` | sed, awk | Safe edits. Literal matching, dry-run by default, tree-sitter syntax validation. |
| `prx diff` | diff, git diff | Semantic diffs with function-level attribution and natural-language summaries. |
| `prx run` | — | Parsed test/build/lint output. 22 parsers; `--auto-json` for tools with structured output. |
| `prx context` | — | Module context package: stats, docs, entrypoints, skeletons, import edges. |
| `prx impact` | — | Reverse dependency analysis: what depends on a given file. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx exists` | grep -q | Fast bloom-filter existence check, near-zero tokens. |
| `prx index` | — | Parallel persistent index: 11K files in ~55s (7.6x speedup via rayon). |
| `prx mcp` | — | MCP server over stdio for direct agent integration. |
| `prx batch` | xargs | Parallel JSONL batch execution. |
| `prx init` | — | Detects agent frameworks and generates integration configs. |
| `prx stats` | — | Token-savings dashboard, with `--compare`. |
| `prx bench` | — | Side-by-side benchmark: prx vs grep+cat. |

16 commands total. Full reference with examples: [docs/USAGE.md](docs/USAGE.md).

---

## Quick start

```bash
# Search by meaning, not just text
prx search "authentication flow" src/

# Get a module's whole shape in one call
prx context src/auth/

# See what depends on a file before refactoring it
prx impact src/auth.ts

# File structure without the bodies (~10% of the tokens)
prx read src/auth.ts --skeleton

# Read just the function you need
prx read src/auth.ts --lines 42 --snap function

# Skip re-reading a file that hasn't changed
prx read src/auth.ts --if-changed a3f9b2c1

# Safe edit with a preview before applying
prx edit src/auth.ts --find "old_api()" --replace "new_api()"

# Run tests, get only failures and a summary
prx run cargo test
```

---

## How search works

prx fuses three retrieval methods into one ranked result:

- **Literal** — regex matching at ripgrep speed.
- **Semantic** — the embedded potion-retrieval-32M Model2Vec model (PCA-reduced to 256 dims, float16); runs on CPU in milliseconds, no server.
- **Structural** — AST pattern matching via tree-sitter, e.g. `fn $NAME($$$) { $$$ }` to match all function definitions.

Results are combined with Reciprocal Rank Fusion and reranked through a multi-stage pipeline: definition boost, identifier-stem matching, file coherence, **import-graph proximity** (favoring files in the dependency neighborhood of strong hits), noise penalties, and saturation decay.

```bash
prx search "authentication flow" src/                  # semantic (auto-detected)
prx search --literal "authenticate(" src/              # exact match, ripgrep speed
prx search --structural 'fn $NAME($$$) { $$$ }' src/   # AST pattern matching
```

The import graph is extracted from the AST (tree-sitter) across 10 language families that have an import concept. Search quality is tracked with NDCG@10 on labeled datasets — see [Search quality](#search-quality) for the honest numbers and methodology.

---

## `prx run` — structured command output

Test runners emit thousands of tokens an agent doesn't need:

```
running 164 tests
test test_one ... ok
test test_two ... ok
[... 162 more lines ...]
test result: ok. 164 passed; 0 failed
```

`prx run` parses that and returns only the signal:

```json
{ "passed": 164, "failed": 0, "duration_ms": 2341, "failures": [] }
```

22 parsers cover Rust, Python, Go, JavaScript/TypeScript, Java, .NET, Docker, Terraform, kubectl, Maven, Gradle, npm, mypy, git, common coverage tools, and a generic fallback for unrecognized commands.

---

## Agent integration

### MCP server

```json
{
  "mcpServers": {
    "prx": { "command": "prx", "args": ["mcp"] }
  }
}
```

Exposes prx over stdio to any MCP-compatible agent. (prx also works equally well as a plain CLI on `PATH` — see the tiers below.)

### Config generation

```bash
prx init                      # detect frameworks, generate configs
prx init --agents-md          # append a usage snippet to AGENTS.md
prx init --agent claude-code  # generate a dedicated Claude Code sub-agent
```

### Integration tiers

| Tier | How | Best for |
|---|---|---|
| **CLI on PATH** | `prx search ...` | Any agent, CI, scripts — the simplest and most portable path |
| **MCP server** | `prx mcp` | Agents that prefer structured tool calls mid-task |
| **Agent definition** | `prx init --agent claude-code` | A dedicated retrieval sub-agent |

### For AI agents

If you're an agent evaluating prx, read [`skills/agents.md`](skills/agents.md): installation, per-command examples, and measured token savings written for machine consumption.

---

## Reliability

If an internal operation fails, prx falls back to the equivalent Unix command and returns results in the same JSON envelope, flagged so the caller can tell a fallback occurred. Errors are logged to `~/.prx/errors.jsonl`. The intent is that prx never hard-breaks an agent's workflow — but because a fallback silently trades semantic search for plain matching, agents that depend on retrieval quality should check the flag rather than assume every result is a full-quality prx result.

---

## Install

### Prebuilt binary (recommended)

```bash
# Linux / macOS
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv prx /usr/local/bin/
prx --version
```

Or download from [GitHub Releases](https://github.com/civitas-io/prx/releases). The prebuilt binary already contains the embedded model — nothing else to install.

### Build from source

> **Important:** `cargo build` on its own will **not** work. The embedding model is compiled into the binary via `include_bytes!`, and its weights are downloaded by a setup step first. Run `make setup` before anything else.

Requirements: Rust ≥ 1.85, a C compiler (for tree-sitter grammars), **network access**, and **Python 3** (used once to convert the model weights to float16 during setup).

```bash
git clone https://github.com/civitas-io/prx.git
cd prx
make setup     # downloads model weights (~35 MB), converts to float16, builds, runs tests
```

First run takes a couple of minutes. After setup, the weights are baked into the binary — no downloads at runtime.

```bash
make build     # debug build
make release    # optimized release build (~49 MB binary)
make check     # fmt + clippy + tests
```

See [CONTRIBUTING](docs/CONTRIBUTING.md) for the full developer guide.

---

## Platform support

| Platform | Status |
|---|---|
| Linux x86_64 | Supported |
| Linux aarch64 | Supported |
| macOS Apple Silicon | Supported |
| Windows x86_64 | Supported |

Single static binary. No runtime dependencies. No network required after build.

---

## Current status

| | |
|---|---|
| Commands | 17 |
| Tests | 442 unit + 75 E2E + 8 MCP |
| Run parsers | 22 (cargo, pytest, go, jest, eslint, tsc, kubectl, terraform, docker, + 13 more) |
| Languages (parsing) | 15 tree-sitter grammars |
| Import graph | 10 language families, tree-sitter AST extraction |
| Symbol index | Definition lookup + reference counting |
| Indexing | Parallel via rayon — 11K files in 54s on 10 cores (7.6x speedup) |
| Embedded model | potion-retrieval-32M (Model2Vec, float16, PCA→256 dims) |
| Release binary | ~49 MB |
| CI | GitHub Actions: Linux x86_64 / aarch64, macOS arm64, Windows |

See [ROADMAP](docs/vision/ROADMAP.md) for what's planned next.

---

## Search quality

NDCG@10 measured on two labeled datasets: prx's own codebase (50 queries, 173 files) and an external production codebase (49 queries, ~11k files, not written by the prx authors). Scores use file-level deduplication. Methodology and ground truth in [docs/design/SEARCH-QUALITY.md](docs/design/SEARCH-QUALITY.md).

| Version | prx (self) | External | Notes |
|---|---|---|---|
| v0.3.0 | 0.639 | 0.451 | Corrected baseline |
| v0.4.0 | 0.681 | 0.494 | Added symbol index |
| v0.5.1 | 0.673 | 0.494 | Tree-sitter imports (no regression) |

The external score is the honest one — self-evaluation inflates results through labeling bias, so we report both and lead with the external number when comparing. A direct head-to-head against other tools on a shared dataset is planned; until that exists, we don't claim a ranking against them.

---

## Contributing

See [CONTRIBUTING](docs/CONTRIBUTING.md) for setup, workflow, and how to add commands, languages, and run parsers.

## License

Apache 2.0

---

Part of the [Civitas](https://github.com/civitas-io) ecosystem — open infrastructure for AI agent tooling.
