# prx (Praxis)

Agent-native Unix tools. Single binary replacing grep, cat, find, sed, diff
for AI coding agents.

prx returns structured JSON with token budgets, structural awareness, and
content hashing. It ships as a single static binary with an embedded semantic
search model. No runtime dependencies. No internet required. No setup.

## Why

AI coding agents spend 30-93% of their tokens on exploration. The core loop --
grep for a pattern, read files for context, grep again -- wastes 93% of consumed
tokens because Unix tools return human-shaped output that agents must re-parse.

ag fixes this at the source. Instead of compressing human tool output after the
fact, ag returns exactly what agents need: labeled fields, ranked results, and
token-budgeted responses.

## Tools

| Command | Replaces | What it does |
|---|---|---|
| `prx search` | grep, rg | Hybrid search: literal + semantic + structural. Token-budgeted, ranked results with enclosing function context. |
| `prx read` | cat, head, tail | Structured file reading. Skeleton mode (signatures only), structural snapping, content hashing. |
| `prx find` | find, ls, tree | Codebase mapping. Dual tree+flat output, inline metadata, .gitignore-aware. |
| `prx edit` | sed, awk | Safe edits. Literal matching, dry-run by default, syntax validation, scoped to functions. |
| `prx diff` | diff, git diff | Semantic diffs. Natural language summaries, function-level attribution, move detection. |
| `prx exists` | grep -q | O(1) bloom filter existence check. ~0 tokens. |
| `prx outline` | ctags | Symbol table for a file or directory. |
| `prx mcp` | -- | MCP server over stdio for direct agent integration. |
| `prx run` | -- | Structured command runner. Parses test/build/lint output, returns only failures and warnings. 95-99% token reduction on test output. |
| `prx batch` | xargs | Parallel JSONL batch execution. One round-trip, multiple results. |

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

# Check if something exists before searching (~0 tokens)
prx exists "redis" src/

# Everything at once
echo '{"cmd":"search","query":"auth","budget":300}
{"cmd":"read","file":"src/auth.ts","skeleton":true}' | prx batch
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
        "context_signature": "async handleLogin(req: Request): Promise<Response>",
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

### AGENTS.md / CLAUDE.md

Add to your project's AGENTS.md:

```markdown
## Code Search

Use `prx search` instead of grep for finding code:

    prx search "authentication flow" .
    prx search --literal "authenticate(" src/
    prx read src/auth.ts --skeleton
    prx read src/auth.ts --snap function --lines 42-67
```

## Install

```bash
# Prebuilt binaries (Linux, macOS, Windows)
curl -L https://github.com/civitas-io/prx/releases/latest/download/ag-$(uname -s)-$(uname -m) -o ag
chmod +x ag

# Via cargo
cargo install prx

# Via homebrew
brew install civitas-io/tap/prx
```

## How Search Works

ag's search is derived from [Semble](https://github.com/MinishLab/semble) (MIT).
It combines three retrieval methods:

- **Literal**: regex matching at ripgrep speed
- **Semantic**: 16M-parameter static embedding model (Model2Vec), embedded in the
  binary, runs on CPU in milliseconds
- **Structural**: AST pattern matching via tree-sitter (e.g., `fn $NAME($$$)`)

Results are fused via Reciprocal Rank Fusion and reranked with code-aware
signals: definition boost, identifier stem matching, file coherence, noise
penalties (test files, compat shims), and saturation decay.

Benchmarks target NDCG@10 >= 0.85, matching Semble at 99% of transformer
quality with 200x faster indexing.

## Platform Support

| Platform | Status |
|---|---|
| Linux x86_64 | Supported |
| Linux aarch64 | Supported |
| macOS Apple Silicon | Supported |
| macOS Intel | Supported |
| Windows x86_64 | Supported |

Single static binary. No runtime dependencies.

## Status

Pre-alpha. Documentation-first phase. See [ROADMAP](docs/vision/ROADMAP.md).

## License

MIT
