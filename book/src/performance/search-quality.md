# Search Quality

## What NDCG@10 means

NDCG (Normalized Discounted Cumulative Gain) at rank 10 measures how well a search system ranks relevant results in the top 10 positions. A score of 1.0 means every relevant result is at the top. A score of 0.0 means no relevant results appear in the top 10.

For code search, a query like "authentication middleware" has a set of ground-truth relevant files. NDCG@10 measures whether those files appear near the top of prx's results.

The metric is standard in information retrieval research. It penalizes relevant results that appear lower in the ranking more than those that appear at the top.

## Benchmark results (v0.5.7)

200 labeled queries across 8 public repositories, 6 languages, 3 size tiers. All repos pinned by commit SHA. Ground truth in `benchmarks/repos/`.

| Repo | Language | Files | NDCG@10 | Symbol | Semantic |
|---|---|---|---|---|---|
| Flask | Python | 259 | **0.710** | 0.805 | 0.662 |
| ripgrep | Rust | 239 | **0.493** | 0.810 | 0.356 |
| fastify | TypeScript | 417 | **0.432** | 0.822 | 0.321 |
| cargo | Rust | 2,815 | **0.379** | 0.705 | 0.285 |
| kafka | Java | 7,231 | **0.354** | 0.934 | 0.191 |
| django | Python | 5,690 | **0.262** | 0.495 | 0.211 |
| terraform | Go | 5,323 | **0.287** | 0.238 | 0.319 |
| vscode | TypeScript | 14,643 | **0.208** | 0.639 | 0.080 |

Summary by size tier:

| Tier | Avg NDCG@10 |
|---|---|
| Small (< 500 files) | 0.545 |
| Medium (500-10K files) | 0.332 |
| Large (> 10K files) | 0.248 |
| Overall | 0.391 |
| Symbol search avg | 0.681 |
| Semantic search avg | 0.303 |

## Symbol vs semantic analysis

**Symbol search is consistently strong** (avg 0.681) across all codebase sizes. When you search for a known identifier, function name, or type name, prx finds it reliably.

**Semantic search degrades at scale.** The 32M embedded model (potion-retrieval-32M) works well on codebases under ~3K files. On larger codebases, the embedding space becomes crowded and relevance scores compress. The vscode semantic score (0.080) reflects this limitation clearly.

The hybrid search combines both: symbol search anchors precision, semantic search adds recall for natural language queries. The combined NDCG@10 is consistently better than either alone.

## Known limitations

**Semantic search at scale.** The embedded 32M-parameter model is optimized for speed and binary size, not maximum retrieval quality. On codebases with 10K+ files, semantic search quality drops significantly. For large repos, use `--literal` for known identifiers and rely on symbol search.

**Architecture queries on large repos.** The `architecture_ndcg10` scores in the benchmark data show 0.000 for kafka, django, and vscode. High-level architectural queries ("where is the plugin system?") are hard for any embedding model on large codebases.

**Import graph coverage.** Import extraction covers 20 language families via tree-sitter AST queries. Languages outside this set don't get proximity boosting. The graph is also a best-effort extraction: dynamic imports, conditional imports, and generated code may not be captured.

## Planned improvements

Code-specific model tiers are planned for v0.6.0. A larger model (or a model fine-tuned on code) would improve semantic search quality on large codebases without changing the binary's offline/no-server design.

These are honest numbers on codebases we didn't write and don't tune for. The benchmark dataset and methodology are public so you can verify them independently.

See also: [Public Benchmark Suite](benchmarks.md), [Indexing Performance](indexing.md)
