# Coding Guidelines

These guidelines apply to all code in prx. They're based on Karpathy's guidelines for reducing LLM coding mistakes, adapted for this codebase. The goal is fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions before implementation rather than after mistakes.

## Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

- State assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what is confusing. Ask.

## Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

The test: would a senior engineer say this is overcomplicated? If yes, simplify.

## Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

Every changed line should trace directly to the request.

## Error Handling

Use `thiserror` for library errors, `anyhow` for CLI entry points.

```rust
// Library errors (thiserror)
#[derive(thiserror::Error, Debug)]
pub enum AgError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// CLI errors (anyhow)
fn main() -> anyhow::Result<()> {
    let result = do_work().context("failed to process")?;
    Ok(())
}
```

**Never `unwrap()` in library code.** `unwrap()` and `expect()` are forbidden outside `#[cfg(test)]` modules. Use `?` propagation with typed errors.

**Unsafe is forbidden** without explicit justification in a code comment.

## Public API Documentation

All public functions and types must have doc comments:

```rust
/// Searches the codebase for chunks matching the query.
///
/// Returns ranked results up to the token budget. If no budget is specified,
/// returns all results above the relevance threshold.
pub fn search(query: &str, path: &Path, opts: SearchOpts) -> Result<Vec<Match>, AgError> {
    // ...
}
```

These doc comments become `--help` text for clap arguments. Write them for the person reading the help output, not just for rustdoc.

Comments in function bodies should explain WHY, not WHAT. If the code is clear, no comment is needed.

## Dependencies

Every new dependency added to `Cargo.toml` must have a comment explaining why it's needed and why an existing dependency can't serve the purpose:

```toml
# sprs: sparse matrix operations for BM25 scoring.
# ndarray doesn't support CSC sparse format; sprs is the standard Rust sparse matrix crate.
sprs = "0.11"
```

Minimize dependencies. A new crate adds compile time, binary size, and supply chain risk. Before adding one, check whether an existing dependency already provides the functionality.

## Output

All output must go through the JSON envelope in `src/output.rs`. Never `println!()` directly to stdout from command handlers.

Errors go to stdout as structured JSON, never to stderr. stderr is reserved for `RUST_LOG` debug logging only.

Every command that returns file content or search results must respect `--budget`. The infrastructure must support it even if the default is unlimited.

## Platform Behavior

No `#[cfg(target_os)]` in command logic. Platform differences are isolated to `src/parsing/languages.rs` (grammar loading) and the notify crate (file watching). Everything else is pure cross-platform Rust.

## Testing

| Tier | Location | Command |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` inline in each module | `make test-unit` |
| Integration tests | `tests/e2e.rs` — test CLI binary end-to-end | `make test-e2e` |
| Benchmarks | `benches/` — criterion benchmarks | `make bench` |

Test data lives in `tests/fixtures/` — small sample files in multiple languages.

Coverage target: >= 80%.

**Unit test structure:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_camel_case() {
        let tokens = tokenize_identifier("getHTTPResponse");
        assert_eq!(tokens, vec!["gethttpresponse", "get", "http", "response"]);
    }
}
```

**Integration test structure:**

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_search_literal() {
    Command::cargo_bin("prx").unwrap()
        .args(["search", "--literal", "fn main", "tests/fixtures/"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"ok\""));
}
```

## Pre-Merge Checklist

- [ ] On a `dev/vX.Y.Z` branch (not `main`)
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] `cargo deny check` passes
- [ ] `cargo build --release` succeeds
- [ ] No `unwrap()` in non-test code
- [ ] Public functions have `///` doc comments
- [ ] JSON output matches schemas in `docs/design/OUTPUT.md`
- [ ] `AGENTS.md` updated if layout or conventions changed
- [ ] `CHANGELOG.md` updated for user-visible changes
- [ ] `Cargo.toml` version bumped

## Git Workflow

No direct pushes to `main`. All work happens on `dev/vX.Y.Z` branches.

Version semantics: `v0.X.0` = features (new capabilities). `v0.X.Y` = fixes and improvements only.

```bash
git checkout -b dev/v0.4.1 main   # cut branch
# ... develop, commit, test ...
# get human sign-off before merging
git checkout main && git merge --no-ff dev/v0.4.1
git tag -a v0.4.1 -m "..."
git push origin main && git push origin v0.4.1
git branch -d dev/v0.4.1
```
