# Indexing Performance

## Parallel indexing: 7.6x speedup

`prx index` builds a persistent search index in a single parallel pass. All five stages run on all available CPU cores via rayon:

1. Read, hash, and chunk files
2. Build BM25 sparse index
3. Compute semantic embeddings
4. Extract import graph from AST
5. Build symbol index

No shared mutable state, no Arc, no Mutex. Pure `par_iter` on thread-safe immutable data. BLAS thread limits prevent oversubscription.

## Benchmark results

Measured on 10-core Apple Silicon (944% CPU utilization):

| Codebase | Language | Files | Chunks | Time |
|---|---|---|---|---|
| Flask | Python | 259 | 1,225 | 0.3s |
| ripgrep | Rust | 239 | 2,465 | 0.6s |
| fastify | TypeScript | 417 | 2,529 | 0.6s |
| cargo | Rust | 2,815 | 12,118 | 5s |
| terraform | Go | 5,323 | 22,798 | 10s |
| django | Python | 5,690 | 30,944 | 32s |
| kafka | Java | 7,231 | 63,740 | 114s |
| vscode | TypeScript | 14,643 | 136,056 | 340s |

On CI runners with 4 cores, expect ~3-4x speedup over sequential. On a single core, indexing is still correct but slower.

## Incremental rebuilds

`prx index` tracks file hashes and skips unchanged files. Only files that have changed since the last index run are re-processed. For a codebase where 10% of files changed, an incremental rebuild takes roughly 10% of the full rebuild time.

## Zero-copy memory-mapped embeddings

Embedding vectors are stored in `embeddings.bin` and memory-mapped via `memmap2`. They're cast to `&[f32]` with `bytemuck::cast_slice`: zero allocation, zero deserialization. The OS page cache keeps the index warm across queries.

On an 11K-file codebase with 54 MB of embeddings:

- **Zero bytes** allocated for embedding data (OS manages the pages)
- Queries after the first hit warm cache, sub-millisecond embedding access
- Falls back to owned `Array2<f32>` automatically if mmap isn't available (network FS, etc.)

The `Embeddings` enum abstracts both paths behind a single `view() -> ArrayView2<f32>` API, so the rest of the search pipeline doesn't need to know which path is active.

## bench-ndcg: 55x speedup with load-once

`prx bench-ndcg` measures search quality (NDCG@10) against labeled datasets. It loads the index once and runs all queries against cached data:

| Benchmark | Before (v0.5.5) | After (v0.5.6) | Speedup |
|---|---|---|---|
| 50-query NDCG suite | 12.76s | 0.23s | 55x |

The speedup comes from loading the index once per benchmark run instead of once per query. The index load dominates query time on warm cache.

## Index location and caching

The index is stored in `.prx/index/` in the project root. It's safe to add `.prx/` to `.gitignore`.

On CI, you can cache `.prx/index/` between runs. The index is invalidated automatically when files change (via content hashing), so stale cache entries are never used.

See also: [index command](../commands/index.md), [Public Benchmark Suite](benchmarks.md)
