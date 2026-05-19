# prx (Praxis)

Agent-native Unix tools for AI coding agents. A single Rust binary replacing
grep, cat, find, sed, and diff with structured JSON output, token budgets,
and embedded semantic search.

Part of the [Civitas](https://github.com/civitas-io) ecosystem.

## Why

AI coding agents waste 30-93% of their tokens on exploration. The grep-read-grep
loop alone burns 93% of consumed tokens on output agents must re-parse. Test
output is worse: 164 passing tests produce ~1,200 tokens of "ok" lines that no
agent needs.

prx fixes this at the source. Instead of compressing human tool output after
the fact, prx returns exactly what agents need: labeled fields, ranked results,
and token-budgeted responses.

## Commands

| Command | Replaces | What it does |
|---|---|---|
| `prx search` | grep, rg | Hybrid search: literal + semantic + structural. Token-budgeted, ranked results. |
| `prx read` | cat, head, tail | Structured file reading. Skeleton mode, structural snapping, content hashing. |
| `prx find` | find, ls, tree | Codebase mapping. Dual tree+flat output, inline metadata. |
| `prx edit` | sed, awk | Safe edits. Literal matching, dry-run by default, syntax validation. |
| `prx diff` | diff, git diff | Semantic diffs. Natural language summaries, function-level attribution. |
| `prx run` | -- | Structured command runner. Parses test/build/lint output. 95-99% token savings. |
| `prx exists` | grep -q | O(1) bloom filter existence check. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx index` | -- | Persistent search index with validation. 6x faster repeated searches. |
| `prx mcp` | -- | MCP server over stdio for direct agent integration. |
| `prx batch` | xargs | Parallel JSONL batch execution. |
| `prx init` | -- | Auto-detect agent frameworks, generate integration configs. |
| `prx stats` | -- | Token savings dashboard with `--compare` for real-world savings. |
| `prx bench` | -- | Synthetic benchmark runner: prx vs grep+cat side-by-side. |

## Quick Start

```bash
# Search by meaning, not just text
prx search "authentication flow" src/

# Get file structure without reading it (~10% of tokens)
prx read src/auth.ts --skeleton

# Read just the function you need
prx read src/auth.ts --lines 42-67 --snap function

# Safe editing with preview
prx edit src/auth.ts --find "old_api()" --replace "new_api()" --dry-run

# Run tests with 95%+ token savings
prx run cargo test

# Check if something exists before searching (~0 tokens)
prx exists "redis" src/

# Build persistent index for faster searches
prx index .
```

## Output

All output is JSON by default:

```json
{
  "version": "0.1.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": {
    "matches": [
      {
        "file": "src/auth/handler.ts",
        "line": 42,
        "match": "authenticate",
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

Use `--plain` for human-readable output. Use `--budget N` to cap token usage.

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

### AGENTS.md

```bash
prx init --agents-md    # appends usage snippet to your AGENTS.md
prx init                # auto-detect frameworks, generate all configs
```

### Three Integration Tiers

1. **CLI on PATH** — works everywhere (top-level agents, sub-agents, scripts, CI)
2. **MCP server** — richer integration for top-level agents
3. **Agent definitions** — dedicated Claude Code sub-agent (`prx init --agent claude-code`)

## How Search Works

prx's search is derived from [Semble](https://github.com/MinishLab/semble) (MIT).
Three retrieval methods:

- **Literal**: regex matching at ripgrep speed
- **Semantic**: 16M-parameter static embedding model (Model2Vec), embedded in the binary
- **Structural**: AST pattern matching via tree-sitter (`fn $NAME($$$)`)

Results are fused via Reciprocal Rank Fusion and reranked with code-aware signals:
definition boost, identifier stem matching, file coherence, noise penalties, and
saturation decay.

## prx run — Structured Command Output

```bash
prx run cargo test      # 95-99% token savings on passing tests
prx run cargo clippy    # only warnings and errors
prx run pytest          # parsed test results
prx run npm test        # jest/vitest output parsed
```

Supports 9 tool parsers: cargo test, cargo build/clippy, pytest, go test,
jest/vitest, tsc, eslint, plus a fallback for unknown commands.

## Reliability

prx never breaks your agent's workflow. If an internal error occurs, prx
silently falls back to the equivalent Unix command (grep/cat/find) and returns
results in the same JSON envelope with `"fallback": true`. Errors are logged
to `~/.prx/errors.jsonl` for debugging.

## Token Savings Tracking

Every prx command logs actual vs baseline token counts automatically.

```bash
prx stats --compare     # per-command savings breakdown
prx bench .             # synthetic benchmark: prx vs grep+cat
```

## Install

### Prebuilt Binaries

Download from [GitHub Releases](https://github.com/civitas-io/prx/releases):

```bash
# Linux / macOS
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv prx /usr/local/bin/

# Verify
prx --version
```

### Build from Source

Requires Rust >= 1.85 and a C compiler (for tree-sitter grammars).

```bash
git clone https://github.com/civitas-io/prx.git
cd prx
make setup    # downloads models (~35MB), converts to float16, builds, tests
```

This takes about 2 minutes on first run. After setup:

```bash
make build    # debug build
make release  # optimized release build (~48MB)
make check    # fmt + clippy + all tests
```

The `make setup` step downloads embedding model weights from HuggingFace and
converts them to float16. These files are embedded into the binary at compile
time — no downloads happen at runtime. See [CONTRIBUTING](docs/CONTRIBUTING.md)
for the full developer guide.

### Available Make Targets

```
make setup      - First-time setup: download models, verify build
make check      - Run fmt, clippy, and all tests
make build      - Debug build
make release    - Release build (optimized, ~48MB)
make test       - Run all tests (unit + E2E)
make test-unit  - Run unit tests only
make test-e2e   - Run E2E integration tests only
make models     - Download and convert model files
make coverage   - Generate HTML coverage report
make clean      - Remove build artifacts
```

## Platform Support

| Platform | Status |
|---|---|
| Linux x86_64 | Supported |
| Linux aarch64 | Supported |
| macOS Apple Silicon | Supported |
| Windows x86_64 | Supported |

Single static binary. No runtime dependencies. No internet required after build.

## Current Status

| Metric | Value |
|---|---|
| Version | 0.1.0 |
| Commands | 14 |
| Tests | 304 (260 unit + 44 E2E) |
| Languages | 14 (tree-sitter grammars) |
| Release binary | ~48 MB (float16 model embedded) |
| CI | GitHub Actions (Linux x86_64, Linux aarch64, macOS arm64, Windows) |
| Fallback | Graceful fallback to grep/cat/find on internal errors |
| Telemetry | Real-world token savings tracking via `prx stats --compare` |

See [ROADMAP](docs/vision/ROADMAP.md) for what's next.

## Contributing

See [CONTRIBUTING](docs/CONTRIBUTING.md) for setup instructions, development
workflow, and how to add new commands, languages, and run parsers.

## License

Apache 2.0
