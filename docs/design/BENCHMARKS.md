# Benchmarking Plan

This document defines how prx measures and reports its performance. Three
dimensions matter: retrieval quality (did we find the right code?), token
efficiency (how much did it cost?), and latency (how fast?). Each is measured
both synthetically and in real agent sessions.

---

## Dimensions

| Dimension | Metric | Target | Baseline |
|---|---|---|---|
| Retrieval quality | NDCG@10 | >= 0.85 | Semble: 0.854, ripgrep: 0.126 |
| Token efficiency | Tokens per query at 90% recall | <= 2,000 | ripgrep+read: ~45,000 |
| Latency (index) | Wall-clock, cold start | < 500ms (avg repo) | Semble: 263ms |
| Latency (query) | Wall-clock, warm cache, p50 | < 5ms | Semble: 1.5ms |
| Real-world savings | Token reduction in agent sessions | >= 60% | grep+read baseline |

---

## Phase 1: Retrieval Quality (NDCG)

### Dataset Construction

Build a query-and-relevance dataset following Semble's methodology:

- **Repositories**: 50-100 repos across 15+ languages, pinned by commit SHA in
  `benchmarks/repos.json`. Mix of sizes: small (1K-10K LOC), medium (10K-100K),
  large (100K-500K). Include repos from Semble's benchmark for direct
  comparison.

- **Queries**: 1,000+ queries in three categories:
  - **Semantic** (60%): natural language describing behavior. "How is
    authentication handled?", "retry logic for failed API calls"
  - **Symbol** (25%): named entity lookup. "getUserById", "Matcher::new",
    "parse_config"
  - **Architecture** (15%): structural questions. "What modules depend on the
    database layer?", "error handling strategy"

- **Ground truth generation**: Use Claude Sonnet as annotator (same approach as
  Semble). For each query, the model identifies relevant code locations with
  relevance scores (0-3 scale). Store as file path + line range + relevance
  score.

- **Human verification**: Spot-check 10% of annotations manually. Flag and
  re-annotate any query where human and model disagree on the top-1 result.

### NDCG Calculation

```
DCG@k  = sum(rel_i / log2(i + 1))  for i = 1..k
IDCG@k = DCG of perfect ranking (all relevant results first)
NDCG@k = DCG@k / IDCG@k
```

Report NDCG@5 and NDCG@10. Break down by:
- Query category (semantic, symbol, architecture)
- Language
- Repository size
- Search mode (hybrid, semantic-only, bm25-only, literal)

### Comparison Baselines

| Tool | How to run | What it measures |
|---|---|---|
| ripgrep | `rg --json` on query keywords | Literal text search quality |
| Semble | Python API, same queries | Hybrid search quality (our target) |
| BM25-only | prx with `--mode bm25` | Lexical retrieval without embeddings |
| Semantic-only | prx with `--mode semantic` | Embedding retrieval without BM25 |

### Correctness Gate

Before quality benchmarks, verify zero false negatives on literal search:

```
Query              ripgrep matches    prx --literal matches    Pass?
"fn search"        22                 22                      PASS
"impl.*Matcher"    43                 43                      PASS
```

ag's literal mode must return a superset of ripgrep's results. Any miss is a
bug, not a benchmark result.

---

## Phase 2: Token Efficiency

### Methodology

Model the agent's actual workflow, not just search results.

**Baseline workflow (ripgrep + read)**:
1. Split query into keywords (drop stopwords, words < 3 chars)
2. Run `rg --fixed-strings --ignore-case` for each keyword
3. Read matched files in full, ranked by keyword match count
4. Count tokens with tiktoken cl100k_base

**ag workflow**:
1. Run `prx search` with default settings (hybrid mode, top-k=5)
2. Count tokens in the JSON response

**ag skeleton workflow** (for read operations):
1. Run `prx read --skeleton` on target files
2. Run `prx read --snap function` on selected functions
3. Count total tokens consumed

### Metrics

Report token efficiency at fixed recall levels:

| Recall target | prx tokens | ripgrep+read tokens | Savings |
|---|---|---|---|
| 50% | ? | ? | ? |
| 75% | ? | ? | ? |
| 90% | ? | ? | ? |
| 95% | ? | ? | ? |

Also report:
- **Tokens per successful result**: total tokens / number of relevant results
- **Budget effectiveness**: at --budget N, what recall is achieved?
- **Skeleton savings**: tokens for `prx read --skeleton` vs `cat` on same files

### Token Counting

- **Tokenizer**: tiktoken cl100k_base (standard for GPT-4/Claude token counting)
- **What counts**: the full JSON response body (what the agent actually receives)
- **What does not count**: tool call overhead, system prompts, agent reasoning

---

## Phase 3: Latency

### Methodology

Use `hyperfine` for statistical rigor. Measure two scenarios:

**Cold start** (no index, first run):

```bash
hyperfine --warmup 0 --runs 10 \
  'prx search "authenticate" src/' \
  'rg --json "authenticate" src/'
```

**Warm cache** (index built, repeated queries):

```bash
prx index src/  # build index once
hyperfine --warmup 3 --runs 20 \
  'prx search "authenticate" src/' \
  'prx search --literal "authenticate" src/' \
  'rg --json "authenticate" src/'
```

### Metrics

Report for each subcommand:

| Metric | What | How |
|---|---|---|
| Index time | Time to build search index from scratch | Median of 10 runs |
| Query p50 | Median query latency, warm cache | Median of 20 runs |
| Query p95 | 95th percentile query latency | 95th percentile of 20 runs |
| Query p99 | Tail latency | 99th percentile of 100 runs |

Break down by:
- Repository size (1K, 10K, 100K, 500K LOC)
- Search mode (literal, semantic, structural, hybrid)
- Subcommand (search, read, find, outline, exists)

### Per-Subcommand Baselines

| prx command | Baseline tool | What to compare |
|---|---|---|
| `prx search --literal` | `rg --json` | Latency, should be within 2x |
| `prx search --semantic` | Semble CLI | Latency at same quality |
| `prx read` | `cat` + `wc -l` | Latency overhead of metadata |
| `prx read --skeleton` | No baseline (new capability) | Absolute latency |
| `prx find` | `fd --json` | Latency, should be within 2x |
| `prx exists` | `rg -q` (exit code only) | Should be faster (bloom filter) |
| `prx diff --stat-only` | `git diff --stat` | Latency, token count |

---

## Phase 4: Real-World Agent Validation

Synthetic benchmarks underestimate savings by 70-90% (Scribe SWE-bench study).
Real-world measurement is non-optional.

### A/B Test Design

**Setup**: Same agent, same tasks, different search tool.

| Variable | Control | Treatment |
|---|---|---|
| Agent | Claude Code (or OpenCode) | Same |
| Model | Claude Sonnet 4.6 | Same |
| Tasks | 20-30 real coding tasks | Same |
| Search tool | ripgrep + cat (default) | prx (all subcommands) |
| Environment | Docker container, identical | Docker container, identical |

**Task corpus** (3 sources, 20-30 tasks total):
1. **SWE-bench Verified subset**: 10 tasks from the 500-task dataset, stratified
   by repo and complexity
2. **Real-world bugs**: 5-10 issues from popular open-source projects
3. **Feature implementation**: 5-10 feature requests on real codebases

**Per-task metrics** (logged automatically):

| Metric | Definition |
|---|---|
| Total tokens (input) | Sum of all input tokens across all LLM calls |
| Total tokens (output) | Sum of all output tokens |
| Tool call count | Number of search/read/find/edit/diff calls |
| Re-read rate | % of file reads that are re-reads of already-loaded files |
| Task success | Did the agent produce a correct patch? |
| Wall-clock time | Total time from task start to patch submission |
| Cost (USD) | Estimated cost at list pricing |

**Statistical requirements**:
- Multiple runs per task (3-5) to capture variance
- Report median and standard deviation (agent behavior is stochastic)
- Paired comparison: Wilcoxon signed-rank test for significance

### Agent Session Logging

ag includes `prx stats` for tracking token savings across sessions. The stats
file at `~/.prx/stats.jsonl` records per-call metrics:

```json
{
  "ts": 1747500000,
  "command": "search",
  "mode": "hybrid",
  "query_tokens": 12,
  "result_tokens": 487,
  "file_chars_avoided": 45000,
  "results_returned": 5,
  "budget": 500,
  "latency_ms": 4.2
}
```

**Savings calculation** (conservative, same as Semble):

```
saved_chars = file_chars_avoided - result_chars
saved_tokens = saved_chars / 4
savings_pct = saved_chars / file_chars_avoided
```

Where `file_chars_avoided` is the total size of files containing returned
results (what the agent would have read via `cat` without ag).

---

## Phase 5: Continuous Benchmarking

### CI Integration

Run benchmarks on every release (not every commit — too expensive):

```yaml
# .github/workflows/benchmark.yml
on:
  release:
    types: [published]
  workflow_dispatch:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build release binary
        run: cargo build --release
      - name: Run NDCG benchmark
        run: cargo bench --bench search
      - name: Run latency benchmark
        run: hyperfine --export-json results.json ...
      - name: Compare to baseline
        run: python benchmarks/compare.py results.json baseline.json
      - name: Upload results
        uses: actions/upload-artifact@v4
```

### Regression Detection

Flag if any metric degrades beyond threshold:

| Metric | Regression threshold |
|---|---|
| NDCG@10 | Drop > 0.02 from baseline |
| Query p50 | Increase > 50% from baseline |
| Index time | Increase > 100% from baseline |
| Token efficiency at 90% recall | Increase > 20% from baseline |
| Binary size | Increase > 10% from baseline |

### Published Results

Results published in `benchmarks/results/` and referenced from README.md:

- `ndcg_by_language.json` — per-language NDCG breakdown
- `ndcg_by_category.json` — per-query-category breakdown
- `token_efficiency.json` — recall-vs-tokens curve
- `latency_by_size.json` — latency vs repository size
- `comparison.json` — prx vs ripgrep vs Semble on same dataset

### Reproducibility

All benchmarks must be reproducible:
- Repos pinned by commit SHA
- Queries and ground truth checked into repo
- Exact tool versions recorded
- Hardware specs documented
- Random seeds fixed where applicable
- Instructions: `make benchmark` runs the full suite
