# search

Hybrid code search combining literal, semantic, and structural retrieval. Results are ranked and token-budgeted.

## Usage

```bash
prx search [options] <query> [path]
```

## Options

| Flag | Description |
|---|---|
| `--literal` | Exact regex match at ripgrep speed |
| `--structural` | AST pattern matching via tree-sitter |
| `--top-k N` | Return top N results (default: 5) |
| `--budget N` | Cap total output at N tokens |
| `--plain` | Human-readable output instead of JSON |

## How it works

prx fuses three retrieval methods into one ranked result:

- **Literal** — regex matching at ripgrep speed
- **Semantic** — the embedded potion-retrieval-32M model (PCA-reduced to 256 dims, float16); runs on CPU in milliseconds, no server
- **Structural** — AST pattern matching via tree-sitter

The query type is auto-detected. Natural language queries use semantic search. Queries that look like identifiers or patterns use literal matching. You can override with `--literal` or `--structural`.

Results are combined with Reciprocal Rank Fusion and reranked through a 6-stage pipeline:

1. **RRF fusion** — combines BM25 and semantic scores with adaptive alpha
2. **File coherence** — boost files with multiple matching chunks
3. **Definition boost** — 3x for chunks defining the queried symbol
4. **Stem matching** — boost files whose path contains query terms
5. **Import graph proximity** — boost files imported by or importing top results
6. **Noise penalties** — penalize test files, compat shims, `.d.ts`

## Examples

```bash
# Semantic search — auto-detected from natural language
prx search "authentication flow" src/

# Exact match — ripgrep speed
prx search --literal "authenticate(" src/

# AST pattern — match all function definitions
prx search --structural 'fn $NAME($$$) { $$$ }' src/

# More results with a token cap
prx search "auth" src/ --top-k 10 --budget 2000
```

Example output:

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

## Import graph

prx extracts `import`/`use`/`require` statements from 7 languages and builds a dependency graph. Files within 2 hops of top-ranked results get a proximity boost. The graph is persisted to `.prx/index/imports.bin` when you run `prx index`.

Supported languages: Rust, Python, JavaScript/TypeScript, Go, Java, C/C++, Ruby.

## Tips

- Use `prx exists` first for a yes/no check before committing to a full search.
- Run `prx index .` once to build a persistent index. Subsequent searches are faster and use the import graph for proximity boosting.
- For symbol lookups (function names, type names), `--literal` is usually faster and more precise than semantic search.
- For "what does this module do?" style questions, semantic search is the right mode.
- Use `--structural` with tree-sitter patterns to find all instances of a code shape, e.g. all async functions, all struct definitions.

See also: [exists](exists.md), [index](index.md), [read](read.md)
