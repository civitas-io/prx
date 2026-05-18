# prx run — System Design

## Architecture

```
prx run <command> [args...]
    |
    v
[Runner] — spawn subprocess, capture stdout+stderr, wait for exit
    |
    v
[Detector] — match command string against known tool patterns
    |
    v
[Parser] — tool-specific parser extracts structured data
    |
    v
[Output] — standard JSON envelope with summary + failures + warnings
```

## Module Structure

```
src/
├── commands/
│   └── run.rs          -- CLI args, orchestration
└── runner/
    ├── mod.rs           -- Runner trait, tool detection, dispatch
    ├── cargo_test.rs    -- cargo test parser
    ├── cargo_build.rs   -- cargo build/clippy parser
    ├── pytest.rs        -- pytest parser
    ├── go_test.rs       -- go test parser
    ├── jest.rs          -- jest/npm test/vitest parser
    ├── tsc.rs           -- TypeScript compiler parser
    ├── eslint.rs        -- eslint parser
    └── fallback.rs      -- unknown command fallback
```

## Runner Framework

### Command Execution

```
RunResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}
```

Spawn via `std::process::Command`. Capture both stdout and stderr. Enforce
`--timeout` via thread-based watchdog (kill child process if exceeded).
Default timeout: 300 seconds.

Combined output: merge stdout + stderr for parsing (most tools write to
both). Preserve order via line-by-line interleaving if possible, but accept
that exact interleaving may not be deterministic.

### Tool Detection

Match the command string (first argument + subcommand) against patterns:

| Pattern | Tool |
|---|---|
| `cargo test` | cargo_test |
| `cargo build` | cargo_build |
| `cargo check` | cargo_build |
| `cargo clippy` | cargo_clippy |
| `pytest` or `python -m pytest` | pytest |
| `go test` | go_test |
| `jest` or `npx jest` or `npm test` | jest |
| `vitest` or `npx vitest` | vitest |
| `tsc` or `npx tsc` | tsc |
| `eslint` or `npx eslint` | eslint |
| `ruff check` or `ruff` | ruff (v0.2) |
| `bun test` | bun_test (v0.2) |

Detection is prefix-based: check if the command starts with or contains the
pattern. First match wins.

### Parser Trait

Each parser implements:

```
ParsedOutput {
    tool: String,
    summary: String,
    passed: usize,
    failed: usize,
    skipped: usize,
    failures: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
}

Diagnostic {
    name: String,
    location: Option<String>,     // "file:line" or "file:line:column"
    message: String,
}
```

## Parser Specifications

### cargo test

**Summary line pattern:**
```
test result: ok. N passed; M failed; I ignored; J measured; K filtered out; finished in Xs
```

Regex: `test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored`

**Failure pattern:**
```
---- module::test_name stdout ----
thread 'module::test_name' panicked at src/file.rs:LINE:COL:
ASSERTION_MESSAGE
note: run with `RUST_BACKTRACE=1` ...
```

Extract:
- Name: text between `---- ` and ` stdout ----`
- Location: text after `panicked at ` up to the next `:`
- Message: lines between the panic line and `note:` or next `----`

**Passing line (skip):**
```
test module::test_name ... ok
```

These lines are the primary waste. The parser discards them entirely.

### cargo build / cargo clippy

**Error pattern:**
```
error[E0382]: borrow of partially moved value: `cli.command`
  --> src/main.rs:30:37
   |
30 |         Ok(data) => write_envelope(&cli.command.name(), data, cli.plain),
   |                                     ^^^^^^^^^^^ value borrowed here after partial move
```

Extract:
- Name: `error[CODE]` or `warning[CODE]`
- Location: line starting with `  --> `
- Message: the error description after the code

**Warning pattern:** Same as error but starts with `warning` instead of `error`.

**Summary line:**
```
warning: `prx` (bin "prx") generated N warning(s)
error: could not compile `prx` (bin "prx") due to N previous error(s)
```

Or on success:
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in Xs
```

### pytest

**Summary line pattern:**
```
====== N passed, M failed, K skipped in Xs ======
```
or
```
====== N passed in Xs ======
```

Regex: `=+ (.+) =+$`

**Failure pattern:**
```
FAILED tests/test_auth.py::test_login - AssertionError: assert 0 == 1
```

Or the verbose form:
```
_______ test_login _______

    def test_login():
>       assert authenticate("user") == True
E       AssertionError: assert False == True

tests/test_auth.py:42: AssertionError
```

Extract:
- Name: test function name
- Location: file:line from the last line of the block
- Message: the `E` lines (assertion details)

### go test

**Package result pattern:**
```
ok      github.com/user/project/pkg     0.003s
FAIL    github.com/user/project/auth    0.005s
```

**Individual failure:**
```
--- FAIL: TestLogin (0.00s)
    auth_test.go:42: expected true, got false
```

Extract:
- Name: text after `--- FAIL: `
- Location: file:line from indented lines
- Message: the assertion text

### jest / npm test

**Summary line pattern:**
```
Tests:       2 failed, 48 passed, 50 total
```

Regex: `^Tests:\s+(?:(\d+) failed, )?(\d+) passed, (\d+) total`

Also look for:
```
Test Suites: 1 failed, 5 passed, 6 total
Time:        3.45 s
```

**Failure pattern:**
```
  ● Auth > should authenticate user

    expect(received).toBe(expected)

    Expected: true
    Received: false

      42 |   const result = authenticate(user);
      43 |   expect(result).toBe(true);
         |                  ^
      44 | });

      at Object.<anonymous> (tests/auth.test.ts:43:18)
```

Extract:
- Name: text after `●` (trimmed)
- Location: `at` line — file:line:col in parentheses
- Message: the expect/received block

**Passing line (skip):**
```
  ✓ should return 200 (3 ms)
  ✓ should validate input (1 ms)
```

### vitest

Vitest output follows the same patterns as jest (it's API-compatible) with
minor differences:

**Summary line pattern:**
```
 Test Files  1 failed | 5 passed (6)
      Tests  2 failed | 48 passed (50)
```

Regex: `^\s+Tests\s+(?:(\d+) failed \| )?(\d+) passed \((\d+)\)`

**Failure pattern:** Same as jest (expect/received format).

### tsc (TypeScript compiler)

**Error pattern:**
```
src/auth.ts(42,18): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.
```

Regex: `^(.+)\((\d+),(\d+)\): (error|warning) (TS\d+): (.+)$`

Extract:
- Name: error code (e.g., `TS2345`)
- Location: file(line,col) -> `file:line:col`
- Message: text after the code

**Summary:** tsc has no summary line. Count errors from individual lines.
Exit code 0 = no errors, non-zero = errors found.

### eslint

**Default formatter output:**
```
/Users/user/project/src/auth.ts
  42:18  error  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any
  55:1   warning  Missing return type on function          @typescript-eslint/explicit-function-return-type

✖ 2 problems (1 error, 1 warning)
```

Regex for individual lines: `^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s{2,}(\S+)$`

Regex for summary: `^✖ (\d+) problems? \((\d+) errors?, (\d+) warnings?\)`

Extract:
- Name: rule name (last column)
- Location: file from header line + line:col from detail
- Message: middle text

## Fallback Parser

For unrecognized commands:
- `tool`: `"unknown"`
- `summary`: `"exited N"` where N is exit code
- `failures`: empty (we can't parse what we don't know)
- `tail`: last 10 lines of combined output (for the agent to inspect if needed)

If exit code is non-zero, include last 20 lines instead of 10.

## Token Savings Calculation

```
output_tokens_raw = (stdout.len() + stderr.len()) / 4
output_tokens_parsed = serialized_json_response.len() / 4
output_tokens_saved = output_tokens_raw - output_tokens_parsed
```

Reported in every response so agents and users can see the savings.

## Error Handling

- Command not found: return error envelope with code `command_not_found`
- Command timeout: kill process, return with `exit_code: -1` and
  `summary: "timed out after Ns"`
- Parser failure: fall back to fallback parser (fail-open)

## Testing Strategy

### Unit Tests

Each parser module has tests with captured real output:

- `cargo_test_all_pass`: paste real cargo test output, verify parsed counts
- `cargo_test_with_failures`: paste output with panics, verify failure extraction
- `cargo_clippy_warnings`: paste clippy output, verify warning extraction
- `pytest_all_pass`: paste pytest output, verify parsed counts
- `pytest_with_failures`: paste pytest failure output, verify extraction
- `go_test_pass`: paste go test output, verify
- `go_test_fail`: paste go test failure, verify
- `fallback_unknown`: arbitrary output, verify last-N-lines behavior

### Integration Tests

- Run `prx run cargo test` on our own test suite, verify JSON output
- Run `prx run cargo build` on our own project, verify
- Run `prx run echo hello`, verify fallback works

## Implementation Order

1. Runner framework (subprocess, timeout, capture)
2. Tool detection
3. Fallback parser
4. cargo_test parser
5. cargo_build/clippy parser
6. pytest parser
7. go_test parser
8. jest/vitest parser (shared — vitest uses jest-compatible output)
9. tsc parser
10. eslint parser
11. Wire into CLI dispatch
12. Integration tests
