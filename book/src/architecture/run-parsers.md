# Run Parsers

`prx run <command>` wraps CLI tools and returns structured JSON with only actionable information. A passing `cargo test` suite that produces 50,000 tokens of raw output becomes ~200 tokens through prx. On suites with failures, you get exactly the failures — nothing else.

## The Problem

Test runners, build tools, and infrastructure CLIs produce output designed for human eyes. A typical `cargo test` run on a medium-sized project outputs thousands of lines: test names, timing, progress dots, success messages. An agent running tests needs one thing: what failed and why.

The same applies to `kubectl describe`, `terraform plan`, `docker build`, and `npm list`. Each tool produces verbose output where the signal is buried in noise.

## Architecture

```
command string → detect_tool() → execute() → parse_output() → JSON envelope
                 ↓                            ↓
              tool name                  ParsedResult {
              (string match)               summary, passed, failed, skipped,
                                           failures: Vec<Diagnostic>,
                                           warnings: Vec<Diagnostic>,
                                           tail: Option<String>
                                         }
```

`detect_tool()` matches the command string to a parser name. `execute()` spawns the process and captures stdout and stderr. `parse_output()` dispatches to the tool-specific parser. The fallback parser handles unknown commands (truncated tail + exit code).

Detection order matters: more specific patterns must match first. `cargo llvm-cov` must match before `cargo test`, and `kubectl logs` before `kubectl`.

Run parsers operate on command output (text logs, compiler diagnostics), not source code. Tree-sitter is used elsewhere in prx for code parsing. The one future exception — enriching error locations with function context — is deferred.

## Parser Catalog

### Test Runners

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_test` | `cargo test` | pass/fail counts, failed test names and output | passing test lines | 95-99% |
| `pytest` | `pytest`, `python -m pytest` | pass/fail/skip counts, failed test names | passing test dots, collection output | 95-99% |
| `go_test` | `go test` | pass/fail counts, failed test output | passing `--- PASS` lines | 90-95% |
| `jest` | `jest`, `vitest`, `npm test` | pass/fail/skip counts, failed test output | passing test lines, transform output | 90-95% |
| `dotnet` | `dotnet test`, `dotnet build` | CS-prefixed errors/warnings, test failures | restore output, dependency noise | 75-85% |

### Build and Lint Tools

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_build` | `cargo build`, `cargo check`, `cargo clippy` | errors and warnings with file:line:col | help text, notes, duplicate messages | 80-90% |
| `mypy` | `mypy`, `python -m mypy` | `file:line: error:` lines, error count | notes without errors, success messages | 50% |
| `tsc` | `tsc`, `npx tsc` | TypeScript errors with file:line:col | help suggestions, project config noise | 70-80% |
| `eslint` | `eslint` | lint errors/warnings with file:line | passing file notifications, fix suggestions | 60-80% |
| `mvn` | `mvn`, `mvnw` | compilation errors, Surefire failures, build result | download spam, dependency resolution | 90% |
| `gradle` | `gradle`, `gradlew` | FAILED tasks, compile errors, test summary | daemon startup, download progress | 85% |

### Coverage Tools

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_llvm_cov` | `cargo llvm-cov` | coverage summary, low-coverage files | per-line coverage data | 90-95% |
| `pytest_cov` | `pytest --cov`, `coverage report` | total %, low-coverage files | per-line miss data, branch detail | 80-90% |
| `go_cover` | `go test -cover`, `go tool cover` | total %, per-package coverage | per-line annotations | 70-80% |
| `jest_cov` | `jest --coverage`, `c8`, `istanbul` | total %, uncovered files table | per-line detail, branch maps | 80-90% |

### Infrastructure and DevOps

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `terraform` | `terraform plan`, `terraform apply` | changed resources, plan summary | `(known after apply)`, unchanged attrs | 75-85% |
| `kubectl` | `kubectl describe`, `kubectl get` | warning events, non-Ready conditions | normal events, managed fields | 80-90% |
| `kubectl_logs` | `kubectl logs`, `docker logs` | ERROR/WARN/FATAL + context, deduped | INFO/DEBUG lines, repeated lines | 70-90% |
| `docker_build` | `docker build`, `docker buildx` | failed step + context, image info | layer cache, download progress | 80% |
| `npm_ls` | `npm list`, `npm ls` | top-level deps, conflicts, warnings | nested transitive dependencies | 95% |
| `git_log` | `git log` | compact hash+subject+author table | full messages, diffs, stats | 50-60% |

### Fallback

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `fallback` | anything else | exit code, truncated tail (last 50-100 lines) | bulk of output | 50-90% |

## Tool Detection

`detect_tool()` matches the command string against a list of patterns in priority order. More specific patterns come first.

```rust
fn detect_tool(command: &str) -> &'static str {
    if command.contains("llvm-cov") { return "cargo_llvm_cov"; }
    if command.starts_with("cargo test") { return "cargo_test"; }
    if command.starts_with("cargo") { return "cargo_build"; }
    if command.starts_with("pytest") { return "pytest"; }
    // ...
    "fallback"
}
```

The detection is string matching, not shell parsing. This is intentional: it's fast, predictable, and covers the common cases without the complexity of a full shell parser.

## JSON Auto-Detection (`--auto-json`)

Several tools support structured output natively. When `--auto-json` is passed, prx injects the appropriate JSON flag before running the command:

- `kubectl get` → adds `-o json`
- `terraform plan` → adds `-json`
- `npm ls` → adds `--json`
- `eslint` → adds `--format json`
- `mypy` → adds `--output json`

When the tool produces JSON output, prx parses it structurally instead of using regex. This is more reliable and handles edge cases that regex parsers miss.

If you pass `--json` yourself in the command, prx detects the JSON response and parses it structurally without needing `--auto-json`.

## Token Savings

On a passing test suite, the savings are dramatic:

- `cargo test` on a 200-test suite: ~50,000 tokens raw → ~200 tokens via prx (99% reduction)
- `pytest` on a 500-test suite: ~30,000 tokens raw → ~150 tokens via prx (99.5% reduction)

On a suite with failures, prx returns exactly the failures. A 200-test suite with 3 failures returns the 3 failure messages plus a summary line — typically 300-500 tokens regardless of how many tests passed.

## Adding a New Parser

Each parser is a module in `src/runner/`. To add a parser:

1. Create `src/runner/mytool.rs` with a `parse(output: &str) -> ParsedResult` function.
2. Add a detection pattern to `detect_tool()` in `src/runner/mod.rs`. Place it before any more general patterns it should take priority over.
3. Register the parser in the dispatch table in `parse_output()`.
4. Add inline tests with at least three cases: all-passing output, output with failures, and an edge case (empty output, mixed warnings, or a tool-specific quirk).

Test fixtures are string literals of representative command output. Keep them short (10-30 lines) — enough to exercise the regex patterns without bloating the test file.

## File Layout

```
src/runner/
├── mod.rs              # detect_tool, parse_output, execute, ParsedResult
├── cargo_build.rs      # cargo build/clippy
├── cargo_llvm_cov.rs   # cargo llvm-cov
├── cargo_test.rs       # cargo test
├── docker_build.rs     # docker build
├── dotnet.rs           # dotnet build/test
├── eslint.rs           # eslint
├── fallback.rs         # unknown commands
├── git_log.rs          # git log
├── go_cover.rs         # go test -cover
├── go_test.rs          # go test
├── gradle.rs           # gradle/gradlew
├── jest.rs             # jest/vitest
├── jest_cov.rs         # jest --coverage / c8
├── kubectl.rs          # kubectl describe/get
├── kubectl_logs.rs     # kubectl/docker logs
├── mvn.rs              # mvn/mvnw
├── mypy.rs             # mypy
├── npm_ls.rs           # npm list/ls
├── pytest.rs           # pytest
├── pytest_cov.rs       # pytest --cov / coverage
├── terraform.rs        # terraform plan/apply
└── tsc.rs              # tsc
```
