# index

Builds a persistent search index: BM25, semantic embeddings, import graph, and symbol definitions. Run once, search faster thereafter.

## Usage

```bash
prx index [options] [path]
```

## Options

| Flag | Description |
|---|---|
| `--rebuild` | Force a full rebuild even if the index is current |
| `--stats` | Show index statistics |
| `--plain` | Human-readable output |

## Examples

```bash
# Build index for current directory
prx index .

# Force rebuild
prx index . --rebuild

# Show what's in the index
prx index . --stats
```

## What gets indexed

A single parallel pass builds five artifacts:

1. **BM25 sparse index** — for literal and keyword search
2. **Semantic embeddings** — float16 vectors for semantic search
3. **Import graph** — dependency edges extracted from AST
4. **Symbol index** — definition lookup and reference counting
5. **Chunk data** — code chunks with metadata

All five stages run in parallel via rayon. On a 10-core machine, indexing is 7.6x faster than sequential.

## Incremental rebuilds

`prx index` skips unchanged files. Only files that have changed since the last index run are re-processed. On large codebases, incremental rebuilds are much faster than full rebuilds.

## Index location

The index is stored in `.prx/index/` in the project root. It's safe to add `.prx/` to `.gitignore`.

## Performance

| Codebase | Files | Chunks | Time |
|---|---|---|---|
| Flask (Python, 15K LOC) | 259 | 1,225 | 0.3s |
| ripgrep (Rust, 25K LOC) | 239 | 2,465 | 0.6s |
| fastify (TypeScript, 15K LOC) | 417 | 2,529 | 0.6s |
| cargo (Rust, 150K LOC) | 2,815 | 12,118 | 5s |
| terraform (Go, 2M LOC) | 5,323 | 22,798 | 10s |
| django (Python, 300K LOC) | 5,690 | 30,944 | 32s |
| kafka (Java, 500K LOC) | 7,231 | 63,740 | 114s |
| vscode (TypeScript, 1M LOC) | 14,643 | 136,056 | 340s |

Measured on 10-core Apple Silicon. On 4-core CI runners, expect ~3-4x speedup over sequential.

## Zero-copy embeddings

Embedding vectors are memory-mapped directly from disk via `memmap2` and cast to `&[f32]` with zero allocation using `bytemuck`. The OS page cache keeps the index warm across queries. On an 11K-file codebase with 54 MB of embeddings:

- Zero bytes allocated for embedding data (OS manages the pages)
- Queries after the first hit warm cache, sub-millisecond embedding access
- Falls back to owned allocation automatically if mmap isn't available (network FS, etc.)

## Tips

- Run `prx index .` once at the start of a project. Subsequent searches use the persistent index and are faster.
- The import graph built by `prx index` is what powers `prx impact` and the proximity boost in `prx search`. Without an index, both fall back to slower on-demand extraction.
- Add `.prx/` to `.gitignore`. The index is machine-specific and regenerates quickly.
- On CI, you can cache `.prx/index/` between runs to avoid re-indexing unchanged code.

See also: [search](search.md), [impact](impact.md), [context](context.md)
