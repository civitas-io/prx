# Run Parsers: Design & Catalog

`prx run <command>` wraps CLI commands and returns structured JSON with
only actionable information. A passing `cargo test` suite producing 50k
tokens raw becomes ~200 tokens through prx.

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

**Pipeline:** `detect_tool()` matches the command string to a parser name.
`execute()` spawns the process, captures stdout+stderr. `parse_output()`
dispatches to the tool-specific parser. The fallback parser handles unknown
commands (truncated tail + exit code).

**Detection order matters:** More specific patterns must match first
(e.g. `cargo llvm-cov` before `cargo test`, `kubectl logs` before `kubectl`).

**No tree-sitter involvement.** Run parsers parse command OUTPUT (text logs,
compiler diagnostics), not source code. Tree-sitter is used elsewhere in prx
for code parsing (chunking, outline, snap, structural search). The one future
exception: enriching error locations with function context (e.g. "error in
authenticate() at auth.rs:42") — deferred.

## Parser Catalog

### Implemented (22 parsers)

**Test runners:**

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_test` | `cargo test` | pass/fail counts, failed test names+output | passing test lines | 95-99% |
| `pytest` | `pytest`, `python -m pytest` | pass/fail/skip counts, failed test names | passing test dots, collection output | 95-99% |
| `go_test` | `go test` | pass/fail counts, failed test output | passing `--- PASS` lines | 90-95% |
| `jest` | `jest`, `vitest`, `npm test` | pass/fail/skip counts, failed test output | passing test lines, transform output | 90-95% |
| `dotnet` | `dotnet test`, `dotnet build` | CS-prefixed errors/warnings, test failures | restore output, dependency noise | 75-85% |

**Build/lint tools:**

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_build` | `cargo build`, `cargo check`, `cargo clippy` | errors+warnings with file:line:col | help text, notes, duplicate messages | 80-90% |
| `mypy` | `mypy`, `python -m mypy` | `file:line: error:` lines, error count | notes without errors, success messages | 50% |
| `tsc` | `tsc`, `npx tsc` | TS errors with file:line:col | help suggestions, project config noise | 70-80% |
| `eslint` | `eslint` | lint errors/warnings with file:line | passing file notifications, fix suggestions | 60-80% |
| `mvn` | `mvn`, `mvnw` | compilation errors, Surefire failures, build result | download spam, dependency resolution | 90% |
| `gradle` | `gradle`, `gradlew` | FAILED tasks, compile errors, test summary | daemon startup, download progress | 85% |

**Coverage tools:**

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_llvm_cov` | `cargo llvm-cov` | coverage summary, low-coverage files | per-line coverage data | 90-95% |

**Infrastructure/DevOps:**

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `terraform` | `terraform plan`, `terraform apply` | changed resources, plan summary | `(known after apply)`, unchanged attrs | 75-85% |
| `kubectl` | `kubectl describe`, `kubectl get` | warning events, non-Ready conditions | normal events, managed fields | 80-90% |
| `kubectl_logs` | `kubectl logs`, `docker logs` | ERROR/WARN/FATAL + context, deduped | INFO/DEBUG lines, repeated lines | 70-90% |
| `docker_build` | `docker build`, `docker buildx` | failed step + context, image info | layer cache, download progress | 80% |
| `npm_ls` | `npm list`, `npm ls` | top-level deps, conflicts, warnings | nested transitive dependencies | 95% |
| `git_log` | `git log` | compact hash+subject+author table | full messages, diffs, stats | 50-60% |

**Fallback:**

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `fallback` | anything else | exit code, truncated tail (last 50/100 lines) | bulk of output | 50-90% |

### Planned — Coverage parsers

Coverage tools are a gap: we parse test output but not coverage reports.
All follow the same pattern as `cargo_llvm_cov`: extract summary + low-coverage
files, drop per-line detail.

| Parser | Commands | Extracts | Drops | Savings | Complexity |
|---|---|---|---|---|---|
| `pytest_cov` | `pytest --cov`, `coverage report` | total %, low-coverage files | per-line miss data, branch detail | 80-90% | Simple |
| `go_cover` | `go test -cover`, `go tool cover` | total %, per-package coverage | per-line annotations | 70-80% | Simple |
| `jest_cov` | `jest --coverage`, `c8`, `istanbul` | total %, uncovered files table | per-line detail, branch maps | 80-90% | Simple |

## Shared Infrastructure

### Generic log noise filter (kubectl_logs, docker logs)

Shared module for log-style output:
- Deduplicate consecutive identical lines → `[repeated 47 times]`
- Keep ERROR/WARN/FATAL lines with N lines of leading context
- Collapse INFO/DEBUG runs to count

Could live in `src/runner/log_filter.rs` and be called by `kubectl_logs`
and any future log parser.

### JVM build parser (mvn + gradle)

Both Maven and Gradle produce Surefire/Failsafe test reports with similar
format. Shared extraction for:
- `Tests run: N, Failures: N, Errors: N, Skipped: N`
- `[ERROR] file.java:[line,col] error: message`
- `BUILD SUCCESS` / `BUILD FAILURE`

Could share a `jvm_common.rs` module or just duplicate the ~20 lines of
regex — duplication is fine at this scale.

### JSON auto-detection

Several tools support structured output natively:
- `terraform plan -json` / `terraform apply -json`
- `kubectl get -o json` / `kubectl describe -o json`
- `npm ls --json`
- `eslint --format json` (already exists but we parse text)

**Decision:** The `--auto-json` flag on `prx run` actively injects JSON flags for kubectl, terraform, npm, eslint, mypy. The user typed a specific command; without the flag, we parse what they asked for. If they pass `--json` themselves, our parser should detect the JSON and extract from it.

### Common regex patterns

Reused across multiple parsers:
- `file:line:col: severity: message` (mypy, dotnet, tsc, cargo_build)
- Test summary `N passed, N failed` (cargo_test, pytest, go_test, jest, dotnet)
- Build result `BUILD SUCCESS`/`BUILD FAILED` (mvn, gradle)

These are simple enough to be per-parser regexes, not a shared abstraction.

## Implementation Plan

### Phases 1-3 — DONE (10 parsers shipped)

All 10 infrastructure/devops/build parsers implemented and tested.

### Phase 4 — Coverage parsers (~0.5 day)

| Parser | Pattern | Effort |
|---|---|---|
| `pytest_cov` | `Name Stmts Miss Cover` table → summary + low-coverage files | ~80 lines |
| `go_cover` | `coverage: N% of statements` line + per-package table | ~60 lines |
| `jest_cov` | `% Stmts` table (same as istanbul/c8) → summary + uncovered | ~80 lines |

### Phase 5 — Future (not scheduled)

| Parser | Notes |
|---|---|
| `ruff` | Python linter, supports `--output-format=json` |
| `golangci_lint` | Go linter, similar to eslint format |
| `bun_test` | Reuse jest parser — similar output |
| `deno_test` | Test failures + summary |

## Testing Strategy

Each parser has inline `#[cfg(test)] mod tests` with 2-3 tests:
1. **All-passing / clean output** — verify zero failures, correct summary
2. **With failures** — verify failure extraction, locations, messages
3. **Edge case** — empty output, mixed warnings+errors, or tool-specific quirk

Test fixtures are string literals of representative command output. Keep them
short (10-30 lines) — enough to exercise the regex patterns without bloating
the test file.

`detect_tool` tests go in `src/runner/mod.rs` tests section (one per parser,
matching existing pattern).

## Low-ROI parsers to reconsider

| Parser | Concern |
|---|---|
| `kubectl` (describe/get) | Output varies wildly by resource type. Consider supporting only `kubectl get` with table parsing, skip `describe` initially. |
| `npm_ls` | If user passes `--json`, we should parse that instead of the tree format. Text tree parsing is brittle for deeply nested deps. |
| `gradle` | Very similar to mvn. If mvn is done well, gradle may not justify separate implementation if agent usage is low. |

## File layout after implementation

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

22 implemented parsers.
