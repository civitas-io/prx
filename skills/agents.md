# prx — Agent Skill Guide

prx (Praxis) is a single Rust binary that replaces grep, cat, find, sed,
diff, and multi-tool workflows with agent-native equivalents. Every command
returns structured JSON with token counts and content hashes.

Standard Unix tools waste 30-93% of your tokens on output you re-parse.
prx returns exactly what you need.

**Single binary (~49 MB), zero dependencies, works offline/sandboxed. Embeds 32M-parameter retrieval-optimized embedding model. Parallel indexing via rayon (~17x speedup: 410s → 24s on 11K files). Zero-copy memory-mapped embeddings. 50-query benchmark in 0.23s.**

## When to Use prx Instead of Other Tools

| You're about to... | Use this instead | Why |
|---|---|---|
| `grep -r "pattern" src/` | `prx search "pattern" src/` | Ranked results, semantic + literal + AST search |
| `cat file.rs` | `prx read file.rs` | Metadata, outline, content hash, token count |
| `cat file.rs \| head -20` | `prx read file.rs --skeleton` | Signatures only (~10% tokens) |
| `find . -name "*.rs"` | `prx find . --pattern "*.rs"` | Tree + flat output, language detection |
| `sed -i 's/old/new/' file` | `prx edit file --find old --replace new` | Dry-run preview, syntax validation |
| `git diff` | `prx diff` | Function-attributed hunks, semantic summary |
| `cargo test` / `pytest` / `go test` | `prx run cargo test` | Only failures shown, 95-99% savings |
| `grep -q "pattern"` | `prx exists "pattern" .` | O(1) bloom filter, ~0 tokens |
| Read a file again | `prx read file --if-changed <hash>` | 50-byte stub if unchanged (99% savings) |
| Understand a module | `prx context src/auth/` | Stats + doc + entrypoints + files + edges in one call |
| "What breaks if I change this?" | `prx impact src/auth.rs` | Reverse dependency walk with symbol attribution |
| `outline` / `ctags` equivalent | `prx outline src/` | Tree-sitter symbol table (27 languages, nested) |
| Multiple independent queries | `prx batch < commands.jsonl` | Parallel execution, single response |

## Core Workflow

```bash
# 1. Quick existence check before searching (O(1))
prx exists "authenticate" src/

# 2. Search — semantic auto-detects for natural language
prx search "authentication flow" src/
prx search --literal "authenticate(" src/
prx search --structural 'fn $NAME($$$) { $$$ }' src/

# 3. Understand a module in one call
prx context src/auth/

# 4. Read structure before content (~10% tokens)
prx read src/auth.rs --skeleton

# 5. Read specific functions, not whole files
prx read src/auth.rs --lines 42-67 --snap function

# 6. Skip re-reading unchanged files (99% savings)
prx read src/auth.rs --if-changed <hash-from-previous-read>

# 7. See only changed lines vs git HEAD (98% savings)
prx read src/auth.rs --mode diff

# 8. Strip comments / filter repetitive code
prx read src/auth.py --mode aggressive    # strip comments (11-19%)
prx read schema.rs --mode entropy         # filter repetition (up to 86%)

# 9. Check impact before refactoring
prx impact src/auth.rs
prx impact src/auth.rs --symbol authenticate

# 10. Safe editing with preview
prx edit src/auth.rs --find "old_api()" --replace "new_api()"
prx edit src/auth.rs --find "old" --replace "new" --apply  # commit change

# 11. Run tests/build with structured output (95-99% savings)
prx run cargo test
prx run cargo clippy
prx run pytest
prx run go test ./...

# 12. Build search index for faster repeated queries
#     (parallel — uses all CPU cores, 11K files in ~24s, ~17x speedup)
prx index .
```

## New Commands (v0.4.0)

### `prx context` — Module Understanding in One Call

Instead of: `prx outline dir/ && prx find dir/ && cat dir/README.md && grep imports ...`

```bash
prx context src/auth/              # full module context
prx context src/auth/ --budget 2000 # capped output
prx context src/auth/ --no-edges   # skip import graph
```

Returns: file stats, documentation, top entrypoints (ranked by reference
count), file-level symbol skeletons, and 1-hop import graph edges — all
in one JSON response with budget enforcement.

### `prx impact` — Reverse Dependency Analysis

Instead of: `grep -r "import auth" . && grep -r "from auth" . && ...`

```bash
prx impact src/auth.rs                       # what depends on this file?
prx impact src/auth.rs --symbol authenticate # what uses this function?
prx impact src/auth.rs --hops 1              # direct dependents only
```

Returns: target exports, list of dependent files with hop distance and
symbol usage attribution, stats (direct/transitive/test counts).

## Token Budget

Use `--budget N` on any content-returning command:

```bash
prx search "auth" src/ --budget 500
prx read src/auth.rs --budget 1000
prx context src/ --budget 3000
prx impact src/lib.rs --budget 2000
```

## Conditional Reads

Every `prx read` response includes `meta.hash`. Pass it back to avoid
re-reading unchanged files:

```bash
prx read src/auth.rs --if-changed a3f9b2c1...
# Unchanged: 50-byte stub. Changed: full content.
```

## Structured Command Runner

`prx run` parses output from 22 tools, returning only failures/warnings. Use `--auto-json` to auto-inject JSON flags for kubectl, terraform, npm, eslint, mypy.

| Tool | What prx extracts |
|---|---|
| `cargo test` | Failed tests + summary |
| `cargo build` / `clippy` | Errors + warnings with file:line |
| `pytest` | Failed tests + tracebacks |
| `go test` | Failed tests + summary |
| `jest` / `vitest` | Failed tests + summary |
| `tsc` | Type errors with file:line |
| `eslint` | Lint errors with file:line |

A passing `cargo test` suite that outputs 50k tokens raw becomes ~200
tokens through prx.

## Output Format

All output follows a JSON envelope:

```json
{"version": "0.3.0", "command": "read", "status": "ok", "tokens": 487, "data": {...}}
```

Errors are also JSON on stdout (never stderr):

```json
{"status": "error", "error": {"code": "file_not_found", "message": "...", "suggestion": "..."}}
```

Use `--plain` for human-readable output.

## Estimated Token Savings

| Feature | Scenario | Savings |
|---|---|---|
| `--if-changed` (hit) | Re-reading unchanged file | **99%** |
| `--mode diff` | File with local changes | **98-99%** |
| `--mode entropy` | Generated code (50+ fields) | **86%** |
| `prx run` | Passing test suites | **95-99%** |
| `--skeleton` | Full file to signatures | **~90%** |
| `--mode aggressive` | Python with docstrings | **11-19%** |
| `prx context` vs manual | Module exploration (4-5 calls → 1) | **60-80%** |

## Installation

```bash
# Download binary
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv prx /usr/local/bin/

# Or: cargo install prx

# Auto-setup for your agent framework
prx init
```

### MCP Server

```json
{"mcpServers": {"prx": {"command": "prx", "args": ["mcp"]}}}
```

Works with Claude Code, Cursor, Codex, OpenCode. Sub-agents cannot call
MCP — use CLI on PATH for sub-agent access.

## Recommended Workflow Order

1. `prx exists` — yes/no before committing to a search
2. `prx read --skeleton` or `prx context` — structure before content
3. `prx search` — find code by meaning or pattern
4. `prx read --snap function` — read only what you need
5. `prx impact` — check blast radius before changing
6. `prx edit` — preview then apply
7. `prx run cargo test` — verify with minimal output
8. `prx index .` — build once, search faster thereafter

## Supported Languages (27)

**Full support** (parsing, imports, outline, snap):
Rust, Python, JavaScript/TypeScript, Go, Java, C/C++, Kotlin, Swift, C#, PHP, Elixir

**Parsing + imports**: Ruby, Bash, HTML, CSS

**Parsing + outline**: HCL/Terraform (resource/variable blocks), Makefile (rules + variables)

**Parsing only** (chunking, skeleton, aggressive mode): YAML, TOML, Markdown, Dockerfile, SQL, JSON

## Links

- Source: https://github.com/civitas-io/prx
- License: Apache 2.0
- Developer reference: [AGENTS.md](AGENTS.md)
