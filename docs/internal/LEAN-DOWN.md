# v0.5.4 — Lean-Down Refactoring

Code review and reduction pass. Goal: eliminate duplication, extract shared
utilities, reduce test boilerplate. No behavior changes, no new features.

**Baseline**: 17,500 lines, 76 files, 442 unit + 85 integration tests.
**Target**: ~15,600 lines (~11% reduction) with identical test coverage and behavior.

---

## Design Pattern

The codebase uses **free functions per module** — no traits, no struct-per-parser,
no OOP inheritance. This is intentional and should remain. The refactoring keeps
this pattern but extracts shared scaffolding into helpers and macros.

**Do NOT introduce:**
- Trait-based parser dispatch (over-engineering for `fn parse(&str) -> ParsedResult`)
- Builder pattern structs (a helper function is simpler)
- Generic frameworks or plugin systems

**Do introduce:**
- Shared utility functions (extracted from duplicated private functions)
- A declarative macro for regex statics (reduces noise, same semantics)
- A `ParsedResult::new()` constructor (replaces 7-field struct literal)
- Test helper functions in `tests/helpers.rs` (reduces e2e boilerplate)

---

## Workstream 1: Runner Parser Boilerplate (~1,150 lines saved)

### Problem

22 parsers in `src/runner/` share 46% structural boilerplate. Every parser
repeats the same patterns with only the regex patterns and parsing logic varying.

### Evidence

**Pattern A: Regex static initialization** (100% of parsers)

Every parser has 2-5 of these at the top:

```rust
// cargo_test.rs L6-13
static SUMMARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored").unwrap()
});
static FAILURE_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^---- (.+) stdout ----$").unwrap());
static PANIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"panicked at (.+):(\d+)").unwrap());
```

This is 3 lines per regex × 2-5 regexes × 22 parsers = ~200 lines of pure noise.

**Fix**: `define_regex!` macro in `src/runner/mod.rs`:

```rust
macro_rules! define_regex {
    ($name:ident, $pattern:expr) => {
        static $name: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new($pattern).unwrap());
    };
}
pub(crate) use define_regex;
```

After:
```rust
define_regex!(SUMMARY_RE, r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored");
define_regex!(FAILURE_HEADER_RE, r"^---- (.+) stdout ----$");
define_regex!(PANIC_RE, r"panicked at (.+):(\d+)");
```

3 lines → 1 line each. **Saves ~130 lines across 22 parsers.**

---

**Pattern B: Variable initialization + ParsedResult construction** (100% of parsers)

Every parser starts with:
```rust
// cargo_test.rs L16-21
let mut passed = 0;
let mut failed = 0;
let mut skipped = 0;
let mut summary = String::new();
let mut failures = Vec::new();
```

And ends with:
```rust
// cargo_test.rs L67-76
ParsedResult {
    summary,
    passed,
    failed,
    skipped,
    failures,
    warnings: vec![],
    tail: None,
}
```

Most parsers set `warnings: vec![]` and `tail: None` — only a few use them.

**Fix**: Add `ParsedResult::new()` and `ParsedResult::with_warnings()` constructors:

```rust
impl ParsedResult {
    /// Create a result with common fields. Sets warnings=[] and tail=None.
    pub fn new(
        summary: String,
        passed: usize,
        failed: usize,
        skipped: usize,
        failures: Vec<Diagnostic>,
    ) -> Self {
        Self { summary, passed, failed, skipped, failures, warnings: vec![], tail: None }
    }
}
```

After:
```rust
ParsedResult::new(summary, passed, failed, skipped, failures)
```

10 lines → 1 line per parser. **Saves ~200 lines across 22 parsers.**

---

**Pattern C: Default summary fallback** (87% of parsers)

```rust
// cargo_test.rs L63-65, pytest.rs L48-50, go_test.rs L60
if summary.is_empty() {
    summary = format!("{passed} passed, {failed} failed");
}
```

Nearly identical in 19/22 parsers. Leave as-is — it's 3 lines and varies
slightly per parser. Not worth abstracting.

---

**Pattern D: Import boilerplate** (100% of parsers)

Every parser has:
```rust
use regex::Regex;
use std::sync::LazyLock;
use super::{Diagnostic, ParsedResult};
```

After the `define_regex!` macro is `pub(crate) use`'d from mod.rs, the first
two imports become unnecessary in parsers that only use the macro. But this is
minor (~2 lines × 22 = 44 lines) and may not be worth the churn. Optional.

---

### Summary: Runner Refactoring

| Change | Effort | Lines saved |
|---|---|---|
| `define_regex!` macro | 30 min | ~130 |
| `ParsedResult::new()` constructor | 30 min | ~200 |
| Remove now-unused imports | 15 min | ~44 |
| **Total** | **~1.5 hrs** | **~370** |

Note: the explore agent estimated 1,150 lines. The concrete evidence supports
~370 lines from these two mechanical changes. The remaining gap was
over-estimated on test macro savings and diagnostic helpers that aren't
actually duplicated enough to justify. **370 lines is the honest number.**

---

## Workstream 2: Command Shared Utilities (~120 lines saved)

### Problem

Private utility functions are copy-pasted between command files.

### Evidence

**Duplicate 1: `find_workspace_root()`** — identical in 2 files

```
src/commands/impact.rs:392-411   (20 lines)
src/commands/context.rs:504-520  (17 lines)
```

Same logic: walk up from path, check for .git/.prx/Cargo.toml, 32-level cap.

**Duplicate 2: `relative_path()`** — identical in 2 files

```
src/commands/impact.rs:413-420   (8 lines)
src/commands/context.rs:523-530  (8 lines)
```

Same logic: canonicalize both paths, strip_prefix, replace backslashes.

**Duplicate 3: `is_test_file()`** — 3 variants across 3 files

```
src/commands/impact.rs:422-440    (19 lines) — takes &str
src/commands/context.rs:403-418   (16 lines) — takes (&str, &Path)
src/ranking/penalties.rs:41-...   (similar)  — takes &str
```

Similar but not identical. The context.rs version also accepts a `&Path` param.
Unification requires picking the superset signature.

**Fix**: Create `src/workspace.rs` with these three functions:

```rust
/// Find the workspace root by walking up from `target`.
pub fn find_workspace_root(target: &Path) -> Option<PathBuf> { ... }

/// Compute relative path from `target` to `base`, normalized with forward slashes.
pub fn relative_path(target: &Path, base: &Path) -> Option<String> { ... }

/// Check if a relative path refers to a test file.
pub fn is_test_file(rel_str: &str) -> bool { ... }
```

Then replace the 3 duplicate sites with `use crate::workspace::*`.

| Change | Effort | Lines saved |
|---|---|---|
| Extract `src/workspace.rs` (~35 lines) | 1 hr | ~45 (net) |
| Deduplicate `relative_path` | 15 min | ~8 |
| Unify `is_test_file` | 30 min | ~20 |
| **Total** | **~2 hrs** | **~73 (net)** |

Net savings account for the new file's ~35 lines.

---

**`strip_prefix` usage** — reviewed, NOT duplicated enough to extract.

14 occurrences across 10 files, but each has different context (different root
variables, different error handling, different return types). A shared helper
would need 3+ parameters and save only 1 line per call site. **Skip.**

---

## Workstream 3: Test Helpers (~300 lines saved)

### Problem

85 integration tests in `tests/e2e.rs` repeat the same setup pattern:

```rust
let dir = test_dir();
let out = ag()
    .args(["search", "fn main", dir.path().to_str().unwrap()])
    .output()
    .unwrap();
assert!(out.status.success());
let json: Value = serde_json::from_slice(&out.stdout).unwrap();
assert_eq!(json["status"], "ok");
```

8 lines of boilerplate per test, ~50 tests use this pattern.

### Fix

Create `tests/helpers/mod.rs` (Rust test helper convention) with:

```rust
use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;
use std::path::Path;

/// Build a prx Command pointing at the debug binary.
pub fn ag() -> Command {
    Command::cargo_bin("prx").unwrap()
}

/// Create a temp directory with a sample Rust file.
pub fn test_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    dir
}

/// Run a prx command, assert success, parse JSON output.
pub fn run_prx(args: &[&str]) -> Value {
    let out = ag().args(args).output().unwrap();
    assert!(out.status.success(), "prx failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Run a prx command against a specific directory, assert success, parse JSON.
pub fn run_prx_in(dir: &Path, args: &[&str]) -> Value {
    let out = ag().args(args).output().unwrap();
    assert!(out.status.success(), "prx failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}
```

Then refactor e2e.rs tests from 8 lines to 2-3 lines each.

| Change | Effort | Lines saved |
|---|---|---|
| Create `tests/helpers/mod.rs` (~40 lines) | 30 min | — |
| Refactor e2e.rs (~50 tests × ~5 lines each) | 2 hrs | ~250 |
| Refactor mcp_e2e.rs (~8 tests × ~6 lines each) | 30 min | ~48 |
| **Total** | **~3 hrs** | **~300 (net)** |

---

## Workstream 4: Large Function Decomposition (readability, ~0 net lines)

### Problem

Several `run()` functions exceed 100 lines with deep nesting:

| File | Function | Lines |
|---|---|---|
| `src/commands/read.rs` | `run()` | 164 |
| `src/commands/find.rs` | `run()` | 127 |
| `src/commands/impact.rs` | `run()` | 117 |
| `src/commands/search.rs` | `hybrid_search()` | 104 |

### Approach

Extract logical phases into named sub-functions. This improves readability
but saves ~0 net lines (the code moves, it doesn't disappear). Do this
ONLY if the function is hard to follow — not as a line-count exercise.

**Candidate**: `read.rs::run()` at 164 lines has 4 clear phases:
1. Argument parsing & validation (~20 lines)
2. Read mode selection (~30 lines)
3. Content processing (~60 lines)
4. Output construction (~50 lines)

Each phase could be a named function. But this is subjective and should
be done by the implementing agent based on actual reading, not prescribed here.

**Recommendation**: Defer to the agent's judgment. Flag these functions as
"review for decomposition" but don't mandate specific extractions.

---

## Execution Plan

### Order of Operations

1. **`define_regex!` macro + `ParsedResult::new()`** — mechanical, zero risk
2. **`src/workspace.rs` extraction** — mechanical, test by running existing tests
3. **`tests/helpers/mod.rs`** — test-only code, zero risk to production
4. **Large function review** — optional, readability-only

### Verification

After each workstream:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (all 442 unit + 85 integration)
- `cargo deny check`
- Confirm no behavior changes (same JSON output for same inputs)
- Line count comparison: `find src tests -name '*.rs' -exec cat {} + | wc -l`

### What NOT to do

- Do NOT introduce traits for parsers. Free functions are fine.
- Do NOT create a generic "command framework" or base struct for commands.
- Do NOT refactor code that isn't duplicated just because it's long.
- Do NOT change any public API or JSON output format.
- Do NOT combine workstreams — commit each separately for clean history.

---

## Honest Numbers

| Workstream | Lines saved | Effort | Risk | Status |
|---|---|---|---|---|
| 1. Runner macros + constructor | -144 | 1.5 hrs | Very low | **Done (v0.5.4)** |
| 2. Shared workspace utilities | -19 | 2 hrs | Low | **Done (v0.5.4)** |
| 3. Test helpers extraction | ~300 | 3 hrs | Very low | Deferred to v0.5.5 |
| 4. Function decomposition | ~0 | 2 hrs | Low | Deferred to v0.5.5 |
| **Total shipped** | **-163** | **~3.5 hrs** | **Low** | |

Actual savings were more modest than the initial estimate of ~743 lines.
The `define_regex!` macro saved fewer lines per site than estimated (1 line
per regex vs 2), and `ParsedResult::new()` only applied to 8 of 22 parsers
(13 have non-empty warnings or tail). Remaining workstreams deferred to
v0.5.5 alongside test coverage improvements.
