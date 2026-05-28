# Search Quality: NDCG Analysis & Improvement Plan

Measured May 2026. Updated after Tier 4 symbol index.

All NDCG scores below use file-level deduplication (each file counted once,
regardless of how many chunks matched). Earlier saved results were inflated
by a measurement bug that counted duplicate file entries; corrected May 2026.

---

## v0.4.0 Results (after Tier 4 symbol index)

| Metric | v0.3.0 (corrected) | v0.4.0 | Delta |
|---|---|---|---|
| Fiddler NDCG@10 | 0.451 | 0.494 | +9.5% |
| Fiddler semantic | 0.470 | 0.470 | ~0% |
| Fiddler architecture | 0.526 | 0.526 | ~0% |
| Fiddler symbol | 0.263 | 0.619 | +135% |
| Complete misses | 13 | 9 | -4 recovered |
| prx NDCG@10 | 0.639 | 0.681 | +6.6% |
| Tests | 402 | 333 (unit) | +7 |

Recovered symbol queries: ConfigurationManager (0→0.798), EventStore
(0→0.290), feature_impact (0→0.371), fiddlerAPIClient (0→0.787).

Remaining misses (9, all semantic): sentiment analysis enrichment,
Fiddler Query Language parser, Alembic database migration, Redux store
configuration, data drift detection, dashboard chart component, toxicity
detection, time series metric, session management tokens.

## v0.5.1 Results (after tree-sitter imports)

| Metric | v0.4.0 | v0.5.1 | Delta |
|---|---|---|---|
| Fiddler NDCG@10 | 0.494 | 0.494 | ~0% (no regression) |
| Fiddler semantic | 0.470 | 0.470 | ~0% |
| Fiddler architecture | 0.526 | 0.526 | ~0% |
| Fiddler symbol | 0.619 | 0.617 | ~0% (noise) |
| Complete misses | 9 | 9 | unchanged |
| prx NDCG@10 | 0.681 | 0.673 | -1.2% (noise) |

Tree-sitter import rewrite produced no regression. The 9 remaining
misses are all semantic queries unrelated to import extraction.
Improvements from tree-sitter imports will show on repos with complex
import patterns (re-exports, dynamic imports, multiline), which the
current fiddler dataset doesn't specifically test.

---

## Corrected v0.3.0 Baseline

All scores use file-level deduplication (corrected May 2026).

### prx codebase (self-evaluation)

50 queries, 173 Rust files. Ground truth hand-labeled by the author.

| Metric | Score |
|---|---|
| NDCG@5 | 0.620 |
| NDCG@10 | 0.639 |
| Semantic (n=34) | 0.738 |
| Symbol (n=10) | 0.372 |
| Architecture (n=6) | 0.524 |

Dataset: `benchmarks/ndcg_dataset.json`
Results: `benchmarks/ndcg_results_prx.json`

### External codebase (validation)

49 queries, 11,021 files (Python/TypeScript/JavaScript). Ground truth
hand-labeled after codebase exploration.

| Metric | Score |
|---|---|
| NDCG@5 | 0.410 |
| NDCG@10 | 0.451 |
| Semantic (n=38) | 0.470 |
| Symbol (n=6) | 0.263 |
| Architecture (n=5) | 0.526 |

Dataset: `benchmarks/ndcg_dataset_fiddler.json`
Results: `benchmarks/ndcg_results_fiddler.json`

### Competitor reference points

| Tool | NDCG@10 | Dataset | Notes |
|---|---|---|---|
| Semble | 0.854 | Their benchmark (CodeSearchNet-derived) | BM25 + Model2Vec + code-aware reranking, no graph |
| CodeRankEmbed Hybrid | 0.862 | Same benchmark | 137M-param transformer, 57s index, 16ms query |
| ripgrep | ~0.13 | Semble's benchmark | Literal text only |

Semble achieves 99% of CodeRankEmbed's quality at 218x faster indexing and
11x faster query. No explicit symbol graph.

---

## Failure Mode Analysis

### 1. Symbol queries are broken at scale (NDCG 0.239)

`ConfigurationManager` appears in ~200 files as an import but is defined in
one file. BM25 scores all files similarly because the compound identifier
splits into `configuration` + `manager` -- both common tokens with near-zero
IDF. The definition file drowns in noise.

The existing definition boost in `ranking/boosting.rs` was tuned on the prx
codebase (145 Rust files) and likely does not fire correctly on Python/TypeScript
definition patterns. Even when it fires, the boost multiplier is insufficient
to overcome a 200:1 import-to-definition ratio.

**Complete misses**: `ConfigurationManager`, `EventStore`, `feature_impact`,
`fiddlerAPIClient` -- all symbols that appear pervasively as imports.

### 2. Semantic search is architecturally capped

The dense index is built at query time from the same chunks BM25 already found.
Semantic search can only rescore BM25 candidates -- it can never rescue a BM25
miss. When BM25 fails to surface `enrichments/sentiment.py` for "sentiment
analysis enrichment", semantic search never gets a chance either.

**Complete misses**: "sentiment analysis enrichment", "PII detection enrichment",
"toxicity detection", "faithfulness coherence evaluation" -- domain-specific
terms where BM25 recall fails and semantic can't compensate.

### 3. Chunks lack contextual metadata

A chunk containing `def enrich(self, event):` in `enrichments/sentiment.py`
tells neither BM25 nor the embedding model anything about sentiment. The file
path, parent class name, and module context are absent from the chunk text.

---

## Improvement Plan (ranked by ROI)

### Tier 1: Fix structural issues (5-6 days, target ~0.60-0.62)

| # | Item | Effort | Expected gain |
|---|---|---|---|
| 1 | Symbol-query ranking overhaul | 1d | +0.06-0.10 |
| 2 | Chunk header enrichment | 1d | +0.04-0.06 |
| 3 | Persistent dense index | 3-4d | +0.05-0.10 |

**Item 1: Symbol-query ranking overhaul**

Detect symbol-intent queries (single PascalCase/snake_case token). Preserve the
compound identifier as a high-weight BM25 term so IDF is not diluted by split
components. Apply 5-10x rerank boost when tree-sitter identifies a definition
node matching the query. Apply 0.2x penalty when matches occur only inside
import/use lines.

Verify definition boost fires on Python/TypeScript (currently may be Rust-only).

**Item 2: Chunk header enrichment**

Prepend each chunk with `[lang] path/to/file.py :: EnclosingClass.method`.
BM25 gets high-IDF terms (filenames, class names). Embeddings get semantic
anchor context. Pure indexing change, compounds across both modalities.

Watch out: all chunks sharing a path prefix can swamp BM25. Either
field-weight the header separately or strip common prefixes.

**Item 3: Persistent dense index**

Pre-compute and store all chunk embeddings at index time. For fiddler's 55k
chunks x 256-dim x 4 bytes = 56 MB. Flat inner-product search is fine at
this scale.

At query time, retrieve top-K from both BM25 and dense index independently,
then fuse via RRF. This is the structural unlock -- until semantic candidates
can compete with BM25 candidates on equal footing, semantic improvements have
a low ceiling.

### Tier 2: Tune and expand (3-4 days, target ~0.65-0.68)

| # | Item | Effort | Expected gain |
|---|---|---|---|
| 4 | Sharper mode detection | 1d | +0.02-0.04 |
| 5 | Reranker ablation + reweighting | 1-2d | +0.02-0.05 |
| 6 | Chunk overlap + size sweep | 1d | +0.01-0.03 |

**Item 4**: For symbol queries: alpha ~0.1, exact-match BM25, skip semantic.
For multi-word NL: alpha ~0.6. Add a static synonym dict for common domains
(auth/authentication, db/database, k8s/kubernetes). No LLM, no latency hit.

**Item 5**: Run leave-one-out NDCG over the 49 fiddler queries per rerank
stage. On 11k-file codebases, import-graph proximity and file coherence may
add noise. Grid-tune weights on a held-out split.

**Item 6**: 1500 chars with no overlap loses signal at boundaries. Add
150-200 char overlap. Benchmark 800/1500/2500 char variants on the same
49 queries.

### Tier 3: Model upgrade (2-3 days, target ~0.70-0.73, gated on Tier 1 #3)

| # | Item | Effort | Expected gain |
|---|---|---|---|
| 7 | Upgrade embedding model | 2-3d | +0.05-0.10 |

Candidates: jina-embeddings-v2-base-code (161M, code-tuned) or
nomic-embed-text-v1.5 (137M). Do not do this before persistent dense index
(Tier 1 #3) -- per-query embedding cost on a larger model only makes sense
if embeddings are computed once at index time. Consider download-on-first-use
to avoid binary bloat.

### Tier 4: Symbol index (1-2 weeks, target ~0.70-0.75)

| # | Item | Effort | Expected gain |
|---|---|---|---|
| 8 | Symbol index with reference counting | 1-2w | +0.03-0.08 |

Build a lightweight symbol index at index time: map each symbol name to its
definition location (file + line + kind) and count how many chunks reference
it. This is 80% of a full code graph's value at 20% of the cost.

For symbol queries, look up the definition directly instead of relying on
BM25. Use reference count as a ranking signal (widely-referenced symbols
rank higher).

This does NOT require a full call graph, inheritance tree, or PageRank.

---

## Symbol Graph: Full Analysis

### What prx already has

File-level import graph in `search/graph.rs` -- directed edges (A imports B),
BFS traversal up to 2 hops, persisted as binary. Symbol extraction in
`parsing/outline.rs` -- 10 symbol kinds with nesting (methods-of-class).
The infrastructure for building, persisting, and querying relationship
graphs is proven.

### What a full symbol graph would add

Instead of "file A imports file B":
- `ConfigurationManager` (class, defined in `manager.py`) <- referenced by 200 files
- `manager.py::ConfigurationManager.get()` -> calls `config_store.py::ConfigStore.fetch()`
- `SentimentEnricher` extends `BaseEnrichment` in `enrichments/base.py`

### Evidence from industry and research

**Sourcegraph** uses PageRank on a symbol graph for search ranking. Steve
Yegge (2022): "Sourcegraph's new search ranking uses a rendition of the Google
PageRank algorithm on source code, powered by the code symbol graph."

**LARGER** (2026, arxiv 2605.16352): Lexical search + graph expansion improves
file-level Acc@5 by +13.9 points over strongest baseline.

**RANGER** (2025, arxiv 2509.25257): MCTS-guided graph traversal with
cross-encoder reranking beats embedding-only baselines including Qwen3-8B
on CodeSearchNet and RepoQA.

**GRACE** (2025, arxiv 2509.05980): Multi-level code graphs + hybrid retrieval
improve code completion by +8.19% EM over graph-based RAG baselines.

**CKB v7.4**: Personalized PageRank over symbol graphs re-ranks FTS results.
Edge weights: Call=1.0, Definition=0.9, Reference=0.8, Implements=0.7,
Type-of=0.6, Same-module=0.3.

**Semble** achieves 0.854 NDCG@10 WITHOUT any graph -- purely through
BM25 + embeddings + code-aware reranking. This is the counterpoint: explicit
graphs are not necessary for high-quality search ranking if reranking
signals approximate graph properties.

### Recommendation

Build the symbol index (Tier 4) before considering a full graph. The symbol
index (definitions + reference counts) solves the primary failure mode
(symbol queries at 0.239) without the complexity of call graphs, inheritance
trees, or PageRank.

A full weighted graph (call edges, inheritance, type references) is justified
only if:
1. Symbol index + Tiers 1-3 plateau below 0.65 on fiddler
2. Users request impact analysis (`prx impact "what breaks if I change X?"`)
3. Users request entity queries ("find all callers of function X")

### Implementation feasibility

Tree-sitter exposes all necessary AST structure. The existing `outline.rs`
already extracts symbol definitions with nesting. `graph.rs` proves
graph persistence works. Estimated effort for a full symbol graph:

- Call graph extraction: ~200-300 lines per language x 13 languages, 2-3 days
- Inheritance extraction: ~150-200 lines per language, 2 days
- Name/type resolution: the hard part -- mapping "foo()" to actual definition
  requires scope analysis, 1-2 weeks
- Graph persistence + query API: 2-3 days (follow graph.rs pattern)
- Ranking integration (PPR or in-degree scoring): 1-2 days

Total: 4-6 weeks for a production-quality symbol graph.

---

## Realistic ceiling

| Scope | Expected NDCG@10 (external) | Status |
|---|---|---|
| Tiers 1-3 (v0.3.0) | 0.451 | Done |
| + Tier 4 (symbol index) | 0.494 | Done |
| + Full symbol graph | 0.55-0.65 | Planned |
| + Cross-encoder reranker | 0.70+ | Planned |

Breaking 0.70 on unfamiliar large codebases likely requires a cross-encoder
reranker (~25M params, scores query+chunk jointly over top-50 candidates).
This adds 20-80ms latency and a second model to the binary.
