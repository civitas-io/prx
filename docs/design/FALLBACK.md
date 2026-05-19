# Graceful Fallback System

## Problem Statement

prx is a young tool. It will have bugs. When a prx command fails — crash,
panic, parse error, unexpected input — the agent's workflow should not break.
Today, a prx failure returns an error JSON and the agent must decide what to
do. Most agents will retry or give up, wasting tokens either way.

The fix: when prx fails, silently fall back to the equivalent Unix command
(grep, cat, find, etc.), return the result in the same JSON envelope, and log
the failure for debugging. The agent never knows prx failed. The user gets
real error data to fix prx.

## Goals

1. **Zero agent disruption** — a prx failure produces the same shaped output
   as a prx success. The agent sees results, not errors.
2. **Error capture** — every fallback logs the error, the command that failed,
   the fallback command used, and a timestamp to `~/.prx/errors.jsonl`.
3. **Real-world baseline data** — fallback results are raw Unix tool output,
   which gives us actual baseline token counts for free. Log both the fallback
   bytes (what grep/cat returned) and what prx would have returned (0, since
   it failed) to stats.
4. **Transparency** — the JSON envelope includes a `"fallback": true` field
   when fallback was used, so the agent CAN detect it if it wants to.

## Non-Goals

- Retry logic (if prx failed once, don't retry — fall back immediately)
- Fallback for `prx edit --apply` (edits are destructive — never fall back
  to sed on a write operation)
- Fallback for `prx mcp`, `prx init`, `prx stats`, `prx bench` (no Unix
  equivalent, and failure is acceptable for non-critical commands)

## Fallback Mapping

| prx command | Fallback command | What it returns |
|---|---|---|
| `prx search "pattern" path/` | `grep -rn "pattern" path/` | Raw grep output as `data.raw` |
| `prx read file.rs` | `cat file.rs` | Raw file content as `data.raw` |
| `prx read file.rs --skeleton` | `cat file.rs` | Full file (no skeleton — fallback can't parse) |
| `prx read file.rs --lines 10-20` | `sed -n '10,20p' file.rs` | Line range |
| `prx find path/` | `find path/ -type f` | File list |
| `prx find path/ --pattern "*.rs"` | `find path/ -name "*.rs" -type f` | Filtered file list |
| `prx exists "pattern" path/` | `grep -rl "pattern" path/` | File list (non-empty = exists) |
| `prx outline file.rs` | `grep -n "fn \|struct \|impl \|enum \|trait " file.rs` | Rough symbol grep |
| `prx diff` | `git diff` | Raw git diff output |
| `prx run <cmd>` | `<cmd>` | Raw command output |
| `prx edit` | No fallback | Error returned as-is (destructive) |
| `prx mcp` | No fallback | Error returned as-is |
| `prx init` | No fallback | Error returned as-is |
| `prx stats` | No fallback | Error returned as-is |
| `prx bench` | No fallback | Error returned as-is |
| `prx batch` | No fallback | Per-command fallback within batch |

## Fallback Output Format

When fallback is used, the envelope looks like:

```json
{
  "version": "0.1.0",
  "command": "search",
  "status": "ok",
  "tokens": 1250,
  "fallback": true,
  "data": {
    "raw": "src/auth.rs:42:fn authenticate(...)\nsrc/auth.rs:55:...\n...",
    "source": "grep -rn \"pattern\" path/"
  }
}
```

Key: `status` is `"ok"` (not `"error"`), because the agent got results.
The `fallback: true` flag is informational.

## Error Log Format

Appended to `~/.prx/errors.jsonl`:

```json
{
  "ts": 1747500000,
  "command": "search",
  "args": ["search", "pattern", "src/"],
  "error": "thread panicked at src/search/fusion.rs:42",
  "fallback_cmd": "grep -rn pattern src/",
  "fallback_bytes": 4500
}
```

## Design

### Where Fallback Lives

In `main.rs`, wrapping the command dispatch. Not inside each command handler.

```
CLI parse -> try prx command -> success? -> output
                              -> error?  -> run fallback command
                                         -> log error
                                         -> output fallback result as "ok"
```

### Panic Handling

Use `std::panic::catch_unwind` around the command dispatch. This catches
panics (unwrap on None, index out of bounds, etc.) in addition to returned
errors.

### Commands That Skip Fallback

`edit` (with `--apply`), `mcp`, `init`, `stats`, `bench`, `batch`, `index`.
These either have no Unix equivalent or are not critical path for agents.

For these commands, errors are returned as-is (current behavior).

## Implementation Plan

1. Add `fallback` module at `src/fallback.rs`
   - `can_fallback(command: &str) -> bool`
   - `run_fallback(command: &str, args: &Commands) -> Option<serde_json::Value>`
   - `log_error(command: &str, args: &[String], error: &str, fallback_cmd: &str, fallback_bytes: usize)`

2. Update `main.rs` dispatch:
   - Wrap command execution in `catch_unwind`
   - On error/panic for fallback-eligible commands, call `run_fallback`
   - Log error via `log_error`
   - Output fallback result with `fallback: true` in envelope

3. Update `output.rs`:
   - `write_envelope` accepts optional `fallback: bool` parameter
   - When true, adds `"fallback": true` to the JSON envelope

4. Tests:
   - Unit test: `can_fallback` returns correct values per command
   - Unit test: `log_error` writes to errors.jsonl
   - E2E test: force a search error (invalid path), verify fallback grep runs

## Success Metrics

| Metric | Target |
|---|---|
| Agent workflow disruption from prx bugs | 0 (fallback catches all) |
| Error capture rate | 100% (every fallback logged) |
| Latency overhead for successful commands | 0 (fallback only runs on failure) |
| False fallbacks (prx succeeds but fallback runs) | 0 |
