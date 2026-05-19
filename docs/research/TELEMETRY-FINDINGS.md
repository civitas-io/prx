# Telemetry Findings — First Real-World Usage

Date: 2026-05-19
Data: 200 calls across 2 agent sessions (PR review + coding tasks)

## Summary

| Metric | Value |
|---|---|
| Total calls | 200 |
| Total tokens saved | 36,114 |
| Most-used command | search (56 calls, 28%) |
| Highest savings | run (52.9%), read (46.3%) |
| Zero real errors | 12 logged errors were all from E2E test suite |

## Per-Command Analysis

### search (56 calls, 34.9% savings)

Most-called command. Savings are understated because the baseline estimate
(matches * 200 bytes) doesn't account for the follow-up file reads agents
do after grep. Real savings are likely 50-70% when including the read-after-grep
loop.

Action: Fix baseline to include estimated file read cost.

### read (24 calls, 46.3% savings)

Biggest absolute savings. Key observation: multiple re-reads of the same
21,430-byte file, each costing ~3,400B through prx (skeleton/outline) but
would cost 21,430B through cat.

With session caching, re-reads would cost ~50B. This is the single
highest-ROI feature for v0.2.0.

Action: Implement session cache (v0.2.0 priority).

### run (13 calls, 52.9% savings)

Test output parsing working as designed. 675 tokens vs 1,434 baseline.

### outline (5 calls, 27.9% savings)

Moderate savings. The baseline (cat files to get symbols) is reasonable.

### find (23 calls, 0% savings shown)

Zero savings shown because baseline is parity. But prx find returns
structured JSON with metadata (lines, language, symbols) that find+wc+file
would require multiple follow-up commands to produce.

Action: Improve baseline to include follow-up command cost.

### exists (14 calls, 0% savings shown)

Bloom filter O(1) check vs grep -rl (full scan). Real savings are massive
for large codebases but our baseline doesn't capture it.

Action: Baseline should measure grep -rl output size.

### diff (16 calls, 0% savings shown)

Semantic summaries vs raw git diff. Real savings depend on diff size.

Action: Baseline should measure full git diff output.

## Issues Found

### 1. Baseline estimation too conservative for search

```
search actual=765B baseline=600B  ← prx larger than estimated baseline
```

The 200 bytes/match estimate is too low. Real grep output per match is
~100B, but agents then read matched files (averaging ~5,000B each).
Search baseline should be: grep_output + sum(matched_file_sizes).

### 2. Find/exists/diff show 0% because baseline = parity

These commands provide structured output that replaces multiple Unix
commands. The baseline should reflect the full chain of commands an
agent would run, not just the single equivalent.

### 3. Test pollution in telemetry logs

E2E tests write to ~/.prx/errors.jsonl and stats.jsonl, mixing with
real-world data. Tests should use isolated paths.

### 4. Re-reads are the biggest waste

Multiple reads of the same unchanged file are common (3-5 re-reads per
file per session). Session caching would eliminate ~80% of read tokens.

## Recommendations

1. Fix baseline estimation (quick win, immediate)
2. Isolate test telemetry (quick win, immediate)
3. Session cache for reads (v0.2.0, highest ROI)
4. Track re-read rate as a metric in stats --compare
