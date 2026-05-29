# v0.6.0 — Model Tiering Design

## Problem

The embedded 32M potion-retrieval model is a general-purpose retrieval
model. Benchmark data from the v0.5.7 public benchmark suite (200 queries
across 8 repos) shows clear degradation by codebase size:

| Size Tier | Repos | Avg NDCG@10 | Symbol | Semantic |
|---|---|---|---|---|
| Small (<3K files) | flask, ripgrep, fastify | 0.545 | 0.812 | 0.446 |
| Medium (3-10K files) | cargo, kafka, django | 0.332 | 0.711 | 0.229 |
| Large (10K+ files) | terraform, vscode | 0.248 | 0.439 | 0.200 |

Symbol search remains strong across all sizes (relies on BM25 + symbol
index, not embeddings). The bottleneck is semantic search — the 32M model
doesn't have enough capacity to distinguish relevant code in large
embedding spaces.

## Solution: Model2Vec Distillation of Code-Specific Models

Model2Vec distillation converts any sentence-transformer model into a
static embedding lookup table. The distilled model uses the same inference
path as the current model (tokenize → lookup → mean pool → normalize) —
no architecture changes needed in prx.

### Distillation Process

```python
from model2vec.distill import distill

model = distill(
    model_name="codesage/codesage-base-v2",  # 356M teacher
    pca_dims=256,           # Match current dim
    quantize_to="float16",  # Match current quantization
    pooling="mean"          # Match current inference
)
model.save_pretrained("codesage-m2v-256")
```

Time: ~30 seconds on CPU. No training data needed.

### Candidate Models for Distillation

| Teacher | Params | Code2Code NDCG | Distilled Size | Notes |
|---|---|---|---|---|
| all-mpnet-base-v2 | 109M | 0.739 (CodeSearchNet) | ~8 MB | Best general teacher per CodeMalt |
| CodeSage-v2-Base | 356M | 47.17 (Code2Code) | ~8 MB | Code-specific |
| CodeSage-v2-Large | 1.3B | 51.55 (Code2Code) | ~15 MB | Higher quality |
| Jina Code v3 | 570M | ~65 (estimated) | ~30 MB | Latest, task instructions |
| SFR-Embedding-Code-2B | 2B | 67.4 (CoIR) | ~25 MB | SOTA but large |

### Quality Retention

Model2Vec distillation typically retains 81-93% of the teacher model's
performance. For code, retention is expected to be on the higher end
because code has strong syntactic structure that mean pooling captures well.

## Architecture

### Model Storage

```
~/.prx/models/
├── manifest.json            # Available models, SHA-256 hashes
├── codesage-m2v-256.safetensors  # Standard tier (~8 MB)
└── jina-code-m2v-512.safetensors # Large tier (~30 MB)
```

The built-in model stays embedded in the binary via `include_bytes!`.
Downloaded models are stored in `~/.prx/models/` and referenced by the
index metadata.

### Index Metadata

```json
{
  "version": "0.6.0",
  "model": "codesage-m2v-256",
  "embeddings_dim": 256,
  ...
}
```

When loading an index, prx checks the model field. If it doesn't match
an available model, it warns and falls back to the built-in model (with
a quality caveat).

### CLI Interface

```bash
# Default: use built-in model (current behavior)
prx index .

# Use a specific model tier
prx index . --model standard    # Downloads on first use
prx index . --model large

# List available models
prx model list

# Download a model explicitly
prx model download standard
```

### Recommendation Engine

After `prx index`, if the codebase exceeds a size threshold and the
built-in model was used:

```json
{
  "status": "ok",
  "data": {
    "files_indexed": 7231,
    "chunks": 63740,
    "hints": [
      "This codebase has 7,231 files. For better semantic search on medium/large repos, try: prx index --model standard"
    ]
  }
}
```

Thresholds (based on v0.5.7 benchmark data):
- <3,000 files: no recommendation (built-in is sufficient)
- 3,000-10,000 files: recommend `standard`
- >10,000 files: recommend `large`

## Implementation Plan

### Phase 1: Distill and Evaluate

1. Distill 3 candidate models (all-mpnet, CodeSage-Base, CodeSage-Large)
2. Run 200-query benchmark suite against each
3. Pick the best for each tier
4. Publish models to GitHub Releases with SHA-256 hashes

### Phase 2: Download Infrastructure

1. Add `~/.prx/models/` directory support
2. Implement SHA-256 verified download from GitHub Releases
3. Add `--model` flag to `prx index`
4. Support `PRX_MODELS_DIR` env var for offline/CI use

### Phase 3: Recommendation and Polish

1. Add size-based recommendation hints to `prx index` output
2. Add `prx model list` and `prx model download` commands
3. Update AGENTS.md and skills guide with model guidance

## What This Does NOT Change

- Binary size stays ~49 MB (built-in model unchanged)
- Default behavior unchanged (no download on first use)
- Pure Rust inference (Model2Vec format, same code path)
- Index file format (just adds a `model` field to meta.json)
- Existing indexes continue to work (assumed built-in model)
