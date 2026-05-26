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

### Implemented (9 parsers)

| Parser | Commands | Extracts | Drops | Savings |
|---|---|---|---|---|
| `cargo_test` | `cargo test` | pass/fail counts, failed test names+output | passing test lines | 95-99% |
| `cargo_build` | `cargo build`, `cargo check`, `cargo clippy` | errors+warnings with file:line:col | help text, notes, duplicate messages | 80-90% |
| `cargo_llvm_cov` | `cargo llvm-cov` | coverage summary, low-coverage files | per-line coverage data | 90-95% |
| `pytest` | `pytest`, `python -m pytest` | pass/fail/skip counts, failed test names | passing test dots, collection output | 95-99% |
| `go_test` | `go test` | pass/fail counts, failed test output | passing `--- PASS` lines | 90-95% |
| `jest` | `jest`, `vitest`, `npm test` | pass/fail/skip counts, failed test output | passing test lines, transform output | 90-95% |
| `tsc` | `tsc`, `npx tsc` | TS errors with file:line:col | help suggestions, project config noise | 70-80% |
| `eslint` | `eslint` | lint errors/warnings with file:line | passing file notifications, fix suggestions | 60-80% |
| `fallback` | anything else | exit code, truncated tail (last 50/100 lines) | bulk of output | 50-90% |

### Planned (10 parsers)

| Parser | Commands | Extracts | Drops | Savings | Complexity | JSON native? |
|---|---|---|---|---|---|---|
| `mypy` | `mypy`, `python -m mypy` | `file:line: error:` lines, error count | notes without errors, success messages | 50% | Simple | No |
| `git_log` | `git log` | Compact hash+subject+author table | Full commit messages, diffs, stats | 50-60% | Simple | `--format` available |
| `docker_build` | `docker build`, `docker buildx build` | Failed step + last 20 lines, image ID | Layer cache output, download progress, intermediate steps | 80% | Moderate | No |
| `terraform` | `terraform plan`, `terraform apply` | Changed resources, plan summary | `(known after apply)`, unchanged attrs, provider init | 75-85% | Moderate | `-json` available |
| `kubectl` | `kubectl describe`, `kubectl get` | Warning events, non-Ready conditions, error phases | Normal events, managed fields, healthy status | 80-90% | Moderate | `-o json` available |
| `kubectl_logs` | `kubectl logs`, `docker logs` | ERROR/WARN/FATAL + context, deduped | INFO/DEBUG lines, repeated identical lines | 70-90% | Moderate | No |
| `mvn` | `mvn`, `mvnw`, `./mvnw` | Compilation errors, Surefire failures, build result | `Downloading from`, `Downloaded from`, resolution | 90% | Complex | No |
| `gradle` | `gradle`, `gradlew`, `./gradlew` | Compilation errors, test failures, build result | Daemon startup, dependency resolution, download progress | 85% | Complex | No |
| `dotnet` | `dotnet build`, `dotnet test` | CS-prefixed errors/warnings with file:line, test failures | Restore output, dependency noise | 75-85% | Simple | No |
| `npm_ls` | `npm list`, `npm ls` | Top-level deps, version conflicts, peer dep warnings | Nested transitive dependencies | 95% | Simple | `--json` available |

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

**Decision:** Do NOT auto-inject `--json` flags. The user typed a specific
command; we parse what they asked for. If they pass `--json` themselves, our
parser should detect the JSON and extract from it. But a future `--auto-json`
flag on `prx run` could add this.

### Common regex patterns

Reused across multiple parsers:
- `file:line:col: severity: message` (mypy, dotnet, tsc, cargo_build)
- Test summary `N passed, N failed` (cargo_test, pytest, go_test, jest, dotnet)
- Build result `BUILD SUCCESS`/`BUILD FAILED` (mvn, gradle)

These are simple enough to be per-parser regexes, not a shared abstraction.

## Implementation Plan

### Phase 1 — Easy wins (3 parsers, ~1 day)

| Parser | Why first | Effort |
|---|---|---|
| `mypy` | Simple `file:line: error:` format, high agent usage | ~60 lines |
| `dotnet` | Same `file(line,col): error CSxxxx:` pattern as cargo_build | ~70 lines |
| `git_log` | Simple commit parsing, broadly useful | ~80 lines |

### Phase 2 — Moderate (4 parsers, ~1.5 days)

| Parser | Notes | Effort |
|---|---|---|
| `docker_build` | Step detection, failure context window | ~90 lines |
| `npm_ls` | Depth filtering, conflict detection | ~80 lines |
| `terraform` | Plan summary extraction, resource change counting | ~100 lines |
| `kubectl_logs` | Needs the generic log noise filter | ~120 lines (incl. filter) |

### Phase 3 — Complex (3 parsers, ~1.5 days)

| Parser | Notes | Effort |
|---|---|---|
| `kubectl` | Multiple subcommand formats (describe, get, events) | ~120 lines |
| `mvn` | Surefire parsing, download noise filtering | ~110 lines |
| `gradle` | Similar to mvn but different noise patterns | ~100 lines |

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
├── docker_build.rs     # docker build        [NEW]
├── dotnet.rs           # dotnet build/test   [NEW]
├── eslint.rs           # eslint
├── fallback.rs         # unknown commands
├── git_log.rs          # git log             [NEW]
├── go_test.rs          # go test
├── gradle.rs           # gradle/gradlew      [NEW]
├── jest.rs             # jest/vitest
├── kubectl.rs          # kubectl describe/get [NEW]
├── kubectl_logs.rs     # kubectl/docker logs  [NEW]
├── mvn.rs              # mvn/mvnw            [NEW]
├── mypy.rs             # mypy                [NEW]
├── npm_ls.rs           # npm list/ls         [NEW]
├── pytest.rs           # pytest
├── terraform.rs        # terraform plan/apply [NEW]
└── tsc.rs              # tsc
```

19 parsers total (9 existing + 10 new). Each 50-120 lines including tests.
