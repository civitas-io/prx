# Search Pipeline

prx uses a hybrid retrieval engine combining three search modes, fused and reranked into a single result set. This page explains how each stage works.

## Three Retrieval Modes

### Literal (`--literal`)

Regex matching at ripgrep speed. No embeddings are loaded, no index is consulted. Suitable for exact string or pattern searches where you know what you're looking for.

### Semantic (`--semantic`)

Full hybrid pipeline: chunk retrieval via BM25 and dense embeddings, RRF fusion, and reranking. Suitable for concept-level queries and natural language descriptions of what you're looking for.

### Structural (`--structural`)

AST pattern matching via ast-grep. Queries use metavariable syntax — for example, `fn $NAME($$$) { $$$ }` matches any Rust function. Returns structurally matched AST nodes rather than scored chunks.

### Auto-detection

When no mode flag is provided, the query is classified automatically:

- Fewer than 3 tokens, or contains regex metacharacters: `--literal`
- Contains `$VAR`-style metavariables: `--structural`
- Otherwise (natural language words, multi-token phrases): `--semantic`

## Chunking

Before indexing, source files are split into chunks. Chunking is syntax-aware via tree-sitter, targeting 1500 characters per chunk.

**Algorithm:**

1. Parse the file into an AST using the appropriate tree-sitter grammar.
2. Recursively traverse the tree, collecting leaf and intermediate nodes.
3. Merge adjacent sibling nodes greedily until the accumulated character count approaches the target.
4. When a single node exceeds the target, recurse into its children.
5. Emit each accumulated group as a chunk.

Chunks don't overlap. A character belongs to exactly one chunk. A function is never split unless it exceeds 1500 characters.

Files in unsupported languages fall back to line-based chunking at the same character budget.

## Embedding Model (Model2Vec)

Model: **potion-retrieval-32M** (MinishLab, PCA to 256 dims, float16). Embedded in the binary via `include_bytes!`. No network access, no filesystem reads at runtime.

This is not a transformer. There's no forward pass, no attention mechanism, no matrix multiplication through hidden layers. It's a static embedding table.

**Inference pipeline:**

1. Tokenize the input string against a fixed vocabulary (62,500 tokens).
2. Look up each token in a 62,500 × 256 embedding table.
3. Mean-pool the resulting vectors into a single 256-dimensional vector.
4. L2-normalize the pooled vector.

Because it's a table lookup followed by averaging, it runs on CPU only and is roughly 500x faster than transformer-based embedding models. No GPU required, no warm-up cost.

## BM25

BM25 is a classical information retrieval scoring function. It ranks documents by how often query terms appear in them, adjusted for document length. prx uses Robertson BM25 with k1=1.5, b=0.75.

Code identifiers require special handling because standard word tokenization destroys their semantics.

**Compound identifier tokenization:**

Identifiers are extracted via regex, then split on camelCase and snake_case boundaries. Both the original compound form and each sub-token are preserved.

```
getHTTPResponse → ["gethttpresponse", "get", "http", "response"]
```

No stemming is applied. Code identifiers are semantically distinct — `initialize` and `initial` mean different things and shouldn't be conflated.

**Content enrichment:**

Before BM25 indexing, each chunk's text is augmented with:
- The file stem, repeated twice (to increase its term frequency weight)
- The last 3 directory components of the file path

This makes file-name and directory-name terms retrievable via BM25 without separate metadata queries.

**Scoring:**

BM25 scores are pre-computed and stored in a CSC sparse matrix. At query time, scoring is a slice-and-sum operation: extract the column(s) for query terms, sum the values. No per-query document traversal.

## Reciprocal Rank Fusion

RRF (Reciprocal Rank Fusion) is a technique for combining ranked lists from multiple retrieval systems. It's robust to score scale differences between systems — it only cares about rank position, not raw scores.

**Formula:**

```
RRF_score = 1 / (k + rank)    where k = 60
```

Each retrieval system (semantic, BM25) produces an independent ranked list. RRF scores are computed separately for each list, then combined:

```
final_score = alpha * RRF(semantic) + (1 - alpha) * RRF(bm25)
```

**Adaptive alpha:**

- `alpha = 0.3` for symbol-like queries: heavier BM25 weight, since exact identifier matching dominates.
- `alpha = 0.5` for natural language queries: balanced weighting.

Symbol detection uses a regex heuristic matching patterns like `Foo::bar`, `_private`, `getUserById`.

Both retrievers fetch `top_k * 5` candidates before fusion. The expanded candidate pool is then reranked and trimmed to `top_k`.

## Reranking Pipeline

After RRF fusion, results pass through a 6-stage deterministic reranking pipeline. Stages apply in order.

### Stage 1: File Coherence Boost

Files where multiple chunks scored highly get their top chunk boosted. The boost is proportional to the file's aggregate score relative to the highest-scoring file:

```
boost = max_score * 0.2 * (file_aggregate / max_file_aggregate)
```

### Stage 2: Definition Boost

Chunks that define a queried symbol receive a score multiplier. Detection uses a keyword list: `class`, `def`, `fn`, `func`, `struct`, `enum`, `trait`, `interface`, and equivalents across languages. If the file stem also matches the symbol name, an additional multiplier applies.

For natural language queries: 4x multiplier. For symbol queries: 12x multiplier.

### Stage 3: Import Graph Proximity

Files in the dependency neighborhood of top results get an additive boost with hop decay. Uses BFS 2-hop traversal of the import graph. Files 1 hop away get a larger boost than files 2 hops away.

### Stage 4: Identifier Stem Matching

Query keywords are matched against file path components (stem and immediate parent directory) via prefix matching. If at least 10% of query keywords match path components, a boost is applied:

```
boost = max_score * match_ratio * 1.5
```

### Stage 5: Noise Penalties

Certain file categories receive multiplicative score penalties. Penalties compound when multiple conditions apply.

| Category | Multiplier |
|---|---|
| Test files | 0.3x |
| Compat / legacy directories | 0.3x |
| Examples / docs directories | 0.3x |
| Re-export barrels (`__init__.py`, `package-info.java`) | 0.5x |
| TypeScript declaration stubs (`.d.ts`) | 0.7x |

A file matching both "test" and "compat" receives a combined 0.09x multiplier.

### Stage 6: File Saturation Decay

To prevent a single file from dominating results, chunks beyond the first from the same file are penalized during greedy selection:

```
penalty = 0.5^(n - 1)
```

The 2nd chunk from a file scores at 0.5x, the 3rd at 0.25x, the 4th at 0.125x.

## Symbol Index

The symbol index maps each symbol name to its definition location and reference count. Built at index time from tree-sitter AST queries. At query time, symbol queries bypass the full retrieval pipeline and go directly to the symbol index for definition lookup.

This dramatically improves precision for symbol queries. Symbol NDCG improved from 0.263 to 0.619 after the symbol index was added.

## Import Graph

The import graph captures file-level dependency edges extracted via tree-sitter AST queries across 10 language families. Edges are resolved by suffix matching with proximity-based disambiguation. Persisted as `imports.bin`.

The graph is used in two ways:
- **Proximity boost** (stage 3 above): files near top results get a score boost
- **`prx impact`**: reverse dependency analysis walks the graph backwards

## Budget Enforcement

After reranking, results are selected greedily in score order until the token budget is exhausted.

Token counting: chunk content length divided by 4 gives a conservative approximation. When `--budget` is active, the cl100k_base tokenizer provides exact counts.

Results that would exceed the remaining budget are skipped, not truncated. The budget is a hard ceiling on total tokens returned. Paginated retrieval is supported via continuation tokens.

## Index Storage

**In-memory by default:** the index is built on demand at query time. Fast enough for most repositories.

**Persistent index:** `prx index .` writes the index to `.prx/index/` for large repos or repeated queries. Files written:

- `chunks.bin` — chunk content and metadata
- `embeddings.bin` — dense vectors (memory-mapped at query time)
- `sparse.bin` — BM25 CSC sparse matrix
- `bloom.bin` — bloom filter for `prx exists`
- `symbols.bin` — symbol definition index
- `imports.bin` — import graph
- `meta.json` — version, timestamp, per-file content hashes

**Incremental re-indexing:** when a file changes, only that file's chunks are re-embedded and re-scored. The rest of the index is unchanged.

**Bloom filter:** O(1) existence checks before full index lookup. 2% false positive rate, ~75KB for 50K tokens. "No" from bloom means definitely absent. "Yes" means probably present (confirmed with literal search when `--exact` is passed).
