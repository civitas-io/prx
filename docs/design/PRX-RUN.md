# prx run — Structured Command Runner

## Problem Statement

AI coding agents run test suites, builds, and linters dozens of times per
session. The output of these tools is designed for humans scanning a terminal
— verbose progress lines, passing test confirmations, compilation steps. Agents
must consume the full output to find the 1-5 lines that matter (failures,
errors, warnings), paying the token cost for everything else.

Measured waste:

| Scenario | Raw tokens | Useful tokens | Waste |
|---|---|---|---|
| 164 tests, all pass | ~1,200 | ~12 | 99% |
| 164 tests, 2 fail | ~1,200 | ~80 | 93% |
| 500 tests, 1 fail | ~4,300 | ~100 | 97% |
| cargo build success | ~200 | ~5 | 97% |
| cargo clippy, 3 warnings | ~300 | ~90 | 70% |
| npm install | ~2,000+ | ~5 | 99% |

In a typical test-debug-fix loop (10 iterations, 164 tests), agents burn
~12,000 tokens on lines that say "ok". That's context window space not
available for reasoning or code generation.

The SWE-bench token study (arxiv 2604.22750) found that test/build output
accounts for ~8% of total agent tool calls and 15-25% of input tokens. The
context-os project measured 27-36% reduction in test output tokens with 100%
recall on error messages.

## Target Users

Same as ag: AI coding agents (Claude Code, Cursor, Codex, OpenCode, Aider,
SWE-agent) and developers building agent toolchains.

## Product Vision

`prx run` wraps any shell command and returns structured JSON with:
- Exit code and duration
- A one-line summary ("164 passed, 0 failed in 0.49s")
- Only the actionable items (failures, errors, warnings) — with file, line, message
- Token savings metric (how many tokens the agent avoided)

For known tools (cargo test, pytest, jest, go test, cargo clippy, cargo build,
tsc, eslint, ruff), prx run parses the output using tool-specific patterns. For
unknown commands, it falls back to exit code + last N lines.

## CLI Interface

```
prx run <command> [args...]
```

| Flag | Description |
|---|---|
| `--raw` | Bypass parsing, return full output as-is (structured JSON envelope) |
| `--full` | Return parsed summary AND full output |
| `--budget N` | Token budget for output (truncate full output if over budget) |
| `--timeout N` | Command timeout in seconds (default: 300) |

## Output Schema

### Success (all tests pass)

```json
{
  "version": "0.2.0",
  "command": "run",
  "status": "ok",
  "tokens": 15,
  "data": {
    "exit_code": 0,
    "duration_ms": 490,
    "tool": "cargo_test",
    "summary": "164 passed, 0 failed in 0.49s",
    "passed": 164,
    "failed": 0,
    "skipped": 0,
    "failures": [],
    "warnings": [],
    "output_lines": 168,
    "output_tokens_saved": 1185
  }
}
```

### Failure (tests fail)

```json
{
  "data": {
    "exit_code": 1,
    "duration_ms": 520,
    "tool": "cargo_test",
    "summary": "162 passed, 2 failed in 0.52s",
    "passed": 162,
    "failed": 2,
    "skipped": 0,
    "failures": [
      {
        "name": "search::tests::hybrid_search",
        "location": "src/commands/search.rs:45",
        "message": "assertion `left == right` failed\n  left: 0\n right: 1"
      },
      {
        "name": "walk::tests::respects_gitignore",
        "location": "src/walk.rs:149",
        "message": "assertion failed: !paths.contains(&\"ignored.txt\")"
      }
    ],
    "warnings": [],
    "output_lines": 172,
    "output_tokens_saved": 1100
  }
}
```

### Build/Lint errors

```json
{
  "data": {
    "exit_code": 1,
    "duration_ms": 1200,
    "tool": "cargo_clippy",
    "summary": "3 warnings, 1 error",
    "passed": 0,
    "failed": 1,
    "skipped": 0,
    "failures": [
      {
        "name": "error[E0382]",
        "location": "src/main.rs:30",
        "message": "borrow of partially moved value: `cli.command`"
      }
    ],
    "warnings": [
      {
        "name": "unused_variable",
        "location": "src/output.rs:14",
        "message": "unused variable `path`"
      }
    ],
    "output_lines": 45,
    "output_tokens_saved": 180
  }
}
```

### Unknown command (fallback)

```json
{
  "data": {
    "exit_code": 0,
    "duration_ms": 100,
    "tool": "unknown",
    "summary": "exited 0",
    "passed": 0,
    "failed": 0,
    "skipped": 0,
    "failures": [],
    "warnings": [],
    "tail": "last 10 lines of output here...",
    "output_lines": 50,
    "output_tokens_saved": 0
  }
}
```

## Supported Tools

### Tier 1 — Full Parsing (v0.1)

| Tool | Detection | What is extracted |
|---|---|---|
| `cargo test` | command starts with `cargo test` | pass/fail counts, failure names + locations + assertion messages |
| `cargo build` | command starts with `cargo build` | error codes + locations + messages |
| `cargo clippy` | command starts with `cargo clippy` | warnings + errors with codes, locations, messages |
| `pytest` | command contains `pytest` | pass/fail/skip counts, failure names + locations + assertion messages |
| `go test` | command starts with `go test` | ok/FAIL per package, failure names + messages |
| `jest` / `npm test` | command contains `jest`, `npm test`, or `npx jest` | pass/fail/skip counts, failure names + expect/received messages |
| `vitest` | command contains `vitest` or `npx vitest` | pass/fail counts, failure names + diff messages |
| `tsc` | command starts with `tsc` or `npx tsc` | error codes + file:line:col + messages |
| `eslint` | command contains `eslint` | warning/error counts per file + rule names |

### Tier 2 — Full Parsing (v0.2)

| Tool | Detection |
|---|---|
| `ruff` | command contains `ruff check` |
| `bun test` | command starts with `bun test` |
| `deno test` | command starts with `deno test` |
| `dotnet test` | command starts with `dotnet test` |

### Fallback

Any command not matching a known tool:
- Capture exit code
- Capture last 10 lines of combined stdout+stderr
- Report `tool: "unknown"`, `summary: "exited N"`

## Design Principles

1. **Never lose information on failure.** When a command fails, every error and
   warning must be in the output. Summarize passing, preserve failing.

2. **Zero configuration.** Tool detection is automatic from the command string.
   No config files, no flags to say "this is pytest".

3. **Fail-open.** If a parser can't handle the output, fall back to raw output
   rather than silently dropping information. Better too much than too little.

4. **Parsers are additive.** Each parser is a standalone module. Adding a new
   tool parser requires no changes to existing parsers or the runner framework.

## Success Metrics

| Metric | Target |
|---|---|
| Token reduction on all-pass test suites | >= 95% |
| Token reduction on failing test suites | >= 70% |
| Information loss on failures | 0% (every error/warning preserved) |
| Tool detection accuracy | 100% for Tier 1 tools |
| Latency overhead vs raw command | < 10ms |

## Non-Goals (v0.1)

- Interactive commands (stdin required)
- Streaming output (wait for completion, then parse)
- Custom parser plugins (add parsers to source code directly)
- Test re-running or test selection
- CI/CD integration beyond single command execution

## Token Savings Model

Conservative estimate per session (10 test-debug-fix iterations):

| Test suite size | Without prx run | With prx run | Savings |
|---|---|---|---|
| 50 tests | ~4,000 tokens | ~200 tokens | 95% |
| 164 tests | ~12,000 tokens | ~500 tokens | 96% |
| 500 tests | ~43,000 tokens | ~800 tokens | 98% |

These savings compound with every iteration. A 20-iteration debugging session
on a 500-test project saves ~84,000 tokens — equivalent to a small file's
worth of context window reclaimed for reasoning.
