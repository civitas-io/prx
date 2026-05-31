# Fallback System

prx is a young tool. It will have bugs. When a prx command fails — crash, panic, parse error, unexpected input — the agent's workflow shouldn't break.

The fallback system catches internal prx failures, runs the equivalent Unix command, and returns results in the same JSON envelope. The agent sees results, not errors. The failure is logged for debugging.

## How It Works

```
CLI parse → try prx command → success? → output
                             → error?  → run fallback command
                                       → log error to ~/.prx/errors.jsonl
                                       → output fallback result as "ok"
```

`std::panic::catch_unwind` wraps the command dispatch. This catches panics (unwrap on None, index out of bounds) in addition to returned errors.

## Fallback Output Format

When fallback is used, the envelope looks like:

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 1250,
  "fallback": true,
  "data": {
    "raw": "src/auth.rs:42:fn authenticate(...)\nsrc/auth.rs:55:...\n",
    "source": "grep -rn \"pattern\" path/"
  }
}
```

`status` is `"ok"` because the agent got results. The `fallback: true` field is informational — the agent can detect it if it wants to, but doesn't need to.

## Fallback Mapping

| prx command | Fallback command | What it returns |
|---|---|---|
| `prx search "pattern" path/` | `grep -rn "pattern" path/` | Raw grep output as `data.raw` |
| `prx read file.rs` | `cat file.rs` | Raw file content as `data.raw` |
| `prx read file.rs --lines 10-20` | `sed -n '10,20p' file.rs` | Line range |
| `prx find path/` | `find path/ -type f` | File list |
| `prx find path/ --pattern "*.rs"` | `find path/ -name "*.rs" -type f` | Filtered file list |
| `prx exists "pattern" path/` | `grep -rl "pattern" path/` | File list (non-empty = exists) |
| `prx outline file.rs` | `grep -n "fn \|struct \|impl \|enum \|trait " file.rs` | Rough symbol grep |
| `prx diff` | `git diff` | Raw git diff output |
| `prx run <cmd>` | `<cmd>` | Raw command output |

## Commands Without Fallback

Some commands have no Unix equivalent, or are destructive enough that falling back silently would be wrong.

| Command | Reason |
|---|---|
| `prx edit --apply` | Destructive. Never fall back to sed on a write operation. |
| `prx mcp` | No Unix equivalent. |
| `prx init` | No Unix equivalent. |
| `prx stats` | No Unix equivalent. |
| `prx bench` | No Unix equivalent. |
| `prx index` | No Unix equivalent. |
| `prx batch` | Per-command fallback within batch (each command falls back independently). |

For these commands, errors are returned as-is in the standard error envelope.

## Error Logging

Every fallback appends a record to `~/.prx/errors.jsonl`:

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

This log is the primary debugging tool for prx failures. `prx stats` can show fallback rates. The log file grows unboundedly — clear it manually if needed.

## Implementation

The fallback module lives at `src/fallback.rs`. It exposes three functions:

- `can_fallback(command: &str) -> bool` — returns true for commands with Unix equivalents
- `run_fallback(command: &str, args: &Commands) -> Option<serde_json::Value>` — runs the fallback and returns the result
- `log_error(...)` — appends to `~/.prx/errors.jsonl`

The fallback is invoked from `main.rs`, not from inside command handlers. This means the fallback catches any failure in the command, including failures in shared infrastructure (chunking, embedding, ranking).

## Design Goals

The fallback system has four goals:

1. **Zero agent disruption** — a prx failure produces the same shaped output as a prx success.
2. **Error capture** — every fallback logs the error, the command that failed, the fallback command used, and a timestamp.
3. **Real-world baseline data** — fallback results are raw Unix tool output, which gives actual baseline token counts. Both the fallback bytes and what prx would have returned (0, since it failed) are logged.
4. **Transparency** — the JSON envelope includes `"fallback": true` so the agent can detect it if it wants to.
