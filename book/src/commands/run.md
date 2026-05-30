# run

Parses test, build, and lint output into structured JSON. Only failures and summaries are returned. Passing tests are omitted.

## Usage

```bash
prx run [options] <command> [args...]
```

## Options

| Flag | Description |
|---|---|
| `--raw` | Bypass parsing, return full output in JSON envelope |
| `--full` | Return parsed summary AND full output |
| `--budget N` | Token budget for output |
| `--timeout N` | Command timeout in seconds (default: 300) |
| `--plain` | Human-readable output |

## Examples

```bash
prx run cargo test
prx run cargo clippy
prx run pytest
prx run npm test
prx run go test ./...
prx run tsc --noEmit
prx run eslint src/
```

## Token savings

A 164-test suite that outputs ~1,200 tokens raw becomes ~15 tokens through prx. A 304-test suite:

| Method | Tokens |
|---|---|
| Raw `cargo test` output | ~6,000 |
| `prx run cargo test` | ~120 |
| Savings | 98% |

In a 10-iteration test-debug-fix loop on a 500-test project, prx run saves ~84,000 tokens compared to reading raw output.

## Output format

### All tests pass

```json
{
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

### Tests fail

```json
{
  "data": {
    "exit_code": 1,
    "tool": "cargo_test",
    "summary": "162 passed, 2 failed in 0.52s",
    "passed": 162,
    "failed": 2,
    "failures": [
      {
        "name": "search::tests::hybrid_search",
        "location": "src/commands/search.rs:45",
        "message": "assertion `left == right` failed\n  left: 0\n right: 1"
      }
    ]
  }
}
```

### Build/lint errors

```json
{
  "data": {
    "exit_code": 1,
    "tool": "cargo_clippy",
    "summary": "3 warnings, 1 error",
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
    ]
  }
}
```

## Supported tools

### Full parsing

| Tool | What prx extracts |
|---|---|
| `cargo test` | Pass/fail counts, failure names, locations, assertion messages |
| `cargo build` | Error codes, locations, messages |
| `cargo clippy` | Warnings and errors with codes, locations, messages |
| `pytest` | Pass/fail/skip counts, failure names, locations, tracebacks |
| `go test` | ok/FAIL per package, failure names and messages |
| `jest` / `npm test` | Pass/fail/skip counts, failure names, expect/received messages |
| `vitest` | Pass/fail counts, failure names, diff messages |
| `tsc` | Error codes, file:line:col, messages |
| `eslint` | Warning/error counts per file, rule names |
| `ruff` | Lint errors with file:line |
| `bun test` | Pass/fail counts, failure details |
| `deno test` | Pass/fail counts, failure details |
| `dotnet test` | Pass/fail counts, failure details |

### Fallback

Any command not matching a known tool: exit code, last 10 lines of combined stdout+stderr, `tool: "unknown"`.

## Design principles

**Never lose information on failure.** When a command fails, every error and warning is in the output. Passing tests are summarized; failing tests are preserved in full.

**Zero configuration.** Tool detection is automatic from the command string. No config files, no flags to say "this is pytest."

**Fail-open.** If a parser can't handle the output, it falls back to raw output rather than silently dropping information.

## Tips

- Use `prx run` for every test/build/lint invocation in an agent loop. The savings compound across iterations.
- The `output_tokens_saved` field in the response tells you exactly how many tokens were saved on that call.
- Use `--raw` if you need the full output for debugging a parser issue.
- Use `--timeout` for commands that might hang (e.g. integration tests with network calls).

See also: [diff](diff.md), [stats](other.md)
