# prx (Praxis)

[![CI](https://github.com/civitas-io/prx/actions/workflows/ci.yml/badge.svg)](https://github.com/civitas-io/prx/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)](#platform-support)
[![Docs](https://img.shields.io/badge/docs-civitas--io.github.io%2Fprx-blue)](https://civitas-io.github.io/prx/)

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

**It's fast.** Indexing runs on all CPU cores in parallel (~17x total speedup on 10 cores: v0.5.5: 7.6x parallel stages, v0.5.14: 2.2x parallel embeddings + hot-path optimizations). Embeddings are memory-mapped with zero-copy access — no heap allocation, no deserialization. A 50-query benchmark suite runs in 0.23 seconds.

---

## Token savings

Measured via `prx bench .` on real repositories. Your mileage varies by codebase size and query type.

| Feature | Scenario | Savings | Source |
|---|---|---|---|
| `read --if-changed` (cache hit) | Re-reading an unchanged file | ~99% | 48-byte stub vs full file |
| `run` | Passing test suites (cargo test, pytest) | 95–99% | `prx run` parsed output vs raw |
| `find` | File listing vs `find -type f` | ~92% | `prx bench .` measured |
| `read --skeleton` | Signatures only vs full file | 60–90% | `prx bench .` measured (varies by file size) |
| `search --top-k` | Top-5 results vs `grep -rn` | ~86% | `prx bench .` measured |
| `search` (literal) | Ranked results vs `grep -rn` | ~46% | `prx bench .` measured |
| `read --mode diff` | Changed lines vs full file | 80–97% | Measured on modified files |
| `read --mode entropy` | Repetitive code filtered | 5–87% | Measured on generated structs |

<p align="center">
  <img src="docs/assets/token-savings.svg" alt="Token savings per command" width="720"/>
</p>

---

## Performance

### Indexing: ~17x parallel speedup

`prx index` builds a persistent search index — BM25, semantic embeddings, import graph, and symbol definitions — in a single parallel pass. All five stages run on all available CPU cores via rayon (v0.5.5). v0.5.14 added parallel embedding computation via `par_iter` across chunks, O(n) top-k selection, precomputed newline offsets, HashSet-based BM25 df, and per-chunk word sets for symbol refs.

| Codebase | Files | Chunks | Time |
|---|---|---|---|
| Flask (Python, 15K LOC) | 259 | 1,225 | **0.3s** |
| ripgrep (Rust, 25K LOC) | 239 | 2,465 | **0.6s** |
| fastify (TypeScript, 15K LOC) | 417 | 2,529 | **0.6s** |
| cargo (Rust, 150K LOC) | 2,815 | 12,118 | **5s** |
| terraform (Go, 2M LOC) | 5,323 | 22,798 | **10s** |
| django (Python, 300K LOC) | 5,690 | 30,944 | **32s** |
| kafka (Java, 500K LOC) | 7,231 | 63,740 | **114s** |
| vscode (TypeScript, 1M LOC) | 14,643 | 136,056 | **340s** |

Measured on 10-core Apple Silicon with rayon parallelism (944% CPU utilization). On CI runners (4 cores), expect ~3-4x speedup over sequential. Incremental rebuilds skip unchanged files entirely.

> Times above are from the v0.5.7 baseline. v0.5.14 added parallel embedding computation and hot-path optimizations, reducing the 11K-file benchmark from 55s to 24s (2.2x additional speedup).

### `find --tree`: 47x faster at scale

`prx find --tree` on an 11K-file codebase dropped from 33s to 0.7s after replacing the O(n²) JSON tree builder with a native nested map that serializes once.

### Search: zero-copy memory-mapped embeddings

Embedding vectors are memory-mapped directly from disk via `memmap2` and cast to `&[f32]` with zero allocation using `bytemuck`. The OS page cache keeps the index warm across queries — no heap allocation, no deserialization, no repeated file reads.

On an 11K-file codebase with 54 MB of embeddings, this means:

- **Zero bytes** allocated for embedding data (OS manages the pages)
- Queries after the first hit warm cache — sub-millisecond embedding access
- Falls back to owned allocation automatically if mmap isn't available (network FS, etc.)

### Benchmarking: 55x speedup with load-once

`prx bench-ndcg` measures search quality (NDCG@10) against labeled datasets. It loads the index once and runs all queries against cached data:

| Benchmark | Before (v0.5.5) | After (v0.5.6) | Speedup |
|---|---|---|---|
| 50-query NDCG suite | 12.76s | **0.23s** | **55x** |

Use `--plain` for human-readable output in the terminal.

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

### `prx explain` — understand a symbol instantly

```bash
prx explain SearchWorkerBuilder
```

One call returns the definition (with body), all references, and test files for a symbol. Agents currently stitch `search → read --snap → impact` to get this information; `explain` does it in one round-trip.

### `prx rename` — rename across the codebase

```bash
prx rename AuthManager SessionManager       # dry-run preview
prx rename AuthManager SessionManager --apply  # write changes
```

Finds every file that mentions the symbol, generates line-level before/after diffs, and optionally applies the rename. Dry-run by default (like `prx edit`). Use `--include-tests` to also rename in test files.

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
| `prx explain` | — | Symbol explainer: definition + references + tests in one call. |
| `prx rename` | — | Cross-file symbol rename with dry-run preview. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx exists` | grep -q | Fast bloom-filter existence check, near-zero tokens. |
| `prx index` | — | Parallel persistent index: 11K files in ~24s (~17x speedup via rayon). |
| `prx mcp` | — | MCP server over stdio for direct agent integration. |
| `prx batch` | xargs | Parallel JSONL batch execution. |
| `prx init` | — | Detects agent frameworks and generates integration configs. |
| `prx stats` | — | Token-savings dashboard, with `--compare`. |
| `prx bench` | — | Side-by-side benchmark: prx vs grep+cat. |
| `prx bench-ndcg` | — | NDCG search quality benchmark against labeled datasets. |

17 commands total. Full reference with examples in the [documentation site](https://civitas-io.github.io/prx/).

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

The import graph is extracted from the AST (tree-sitter) across 20 language families that have an import concept. Search quality is tracked with NDCG@10 on labeled datasets — see [Search quality](#search-quality) for the honest numbers and methodology.

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

### Skills guide (for AI agents)

prx ships with a machine-readable skill guide at [`skills/agents.md`](skills/agents.md) — a complete reference written for AI agents, not humans. It covers:

- **When to use prx** instead of grep, cat, find, sed (decision table)
- **Core workflow** — the 12-step sequence from existence check to test run
- **Per-command examples** with expected JSON output
- **Measured token savings** per command (99% on cache hits, 95% on test runs)
- **Conditional reads, budgets, and search modes** with concrete usage

Load it into your agent's context with `prx init --agents-md`, or read it directly. It's 210 lines — designed to fit in a single context load.

---

## Reliability

If an internal operation fails, prx falls back to the equivalent Unix command and returns results in the same JSON envelope, flagged so the caller can tell a fallback occurred. Errors are logged to `~/.prx/errors.jsonl`. The intent is that prx never hard-breaks an agent's workflow — but because a fallback silently trades semantic search for plain matching, agents that depend on retrieval quality should check the flag rather than assume every result is a full-quality prx result.

---

## Install

```bash
# Homebrew (macOS / Linux)
brew install civitas-io/tap/prx

# Cargo (Rust developers)
cargo install prx

# Prebuilt binary (any platform)
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-aarch64-apple-darwin.tar.gz | tar xz
sudo mv prx /usr/local/bin/
```

| Method | What you get | Requirements |
|---|---|---|
| **Homebrew** | Prebuilt binary, auto-updates | macOS or Linux with Homebrew |
| **cargo install** | Builds from source | Rust 1.85+, C compiler, network |
| **GitHub Releases** | Prebuilt binary, manual download | None |

Prebuilt binaries for all platforms on [GitHub Releases](https://github.com/civitas-io/prx/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `prx-x86_64-unknown-linux-gnu.tar.gz` |
| Linux aarch64 | `prx-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `prx-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `prx-x86_64-pc-windows-msvc.zip` |

The binary contains the embedded model — no downloads at runtime, works offline.

### Build from source

```bash
git clone https://github.com/civitas-io/prx.git && cd prx
cargo build --release    # model downloaded automatically by build.rs
```

See the [Contributing guide](https://civitas-io.github.io/prx/contributing/setup.html) for the full developer setup.

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
| Tests | 454 unit + 80 E2E + 8 MCP |
| Run parsers | 22 (cargo, pytest, go, jest, eslint, tsc, kubectl, terraform, docker, + 13 more) |
| Languages (parsing) | 27 tree-sitter grammars |
| Import graph | 20 language families, tree-sitter AST extraction |
| Symbol index | Definition lookup + reference counting |
| Indexing | Parallel via rayon — 11K files in ~24s on 10 cores (~17x total speedup: 410s → 24s). Zero-copy mmap embeddings. |
| Embedded model | potion-retrieval-32M (Model2Vec, float16, PCA→256 dims) |
| Release binary | ~49 MB |
| CI | GitHub Actions: Linux x86_64 / aarch64, macOS arm64, Windows |

See the [Roadmap](https://civitas-io.github.io/prx/vision/roadmap.html) for what's planned next.

<details>
<summary><strong>Supported languages (27)</strong></summary>

| Language | Extensions | Parsing | Imports | Outline | Snap |
|---|---|---|---|---|---|
| Rust | `.rs` | ✓ | ✓ | ✓ | ✓ |
| Python | `.py` `.pyi` | ✓ | ✓ | ✓ | ✓ |
| JavaScript | `.js` `.jsx` `.mjs` `.cjs` | ✓ | ✓ | ✓ | ✓ |
| TypeScript | `.ts` `.tsx` `.mts` `.cts` | ✓ | ✓ | ✓ | ✓ |
| Go | `.go` | ✓ | ✓ | ✓ | ✓ |
| Java | `.java` | ✓ | ✓ | ✓ | ✓ |
| C | `.c` `.h` | ✓ | ✓ | ✓ | ✓ |
| C++ | `.cpp` `.cc` `.hpp` `.hxx` | ✓ | ✓ | ✓ | ✓ |
| Ruby | `.rb` | ✓ | ✓ | — | ✓ |
| Bash | `.sh` `.bash` `.zsh` | ✓ | ✓ | — | ✓ |
| Kotlin | `.kt` `.kts` | ✓ | ✓ | ✓ | ✓ |
| Swift | `.swift` | ✓ | ✓ | ✓ | ✓ |
| C# | `.cs` | ✓ | ✓ | ✓ | ✓ |
| PHP | `.php` | ✓ | ✓ | ✓ | ✓ |
| Elixir | `.ex` `.exs` | ✓ | ✓ | ✓ | ✓ |
| SQL | `.sql` | ✓ | — | — | — |
| HCL/Terraform | `.tf` `.hcl` | ✓ | — | ✓ | ✓ |
| YAML | `.yml` `.yaml` | ✓ | — | — | — |
| TOML | `.toml` | ✓ | — | — | — |
| Markdown | `.md` | ✓ | — | — | — |
| Dockerfile | `Dockerfile` | ✓ | — | — | — |
| Makefile | `Makefile` `.mk` | ✓ | — | ✓ | — |
| JSON | `.json` | ✓ | — | — | — |
| HTML | `.html` `.htm` | ✓ | ✓ | — | — |
| CSS | `.css` | ✓ | ✓ | — | — |

**Parsing** = tree-sitter AST (chunking, `--skeleton`, `--mode aggressive`). **Imports** = dependency graph extraction. **Outline** = symbol table (`prx outline`). **Snap** = structural snapping (`--snap function/class`).

</details>

---

## Search quality

NDCG@10 measured on 360 labeled queries across 8 public repositories (6 languages, 3 size tiers). All repos pinned by commit SHA. Ground truth in `benchmarks/repos/`. Methodology in [Search Quality](https://civitas-io.github.io/prx/performance/search-quality.html).

| Repo | Language | Files | Queries | NDCG@10 | 95% CI |
|---|---|---|---|---|---|
| Flask | Python | 263 | 45 | **0.661** | [0.575, 0.744] |
| ripgrep | Rust | 239 | 45 | **0.593** | [0.490, 0.698] |
| fastify | TypeScript | 420 | 45 | **0.513** | [0.416, 0.614] |
| cargo | Rust | 2,818 | 45 | **0.378** | [0.285, 0.470] |
| kafka | Java | 7,255 | 45 | **0.314** | [0.203, 0.427] |
| django | Python | 5,699 | 45 | **0.261** | [0.180, 0.350] |
| terraform | Go | 5,343 | 45 | **0.268** | [0.184, 0.354] |
| vscode | TypeScript | 15,326 | 45 | **0.201** | [0.120, 0.290] |

Symbol search is consistently strong (0.65–0.87) across all sizes. Semantic and architecture queries degrade at scale. Pipeline improvements (learned-to-rank fusion, multi-field BM25) are planned for v0.7.0 — see [Roadmap](https://civitas-io.github.io/prx/vision/roadmap.html).

These are honest numbers on codebases we didn't write and don't tune for.

---

## Contributing

See the [Contributing guide](https://civitas-io.github.io/prx/contributing/setup.html) for setup, workflow, and how to add commands, languages, and run parsers.

## License

Apache 2.0

---

Part of the [Civitas](https://github.com/civitas-io) ecosystem — open infrastructure for AI agent tooling.
