# prx Usage Guide

Complete reference for all prx commands with real-world benchmarks.

## Read Modes

prx read returns structured JSON with metadata, content, and symbol outlines.
Several modes reduce token usage for different scenarios.

### Default Read

```bash
prx read src/auth.ts                    # full file + metadata + outline
prx read src/auth.ts --skeleton          # signatures only (~10% of tokens)
prx read src/auth.ts --outline           # symbol table only
prx read src/auth.ts --lines 42-67       # line range
prx read src/auth.ts --lines 42 --snap function  # expand to enclosing function
prx read src/auth.ts --hash              # content hash only
prx read src/auth.ts --budget 500        # cap output at 500 tokens
```

### Conditional Read (`--if-changed`)

Pass the `meta.hash` from a previous read response. If the file hasn't changed,
prx returns a tiny stub instead of the full content.

```bash
# First read — get the hash
prx read src/auth.ts
# Response includes: "meta": { "hash": "a3f9b2c1...", ... }

# Subsequent read — skip if unchanged
prx read src/auth.ts --if-changed a3f9b2c1...
# If unchanged: { "cached": true, "meta": {...} } — ~50 bytes
# If changed: full content returned normally
```

**Benchmark** (845-line Rust file):

| Scenario | Tokens | Savings |
|---|---|---|
| Full read | 6,531 | — |
| `--if-changed` (hit) | 57 | **99.1%** |
| `--if-changed` (miss) | 6,531 | 0% (full content) |
| Uppercase hash | matches | case-insensitive |
| Malformed hash | error | validated |

### Aggressive Mode (`--mode aggressive`)

Strips comments using tree-sitter (14 grammars) and collapses blank lines.
Preserves all functional code and strings containing comment-like syntax.

```bash
prx read src/auth.ts --mode aggressive
```

**Benchmark** (real-world codebases):

| File type | Savings |
|---|---|
| Clean Rust code (few comments) | 1-7% |
| Python with docstrings | 11-19% |
| Heavily commented config files | 13-19% |
| Code with inline comments | 5-14% |

### Diff Mode (`--mode diff`)

Returns only lines that changed vs git HEAD. Falls back to full content for
untracked files or files outside a git repo.

```bash
prx read src/auth.ts --mode diff
```

Output uses `+`/`-` prefixes with line numbers:

```
+L42: fn new_function() {
+L43:     let x = 1;
+L44: }
-L50:     let old_value = 0;
+L50:     let new_value = 1;
```

**Benchmark** (845-line Rust file with 10 lines changed):

| Scenario | Tokens | Savings |
|---|---|---|
| Full read | 6,603 | — |
| `--mode diff` | 89 | **98.7%** |
| No changes vs HEAD | 5 (`[no changes vs HEAD]`) | **99.9%** |
| Untracked file | full content | 0% (correct fallback) |

### Entropy Mode (`--mode entropy`)

Filters repetitive lines by normalizing patterns (digits replaced, whitespace
trimmed). Allows 3 occurrences of each pattern, suppresses the rest. Appends
a count of filtered lines.

```bash
prx read generated/schema.rs --mode entropy
```

**Benchmark** (real-world codebases):

| File type | Savings |
|---|---|
| Generated structs (50+ fields) | **86%** |
| Repetitive test assertions | 15-18% |
| Config files with similar entries | 3-6% |
| Normal source code | 0% (no filtering needed) |

### Combining Modes

`--if-changed` takes priority. On a cache miss, `--mode` applies normally:

```bash
# If unchanged → cached stub (57 tokens)
# If changed → aggressive mode applied to new content
prx read src/auth.ts --if-changed abc123... --mode aggressive
```

## Search

Hybrid search combining literal, semantic, and structural retrieval.

```bash
prx search "authentication flow" src/     # semantic (auto-detected)
prx search --literal "authenticate(" src/  # exact match
prx search --structural 'fn $NAME($$$)' src/ # AST pattern
prx search "auth" src/ --top-k 10         # more results
prx search "auth" src/ --budget 1000      # cap total tokens
```

Search results are ranked by a 6-stage pipeline:

1. **RRF fusion** — combines BM25 and semantic scores (adaptive alpha)
2. **File coherence** — boost files with multiple matching chunks
3. **Definition boost** — 3x for chunks defining the queried symbol
4. **Stem matching** — boost files whose path contains query terms
5. **Import graph proximity** — boost files imported by/importing top results
6. **Noise penalties** — penalize test files, compat shims, `.d.ts`

### Import Graph

prx extracts `import`/`use`/`require` statements from 7 languages and builds
a dependency graph. Files within 2 hops of top-ranked results get a proximity
boost. The graph is persisted to `.prx/index/imports.bin` when you run
`prx index`.

Supported: Rust, Python, JavaScript/TypeScript, Go, Java, C/C++, Ruby.

## Find

```bash
prx find src/ --pattern "*.ts"            # filter by glob
prx find src/ --depth 3                   # limit depth
prx find src/ --changed-since HEAD~3      # recently modified
prx find . --tree-only                    # tree structure only
prx find . --flat-only                    # flat list only
```

## Edit

```bash
prx edit src/auth.ts --find "old_api()" --replace "new_api()"  # dry-run (default)
prx edit src/auth.ts --find "old_api()" --replace "new_api()" --apply  # apply
prx edit src/auth.ts --find "TODO.*" --replace "" --regex      # regex mode
prx edit src/auth.ts --find "x" --replace "y" --in-function "handleLogin"  # scoped
```

## Diff

```bash
prx diff                                  # all changed files vs HEAD
prx diff src/auth.ts                      # single file
prx diff --since HEAD~3                   # compare against ref
prx diff --staged                         # staged changes
prx diff --stat-only                      # summary only (~30 tokens)
```

## Run

Parses test/build/lint output into structured JSON. Only failures and
summaries are returned — passing tests are omitted.

```bash
prx run cargo test        # 95-99% token savings
prx run cargo clippy      # only warnings/errors
prx run pytest            # parsed test results
prx run npm test          # jest/vitest parsed
prx run go test ./...     # go test parsed
prx run tsc --noEmit      # TypeScript errors only
prx run eslint src/       # ESLint parsed
```

**Benchmark** (304-test Rust project, all passing):

| Method | Tokens |
|---|---|
| Raw `cargo test` output | ~6,000 |
| `prx run cargo test` | ~120 |
| **Savings** | **98%** |

## Exists

O(1) bloom filter check. Use before full searches when you need a yes/no.

```bash
prx exists "authenticate" src/     # returns { "exists": true/false }
```

## Outline

Symbol table for files or directories.

```bash
prx outline src/auth.ts            # single file
prx outline src/ --depth 2         # directory
prx outline src/ --kind function   # filter by kind
```

## Index

Persistent search index for faster repeated searches. Includes BM25 index,
chunk data, and import graph.

```bash
prx index .                        # build (skips if current)
prx index . --rebuild              # force rebuild
prx index . --stats                # show index stats
```

## Batch

Execute multiple commands in one call via JSONL on stdin.

```bash
echo '{"cmd":"read","file":"src/auth.ts","skeleton":true}
{"cmd":"exists","pattern":"redis","path":"src/"}' | prx batch
```

## Stats & Benchmarks

```bash
prx stats                          # total token savings
prx stats --compare                # per-command breakdown
prx bench .                        # synthetic: prx vs grep+cat
```

## Real-World Benchmarks (v0.2.0)

Measured across real agent sessions on production codebases.

### Token Savings by Feature

| Feature | Scenario | Savings |
|---|---|---|
| `--if-changed` (cache hit) | Re-reading unchanged file | **99.1%** |
| `--mode diff` | File with local changes | **98.7%** |
| `--mode diff` | Clean file (no changes) | **99.9%** |
| `--mode entropy` | Generated code (50+ fields) | **86%** |
| `--mode aggressive` | Python with docstrings | **11-19%** |
| `--mode aggressive` | Clean Rust code | **1-7%** |
| `prx run` | 304 passing tests | **98%** |
| `prx read --skeleton` | Full file → signatures | **~90%** |

### Cumulative Savings (from telemetry)

| Metric | Value |
|---|---|
| Total calls measured | 200+ |
| Total tokens saved | 36,114+ |
| Highest per-call savings | `run` at 52.9% average |
| Highest absolute savings | `read` at 46.3% average |

### Optimal Workflow

1. `prx exists` — check before searching (O(1), ~0 tokens)
2. `prx search` — find relevant code (ranked, budgeted)
3. `prx read --skeleton` — understand file structure (~10% of full)
4. `prx read --lines N-M --snap function` — read specific functions
5. `prx read --if-changed HASH` — skip unchanged files on re-read
6. `prx read --mode diff` — see only what changed
7. `prx run cargo test` — verify with 95-99% token savings
