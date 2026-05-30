# Public Benchmark Suite

## Overview

The prx benchmark suite measures search quality (NDCG@10) across 200 labeled queries on 8 public repositories. It's designed to be reproducible, honest, and runnable by anyone.

- **200 queries** across 8 repos
- **6 languages**: Python, Rust, TypeScript, Java, Go
- **3 size tiers**: small (< 500 files), medium (500-10K files), large (> 10K files)
- All repos pinned by commit SHA
- Ground truth in `benchmarks/repos/`

## Running the benchmark

```bash
# Run against the standard dataset
prx bench-ndcg benchmarks/dataset.json

# Human-readable output
prx bench-ndcg benchmarks/dataset.json --plain
```

The benchmark loads the index once and runs all queries against cached data. A 50-query suite runs in 0.23 seconds.

## Dataset format

The dataset is a JSON file with labeled queries:

```json
{
  "repo": "pallets/flask",
  "commit": "abc123...",
  "queries": [
    {
      "query": "request context handling",
      "relevant_files": [
        "src/flask/ctx.py",
        "src/flask/globals.py"
      ],
      "query_type": "semantic"
    }
  ]
}
```

Each query has a set of ground-truth relevant files. NDCG@10 measures how well prx ranks those files in the top 10 results.

## Interpreting results

The output reports NDCG@10 per repo and overall, broken down by search mode:

```json
{
  "repo": "flask",
  "queries": 25,
  "ndcg10": 0.710,
  "symbol_ndcg10": 0.805,
  "semantic_ndcg10": 0.662,
  "misses": 0
}
```

- `ndcg10`: hybrid search (the default)
- `symbol_ndcg10`: literal/symbol search only
- `semantic_ndcg10`: semantic search only
- `misses`: queries where no relevant file appeared in the top 10

A `miss` means the relevant file wasn't in the top 10 at all. Misses are the most actionable signal for improving search quality.

## v0.5.7 results

| Repo | Language | Size | Files | NDCG@10 | Misses |
|---|---|---|---|---|---|
| Flask | Python | small | 259 | 0.710 | 0 |
| ripgrep | Rust | small | 239 | 0.493 | 4 |
| fastify | TypeScript | small | 417 | 0.432 | 5 |
| cargo | Rust | medium | 2,815 | 0.379 | 7 |
| kafka | Java | medium | 7,231 | 0.354 | 11 |
| django | Python | medium | 5,690 | 0.262 | 9 |
| terraform | Go | large | 5,323 | 0.287 | 9 |
| vscode | TypeScript | large | 14,643 | 0.208 | 16 |

Overall average: 0.391. Symbol search average: 0.681.

## CI regression gate

The benchmark suite runs in CI on every release. A regression in NDCG@10 of more than 0.02 on any repo blocks the release.

To run the CI check locally:

```bash
prx bench-ndcg benchmarks/dataset.json --threshold 0.02
```

Returns exit code 0 if no regression, exit code 1 if any repo regressed beyond the threshold.

## Adding queries

To add queries to the dataset, add entries to the relevant repo's query list in `benchmarks/repos/<repo>/queries.json`. Each query needs:

1. A natural language query string
2. A list of ground-truth relevant files (relative paths)
3. A query type (`semantic`, `symbol`, or `architecture`)

Ground truth is determined by human judgment: which files would a developer actually want to find for this query?

See also: [Search Quality](search-quality.md), [Indexing Performance](indexing.md)
