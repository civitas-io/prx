# Search Subsystem Architecture

Hybrid retrieval engine combining literal, semantic, and structural search with a 5-stage reranking pipeline.

---

## Chunking Pipeline

Source files are split into chunks before indexing. Chunking is syntax-aware via tree-sitter, targeting **1500 characters per chunk**.

**Algorithm:**

1. Parse the file into an AST using the appropriate tree-sitter grammar.
2. Recursively traverse the tree, collecting leaf and intermediate nodes.
3. Merge adjacent sibling nodes greedily until the accumulated character count approaches the target length.
4. When a single node exceeds the target, recurse into its children and apply the same merge logic.
5. Emit each accumulated group as a chunk.

Chunks do not overlap. A character belongs to exactly one chunk.

**Fallback:** Files in unsupported languages fall back to line-based chunking, splitting on newline boundaries with the same character budget.

**Supported languages:** 15 languages via tree-sitter grammars compiled directly into the binary. No runtime grammar loading. Additional grammars can be added as crate dependencies. Import extraction uses tree-sitter AST queries for 10 language families.

---

## Embedding Model (Model2Vec)

Model: **potion-code-16M** (MinishLab). Embedded in the binary via `include_bytes!`. No network access, no filesystem reads at runtime.

**Architecture: static embeddings.** This is not a neural network in the transformer sense. There is no forward pass, no attention mechanism, no matrix multiplication through hidden layers.

**Dimensions:** 256 (float16), approximately 32MB total model size.

**Inference pipeline:**

1. Tokenize the input string against a fixed vocabulary.
2. Look up each token in a 62,500 x 256 embedding table.
3. Mean-pool the resulting vectors into a single 256-dimensional vector.
4. L2-normalize the pooled vector.

That's the entire inference path. Because it's a table lookup followed by averaging, it runs on CPU only and is roughly **500x faster** than transformer-based embedding models. There's no GPU requirement and no warm-up cost.

**Vocabulary:** 62,500 tokens, trained on a code corpus covering Python, Java, JavaScript, Go, PHP, and Ruby.

---

## BM25 Implementation

BM25 operates on tokenized text. Code identifiers require special handling because standard word tokenization destroys their semantics.

**Compound identifier tokenization:**

Identifiers are extracted from chunk content via regex, then split on camelCase and snake_case boundaries. Both the original compound form and each sub-token are preserved.

Example: `getHTTPResponse` produces `["gethttpresponse", "get", "http", "response"]`.

No stemming is applied. This is intentional: code identifiers are semantically distinct, and stemming conflates terms that should remain separate (e.g., `initialize` vs `initial`).

**Content enrichment:**

Before BM25 indexing, each chunk's text is augmented with:
- The file stem, repeated twice (to increase its term frequency weight).
- The last 3 directory components of the file path.

This makes file-name and directory-name terms retrievable via BM25 without separate metadata queries.

**Scoring:**

BM25 scores are pre-computed and stored in a CSC sparse matrix. At query time, scoring is a slice-and-sum operation: extract the column(s) for query terms, sum the values. No per-query document traversal.

---

## Reciprocal Rank Fusion

Semantic and BM25 results are merged via Reciprocal Rank Fusion (RRF) before reranking.

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

**Symbol detection:** A regex heuristic identifies symbol-like queries by matching patterns such as `Foo::bar`, `_private`, `getUserById`, and similar forms.

**Over-fetching:** Both retrievers fetch `top_k * 5` candidates before fusion. The expanded candidate pool is then reranked and trimmed to `top_k`.

---

## Reranking Pipeline

After RRF fusion, results pass through a deterministic reranking pipeline. Stages apply in order.

**1. File coherence boost**

Files where multiple chunks scored highly get their top chunk boosted. The boost is proportional to the file's aggregate score relative to the highest-scoring file across all results:

```
boost = max_score * 0.2 * (file_aggregate / max_file_aggregate)
```

**2. Definition boost**

Chunks that define a queried symbol receive a 3x score multiplier. Detection uses a keyword list: `class`, `def`, `fn`, `func`, `struct`, `enum`, `trait`, `interface`, and equivalents across languages. If the file stem also matches the symbol name, an additional 1.5x multiplier applies.

**3. Import graph proximity**

Files in the dependency neighborhood of top results get a 0.25x additive boost with hop decay. Uses `ranking/proximity.rs` with BFS 2-hop traversal of the import graph.

**4. Identifier stem matching**

Query keywords are matched against file path components (stem and immediate parent directory) via prefix matching. If at least 10% of query keywords match path components, a boost is applied:

```
boost = max_score * match_ratio
```

**5. Noise penalties**

Certain file categories receive multiplicative score penalties. Penalties compound when multiple conditions apply.

| Category | Multiplier |
|---|---|
| Test files | 0.3x |
| Compat / legacy directories | 0.3x |
| Examples / docs directories | 0.3x |
| Re-export barrels (`__init__.py`, `package-info.java`) | 0.5x |
| TypeScript declaration stubs (`.d.ts`) | 0.7x |

A file matching both "test" and "compat" receives a combined 0.09x multiplier.

**6. File saturation decay**

To prevent a single file from dominating results, chunks beyond the first from the same file are penalized during greedy selection:

```
penalty = 0.5^(n - 1)
```

Where `n` is the chunk's position within its file in the result set. The 2nd chunk scores at 0.5x, the 3rd at 0.25x, the 4th at 0.125x, and so on.

---

## Budget Enforcement

After reranking, results are selected greedily in score order until the token budget is exhausted.

**Token counting:** Chunk content length divided by 4 gives a conservative token approximation. When precision is required (e.g., for exact context window management), the `tokenizers` crate provides exact counts.

Results that would exceed the remaining budget are skipped, not truncated. The budget is a hard ceiling on total tokens returned.

Paginated retrieval is supported via continuation tokens, allowing callers to fetch subsequent result pages without re-running the full pipeline.

---

## Three Search Modes

**`--literal` (default for short patterns)**

Regex matching at ripgrep speed. No embeddings are loaded, no index is consulted. Suitable for exact string or pattern searches.

**`--semantic` (default for natural language)**

Full hybrid pipeline: chunk retrieval via BM25 and semantic embeddings, RRF fusion, reranking, budget enforcement. Suitable for concept-level and natural language queries.

**`--structural`**

AST pattern matching via ast-grep. Queries use metavariable syntax (e.g., `fn $NAME($$$) { $$$ }`). Returns structurally matched AST nodes rather than scored chunks.

**Auto-detection:**

When no mode flag is provided, the query is classified automatically:

- Fewer than 3 tokens, or contains regex metacharacters: `--literal`
- Contains `$VAR`-style metavariables: `--structural`
- Otherwise (natural language words, multi-token phrases): `--semantic`
