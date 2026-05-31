# prx (Praxis)

AI coding agents burn most of their context window re-discovering code they've already seen. prx fixes that at the source.

prx is a single Rust binary that replaces the Unix tools coding agents lean on most: `grep`, `cat`, `find`, `sed`, `diff`. Every command returns structured JSON with ranked results, hard token budgets, and content hashes. One call returns a budgeted answer instead of a wall of text the agent has to read, parse, and re-read.

## The problem

Every coding agent runs some version of this loop:

```
1. grep "authenticate" src/          → file paths, line numbers
2. cat src/auth/handler.ts           → entire file (thousands of tokens)
3. grep "authenticate" src/ -A 5     → same noise, wider context
```

Most of those tokens are waste: whole files read to use ten lines, the same file loaded twice in a session, test logs dumped in full to find one failure. The tools aren't broken. They were built for humans reading a terminal, not for an agent paying for every token inside a fixed context window. That mismatch is the tax prx removes.

## What makes prx different

**It replaces the tools, it doesn't wrap them.** Compression tools shell out to `grep`/`cat` and squeeze the output afterward. prx does the search, reading, and diffing itself. No subprocess, no re-parsing, no lossy post-processing.

**It covers the whole loop, not just search.** Retrieval-only tools still leave your agent to read, edit, diff, and run tests with the old noisy tools. prx handles search, structured reads, safe edits, semantic diffs, and parsed test/build output behind one consistent JSON envelope.

**No runtime dependencies.** One static binary, ~49 MB, no Python, no package manager, no network at runtime. It runs in containers and sandboxes as-is.

**The semantic model is built in.** A 32M-parameter retrieval-optimized embedding model (potion-retrieval-32M, stored as float16) is compiled directly into the binary. Semantic search runs on CPU in milliseconds. No model server, no vector database, no setup step.

**It's fast.** Indexing runs on all CPU cores in parallel (7.6x speedup on 10 cores). Embeddings are memory-mapped with zero-copy access. A 50-query benchmark suite runs in 0.23 seconds.

## All commands

| Command | Replaces | What it does |
|---|---|---|
| `prx search` | grep, rg | Hybrid search: literal + semantic + structural. Ranked, token-budgeted. |
| `prx read` | cat, head, tail | Structured reading with `--if-changed` cache, `--skeleton`, `--mode`, `--snap`. |
| `prx find` | find, ls, tree | Codebase mapping with tree or flat output, inline metadata, semantic scoring. |
| `prx edit` | sed, awk | Safe edits with literal matching, dry-run by default, tree-sitter syntax validation. |
| `prx diff` | diff, git diff | Semantic diffs with function-level attribution and natural-language summaries. |
| `prx run` | — | Parsed test/build/lint output. 22 parsers; `--auto-json` for structured output. |
| `prx context` | — | Module context package: stats, docs, entrypoints, skeletons, import edges. |
| `prx impact` | — | Reverse dependency analysis: what depends on a given file. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx exists` | grep -q | Fast bloom-filter existence check, near-zero tokens. |
| `prx index` | — | Parallel persistent index: 11K files in ~55s (7.6x speedup via rayon). |
| `prx mcp` | — | MCP server over stdio for direct agent integration. |
| `prx batch` | xargs | Parallel JSONL batch execution. |
| `prx init` | — | Detects agent frameworks and generates integration configs. |
| `prx stats` | — | Token-savings dashboard with `--compare`. |
| `prx bench` | — | Side-by-side benchmark: prx vs grep+cat. |
| `prx bench-ndcg` | — | NDCG search quality benchmark against labeled datasets. |

## Token savings at a glance

| Feature | Scenario | Savings |
|---|---|---|
| `read --if-changed` (cache hit) | Re-reading an unchanged file | ~99% |
| `read --mode diff` | File with local changes | 98-99% |
| `read --skeleton` | Full file reduced to signatures | ~90% |
| `run` | Passing test suites | 95-99% |
| `read --mode entropy` | Generated / highly repetitive code | ~86% |
| `search` | vs grep + follow-up reads | ~35% |

Full telemetry data and methodology: [Token Savings](guide/token-savings.md).

---

Get started: [Quick Start](guide/quickstart.md)
