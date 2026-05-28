# AGENTS.md -- prx (Praxis)

> Machine-readable project reference for AI coding assistants.
> Last updated: 2026-05-18

## Project Identity

**prx** (Praxis) is a busybox-style Rust binary providing agent-native
replacements for core Unix tools. Every subcommand returns structured JSON output with token
budgets, structural awareness, and content hashing -- designed for AI coding
agents, not humans.

- **Repository:** `github.com/civitas-io/prx`
- **License:** Apache 2.0
- **Language:** Rust (edition 2024)
- **Status:** v0.2.0 released

### What prx Is

- A single static binary (~47 MB) with zero runtime dependencies
- Replaces grep, cat, find, sed, diff with agent-native equivalents
- Embeds a 32M-parameter retrieval-optimized embedding model for semantic code search
- Returns structured JSON with labeled fields, token counts, and content hashes
- Works offline, in sandboxes, in containers -- no internet, no daemon, no setup

### What prx Is NOT

- NOT a wrapper around existing tools (unlike RTK, squeez, LeanCTX)
- NOT search-only — covers the full read-search-edit-diff loop
- NOT a framework or SDK -- it is a CLI tool that agents invoke
- NOT an AI/LLM -- the tool is deterministic; the agent calling it is the LLM
- NOT a replacement for LSP -- prx provides structural awareness without a server

---

## Using prx (for agents consuming this tool)

### Installation

prx ships as a single binary. No package manager, no runtime, no dependencies.

```bash
# Download prebuilt binary (Linux x86_64 example)
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-linux-x86_64 -o prx
chmod +x ag

# Or install via cargo
cargo install prx
```

### Quick Reference

```bash
# Search -- find code by meaning, not just text
prx search "authentication flow" src/          # semantic (auto-detected)
prx search --literal "authenticate(" src/      # exact match, ripgrep-speed
prx search --structural 'fn $NAME($$$) { $$$ }' src/   # AST pattern matching

# Read -- structured file access
prx read src/auth.ts                           # full file with metadata
prx read src/auth.ts --skeleton                # signatures and exports only
prx read src/auth.ts --lines 42-67 --snap fn   # expand to enclosing function
prx read src/auth.ts --outline                 # symbol table
prx read src/auth.ts --if-changed abc123...    # skip if unchanged (returns cached stub)
prx read src/auth.ts --mode aggressive         # strip comments (1-19% savings)
prx read src/auth.ts --mode diff               # changed lines vs git HEAD (80-97%)
prx read schema.rs --mode entropy              # filter repetitive code (5-87%)

# Find -- codebase mapping
prx find src/ --pattern "*.ts" --depth 3       # bounded file discovery
prx find src/ --changed-since HEAD~3           # recently modified files

# Edit -- safe, verified modifications
prx edit src/auth.ts --find "old_call()" --replace "new_call()"
prx edit src/auth.ts --find "old_call()" --replace "new_call()" --apply

# Diff -- semantic change summaries
prx diff src/auth.ts --since HEAD~1            # structured diff
prx diff --stat-only                           # summary in ~30 tokens

# Run -- structured command output (95-99% token savings on tests)
prx run cargo test                             # parsed test results
prx run cargo clippy                           # parsed warnings/errors
prx run pytest                                 # parsed test results
prx run cargo build                            # parsed build errors

# Utilities
prx exists "pattern" src/                      # O(1) existence check
prx outline src/auth.ts                        # symbol outline
prx batch < commands.jsonl                     # parallel batch execution
```

### Output Format

All output is JSON. Every response follows this envelope:

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": { ... }
}
```

Errors are also JSON, on stdout, never stderr:

```json
{
  "version": "0.2.0",
  "command": "read",
  "status": "error",
  "error": {
    "code": "file_not_found",
    "message": "File not found: src/missing.ts",
    "suggestion": "Use `prx find` to discover files."
  }
}
```

Use `--plain` for human-readable output. Use `--budget N` to cap token usage.

### Workflow Guidance

1. Start with `prx search` to find relevant code. Prefer semantic queries for
   unfamiliar codebases, literal queries for known identifiers.
2. Use `prx read --skeleton` or `prx read --outline` before reading full files.
   This costs ~10% of the tokens and tells you what is in the file.
3. Use `prx read --snap function` to read specific functions without pulling the
   entire file into context.
4. Re-reading a file? Pass `--if-changed <previous_hash>` to skip if unchanged.
   The hash is in `meta.hash` from the previous response. Cache hits cost ~50 bytes.
5. Use `prx exists "pattern"` before full searches when you just need a yes/no.
6. Use `prx edit` to preview changes (dry-run is the default). Add `--apply` to write.
7. Use `prx diff --stat-only` for cheap change detection (~30 tokens).
8. Use `prx run cargo test` instead of raw `cargo test` — returns only failures,
   saves 95-99% tokens on passing test suites.
9. Use `prx batch` to combine multiple independent queries in one round-trip.
10. Use `--budget N` on every content-returning command to control token cost.

### Integration Strategy

prx supports three integration tiers. Use all three for full coverage.

**Tier 1: CLI on PATH (universal, works everywhere)**

Install the binary and add the usage snippet to your project's AGENTS.md or
CLAUDE.md. This is the primary integration — it works for top-level agents,
sub-agents, scripts, CI, and humans. Run `prx init --agents-md` to append the
snippet automatically.

**Tier 2: MCP server (richer integration, top-level agents only)**

```json
{
  "mcpServers": {
    "prx": {
      "command": "prx",
      "args": ["mcp"]
    }
  }
}
```

Works with Claude Code, Cursor, Codex, OpenCode. Provides typed tool
parameters and auto-discovery. However, sub-agents cannot call MCP tools
(confirmed limitation in Claude Code and Codex CLI).

**Tier 3: Agent definition (Claude Code sub-agents)**

```bash
prx init --agent claude-code
```

Writes `.claude/agents/prx-search.md`, creating a dedicated sub-agent that
uses prx via bash with optimized workflow guidance.

**Quick setup for any framework:**

```bash
prx init           # auto-detect frameworks, write all configs
```

### Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All
breaking changes are documented in CHANGELOG.md with migration guides. Use the
`version` field in JSON output for programmatic detection.

---

## Developing prx (for agents contributing to this codebase)

### Conventions

| Convention | Standard |
|---|---|
| Language | Rust, edition 2024 |
| MSRV | 1.85 |
| Formatter | `cargo fmt` (rustfmt defaults) |
| Linting | `cargo clippy -- -D warnings` |
| Testing | `cargo test` |
| Build | `cargo build --release` |
| License | Apache 2.0 |
| Package layout | `src/` (single crate) |

### Code Style

- **Formatter / linter:** `cargo fmt` + `cargo clippy`. No custom rustfmt config.
- **Line length:** 100 (rustfmt default).
- **Error handling:** Use `thiserror` for library errors, `anyhow` for CLI.
  Never `unwrap()` in library code. `unwrap()` acceptable only in tests.
- **Unsafe:** Forbidden without explicit justification in a code comment.
- **Comments:** only when the WHY is non-obvious.
- **Public API:** all pub functions and types must have doc comments (`///`).
- **Dependencies:** minimize. Every new dependency must justify its inclusion.

### Coding Discipline

Based on [Karpathy's guidelines](https://github.com/multica-ai/andrej-karpathy-skills)
for reducing LLM coding mistakes. Biases toward caution over speed. For trivial
tasks (typo fixes, obvious one-liners), use judgment.

**1. Think before coding.** Do not assume. Do not hide confusion. Surface
tradeoffs.

- State assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them -- do not pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what is confusing. Ask.

**2. Simplicity first.** Minimum code that solves the problem. Nothing
speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that was not requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.
- The test: would a senior engineer say this is overcomplicated? If yes,
  simplify.

**3. Surgical changes.** Touch only what you must. Clean up only your own mess.

When editing existing code:
- Do not "improve" adjacent code, comments, or formatting.
- Do not refactor things that are not broken.
- Match existing style, even if you would do it differently.
- If you notice unrelated dead code, mention it -- do not delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Do not remove pre-existing dead code unless asked.

The test: every changed line should trace directly to the request.

**4. Goal-driven execution.** Define success criteria. Loop until verified.

Transform tasks into verifiable goals:
- "Add validation" becomes "write tests for invalid inputs, then make them pass"
- "Fix the bug" becomes "write a test that reproduces it, then make it pass"
- "Refactor X" becomes "ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] -> verify: [check]
2. [Step] -> verify: [check]
3. [Step] -> verify: [check]
```

Strong success criteria enable independent work. Weak criteria ("make it work")
require constant clarification.

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer
rewrites due to overcomplication, and clarifying questions come before
implementation rather than after mistakes.

### Testing

- **Unit tests:** inline `#[cfg(test)] mod tests` in each module.
- **Integration tests:** `tests/integration/` -- test CLI binary end-to-end.
- **Benchmarks:** `benches/` -- criterion benchmarks for search, chunking, ranking.
- Test data: `tests/fixtures/` -- small sample files in multiple languages.
- Coverage target: >= 80%.

### Repository Layout

```
ag/
├── AGENTS.md                    # This file
├── CLAUDE.md                    # Thin entrypoint (delegates to AGENTS.md)
├── README.md                    # Public-facing overview
├── CHANGELOG.md                 # Release history
├── Cargo.toml                   # Dependencies, features, metadata
├── Cargo.lock                   # Locked dependency versions
│
├── docs/
│   ├── vision/
│   │   ├── PRD.md               # Product requirements
│   │   └── ROADMAP.md           # Phased delivery plan
│   ├── architecture/
│   │   ├── SYSTEM.md            # System architecture
│   │   └── SEARCH.md            # Search subsystem architecture
│   ├── design/
│   │   ├── CLI.md               # CLI interface specification
│   │   ├── OUTPUT.md            # JSON output format specification
│   │   ├── BENCHMARKS.md        # Benchmarking plan and methodology
│   │   ├── SYSTEM-DESIGN.md     # Detailed design for all 20 subsystems
│   │   ├── IMPLEMENTATION.md   # Step-by-step implementation plan
│   │   ├── TESTING.md          # Testing plan (unit, integration, benchmarks)
│   │   └── CRATE-REFERENCE.md  # Crate versions, APIs, and compatibility
│   ├── research/
│   │   ├── LANDSCAPE.md         # Competitive landscape analysis
│   │   └── PLATFORM.md          # Cross-platform compatibility audit
│   └── assets/                  # SVG diagrams
│
├── CONTRIBUTING.md              # Developer setup and workflow guide
│
├── src/
│   ├── main.rs                  # CLI entry point, clap dispatch
│   ├── lib.rs                   # Library surface (public API)
│   ├── output.rs                # JSON envelope, error formatting
│   ├── tokens.rs                # Token counting (tokenizers crate)
│   ├── hash.rs                  # Content hashing (xxh3)
│   ├── walk.rs                  # File walking (ignore crate, .gitignore/.prxignore)
│   ├── fallback.rs              # Graceful fallback to grep/cat/find on internal errors
│   │
│   ├── commands/                # Subcommand handlers
│   │   ├── mod.rs
│   │   ├── search.rs            # prx search
│   │   ├── read.rs              # prx read
│   │   ├── find.rs              # prx find
│   │   ├── edit.rs              # prx edit
│   │   ├── diff.rs              # prx diff
│   │   ├── batch.rs             # prx batch
│   │   ├── bench.rs             # prx bench (synthetic benchmarks)
│   │   ├── context.rs           # prx context (module context package)
│   │   ├── impact.rs            # prx impact (reverse dependency analysis)
│   │   ├── index.rs             # prx index
│   │   ├── init.rs              # prx init
│   │   ├── mcp.rs               # prx mcp
│   │   ├── outline.rs           # prx outline
│   │   ├── exists.rs            # prx exists
│   │   ├── stats.rs             # prx stats
│   │   ├── run.rs               # prx run (structured command runner)
│   │
│   ├── search/                  # Search engine
│   │   ├── mod.rs
│   │   ├── fusion.rs            # RRF fusion, adaptive alpha
│   │   ├── graph.rs             # Import graph (BFS, persistence, suffix resolution)
│   │   ├── semantic.rs          # Model2Vec embedding search
│   │   ├── literal.rs           # Regex/literal search
│   │   ├── structural.rs        # ast-grep pattern search
│   │   ├── tokenize.rs          # Identifier tokenization (camelCase/snake_case)
│   │   └── symbols.rs           # Symbol index (definition lookup, reference counting)
│   │
│   ├── chunking/                # Code chunking
│   │   ├── mod.rs
│   │   └── treesitter.rs        # Tree-sitter AST chunking
│   │
│   ├── ranking/                 # Result ranking
│   │   ├── mod.rs
│   │   ├── boosting.rs          # Definition boost, stem matching, coherence
│   │   ├── penalties.rs         # Noise penalties, saturation decay
│   │   ├── proximity.rs         # Import graph proximity boost
│   │   └── weighting.rs         # Alpha weight resolution
│   │
│   ├── index/                   # Index management
│   │   ├── mod.rs
│   │   ├── dense.rs             # Model2Vec embeddings
│   │   ├── sparse.rs            # BM25 sparse matrix
│   │   └── bloom.rs             # Bloom filter for exists
│   │
│   └── parsing/                 # Tree-sitter integration
│       ├── mod.rs
│       ├── imports.rs           # Per-language regex import extraction (7 languages)
│       ├── languages.rs         # Language detection, grammar loading
│       ├── outline.rs           # Symbol extraction
│       ├── snap.rs              # Structural snapping (function/class boundaries)
│       └── strip.rs             # Tree-sitter comment stripping (--mode aggressive)
│
│   └── runner/                  # prx run parsers
│       ├── mod.rs               # Runner framework, tool detection
│       ├── cargo_test.rs        # cargo test output parser
│       ├── cargo_build.rs       # cargo build/clippy output parser
│       ├── pytest.rs            # pytest output parser
│       ├── go_test.rs           # go test output parser
│       ├── jest.rs              # jest/npm test/vitest output parser
│       ├── tsc.rs               # TypeScript compiler output parser
│       ├── eslint.rs            # eslint output parser
│       └── fallback.rs          # Unknown command fallback
│
├── models/                      # Embedding model weights (build-time)
│   └── potion-code-16M.safetensors  # Included via include_bytes!
│
├── skills/
│   └── agents.md                # Agent-facing skill guide (install, usage, integration)
│
├── tests/
│   ├── integration/             # CLI integration tests
│   └── fixtures/                # Sample source files for testing
│
└── benches/                     # Criterion benchmarks
```

> This layout is authoritative. If you add or remove a module, update this section.

---

## Key Architectural Decisions

These are settled decisions. Do not revisit without discussion.

| # | Decision | Rationale |
|---|---|---|
| 1 | **Single binary, busybox-style** | clap multicall. `prx search` or hardlink `prx-search`. Zero install friction -- download one file, run it. |
| 2 | **Model weights embedded in binary** | `include_bytes!` with float16 potion-retrieval-32M model (file: potion-code-16M.safetensors, ~32 MB). No internet required, works in sandboxes and air-gapped environments. |
| 3 | **Pure Rust Model2Vec inference** | No ONNX Runtime dependency. Inference is tokenize + lookup + mean pool + normalize (~50 lines). ONNX Runtime dropped x86_64 macOS support; pure Rust works everywhere. |
| 4 | **JSON output by default** | Agents parse structured data, not column-aligned text. `--plain` flag for human fallback. Errors in stdout, never stderr. |
| 5 | **Tree-sitter for structural code parsing** | Powers chunking, --snap, --skeleton, --outline, syntax validation, structural search. Import extraction uses per-language regex (7 languages). No LSP server required. |
| 6 | **Token budgets, not truncation** | `--budget N` returns the best N tokens of results, ranked by relevance. Not `head -N` arbitrary cutoff. |
| 7 | **Dry-run edits by default** | `prx edit` previews changes. `--apply` commits. Agents see what will change before it happens. |
| 8 | **Content hashes in every response** | Enables cheap "has this changed?" checks. Eliminates ~50% of redundant file re-reads. |
| 9 | **No daemon for basic usage** | All commands work statelessly. Optional `prx index --watch` for warm caching. |
| 10 | **6-stage reranking pipeline** | Definition boost, stem matching, file coherence, import graph proximity, noise penalties, saturation decay. Quality comes from ranking, not just retrieval. |
| 11 | **BM25 with compound identifier tokenization** | camelCase/snake_case splitting without stemming. Code identifiers are semantically distinct -- "HTTPResponse" and "HTTP" mean different things. |
| 12 | **RRF fusion with adaptive alpha** | Symbol queries (Foo::bar) lean BM25 (alpha=0.3). Natural language queries stay balanced (alpha=0.5). Auto-detected. |

---

## Anti-Patterns

### 1. Returning unstructured text

All output must go through the JSON envelope in `src/output.rs`. Never
`println!()` directly to stdout from command handlers.

### 2. Using stderr for errors

Agents read stdout. Errors are structured JSON on stdout with a non-zero
exit code. stderr is reserved for debug logging only (behind `RUST_LOG`).

### 3. Unbounded output

Every command that returns file content or search results must respect
`--budget`. Default is unlimited, but the infrastructure must support it.

### 4. Platform-specific behavior

No `#[cfg(target_os)]` in command logic. Platform differences are isolated
to `src/parsing/languages.rs` (grammar loading) and the notify crate
(file watching). Everything else is pure cross-platform Rust.

### 5. Unwrap in library code

`unwrap()` and `expect()` are forbidden outside `#[cfg(test)]` modules.
Use `?` propagation with typed errors.

### 6. Adding dependencies without justification

Every crate added to Cargo.toml must have a comment explaining why it is
needed and why an existing dependency cannot serve the purpose.

---

## Git Workflow

**No direct pushes to `main`.** All work happens on `dev/vX.Y.Z` branches.

**Version semantics:** `v0.X.0` = features (new capabilities). `v0.X.Y` = fixes/improvements only.

```
git checkout -b dev/v0.4.1 main   # cut branch
# ... develop, commit, test ...
# >>> GET HUMAN SIGN-OFF <<<       # mandatory before merge
git checkout main && git merge --no-ff dev/v0.4.1   # merge
git tag -a v0.4.1 -m "..."        # tag
git push origin main && git push origin v0.4.1      # push + release
git branch -d dev/v0.4.1          # cleanup
```

Full workflow: `docs/design/GIT-WORKFLOW.md`.

---

## Pre-Merge Checklist

- [ ] On a `dev/vX.Y.Z` branch (not `main`)
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] `cargo deny check` passes
- [ ] `cargo build --release` succeeds
- [ ] No `unwrap()` in non-test code
- [ ] Public functions have `///` doc comments
- [ ] JSON output matches schemas in docs/design/OUTPUT.md
- [ ] AGENTS.md updated if layout or conventions changed
- [ ] CHANGELOG.md updated for user-visible changes (used as GitHub release notes)
- [ ] Cargo.toml version bumped
