# Token Savings

## Measured savings by feature

These numbers come from real agent sessions on production codebases. The benchmark methodology is in [Public Benchmark Suite](../performance/benchmarks.md).

| Feature | Scenario | Savings |
|---|---|---|
| `read --if-changed` (cache hit) | Re-reading an unchanged file | ~99% |
| `read --mode diff` | File with local changes | 98-99% |
| `read --mode diff` | Clean file (no changes vs HEAD) | ~99.9% |
| `read --mode entropy` | Generated code (50+ fields) | ~86% |
| `read --skeleton` | Full file reduced to signatures | ~90% |
| `read --mode aggressive` | Python with docstrings | 11-19% |
| `read --mode aggressive` | Clean Rust code | 1-7% |
| `run` | Passing test suites | 95-99% |
| `context` vs manual exploration | 4-5 calls collapsed to 1 | 60-80% |
| `search` | vs grep + follow-up reads | ~35% |

## Real-world telemetry

Measured across 200 calls in two agent sessions (a PR review and a coding task):

| Metric | Value |
|---|---|
| Total calls | 200 |
| Total tokens saved | 36,114 |
| Most-used command | `search` (56 calls, 28%) |
| Highest savings rate | `run` (52.9% average) |
| Highest absolute savings | `read` (46.3% average) |

### Per-command breakdown

**search (56 calls, 34.9% savings)**

Most-called command. The 34.9% figure understates real savings because the baseline doesn't account for the follow-up file reads agents do after `grep`. When you include the read-after-grep loop, real savings are likely 50-70%.

**read (24 calls, 46.3% savings)**

Biggest absolute savings. The key pattern: multiple re-reads of the same large file, each costing ~3,400 bytes through prx (skeleton/outline) vs ~21,430 bytes through `cat`. With `--if-changed` caching, re-reads cost ~50 bytes.

**run (13 calls, 52.9% savings)**

Test output parsing working as designed. 675 tokens vs 1,434 baseline.

**outline (5 calls, 27.9% savings)**

Moderate savings. The baseline (cat files to get symbols) is reasonable.

**find (23 calls)**

Savings are understated because prx find returns structured JSON with metadata (lines, language, symbols) that `find`+`wc`+`file` would require multiple follow-up commands to produce.

**exists (14 calls)**

Bloom filter O(1) check vs `grep -rl` (full scan). Real savings are large for big codebases but hard to measure against a single-command baseline.

## Before and after examples

### read --if-changed

```bash
# Without prx: re-read the whole file every time
cat src/auth/handler.ts    # 6,531 tokens

# With prx: skip if unchanged
prx read src/auth/handler.ts --if-changed a3f9b2c1...
# Cache hit: 57 tokens (99.1% savings)
# Cache miss: 6,531 tokens (full content returned normally)
```

### run

```bash
# Without prx: full test output
cargo test
# running 164 tests
# test test_one ... ok
# test test_two ... ok
# [... 162 more lines ...]
# test result: ok. 164 passed; 0 failed
# ~1,200 tokens

# With prx: only the signal
prx run cargo test
# {"passed": 164, "failed": 0, "duration_ms": 490, "failures": []}
# ~15 tokens (98.7% savings)
```

### read --skeleton

```bash
# Without prx: full file
cat src/auth/handler.ts    # 6,531 tokens

# With prx: signatures only
prx read src/auth/handler.ts --skeleton    # ~650 tokens (~90% savings)
```

### read --mode diff

```bash
# Without prx: full file to see what changed
cat src/auth/handler.ts    # 6,603 tokens

# With prx: only changed lines
prx read src/auth/handler.ts --mode diff    # 89 tokens (98.7% savings)
```

## How to measure your own savings

Run the token-savings dashboard against your own sessions:

```bash
prx stats                  # total savings across all recorded calls
prx stats --compare        # per-command breakdown
```

Run a synthetic benchmark comparing prx vs grep+cat on your codebase:

```bash
prx bench .
```

## Why re-reads matter most

The telemetry shows that multiple re-reads of the same unchanged file are common: 3-5 re-reads per file per session. Without `--if-changed`, each re-read costs the full file size. With it, re-reads cost ~50 bytes.

In a typical session with 5 re-reads of a 6,500-token file:
- Without caching: 32,500 tokens
- With `--if-changed`: ~6,550 tokens (first read + 4 cache hits)
- Savings: ~80%

The hash is in `meta.hash` in every read response. Store it and pass it back.
