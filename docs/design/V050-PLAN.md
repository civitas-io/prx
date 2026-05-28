# v0.5.0 Execution Plan

## Classification

### Features (v0.5.0 — new capabilities)

| Item | What it enables | Effort | Dependencies |
|---|---|---|---|
| `prx run --auto-json` | Auto-inject `--json` flags for tools with structured output | 1 day | None |
| Tree-sitter import extraction | Accurate imports for multi-line/aliased/re-export/dynamic forms | 3-4 days | None |
| Import language coverage | Extend imports to all 15 registered grammars | 2 days | Tree-sitter imports |
| Additional grammars | Kotlin, Swift, C#, PHP, Elixir | 2-3 days | None |

### Improvements (v0.5.1+ — fixes to existing)

| Item | What it fixes | Effort | Dependencies |
|---|---|---|---|
| Self-contained build (`build.rs`) | `cargo build` works without `make models` or Python | 2-3 days | None |
| Migrate off bincode | Replace unmaintained dep (RUSTSEC-2025-0141) | 1 day | None |

### Distribution (v0.5.1+ — packaging, not features)

| Item | What it enables | Effort | Dependencies |
|---|---|---|---|
| `cargo publish` | `cargo install prx` | 0.5 day | build.rs |
| Homebrew formula | `brew install civitas-io/tap/prx` | 0.5 day | cargo publish |
| npm wrapper | `npx prx` | 0.5 day | cargo publish |
| pip wrapper | `pip install prx` | 0.5 day | cargo publish |

## Proposed Release Sequence

### v0.5.0 — Features

New user-facing capabilities only:

1. **`prx run --auto-json`** (~1 day)
   - Add `--auto-json` flag to `RunArgs`
   - In `execute()`, detect tools that support JSON output and inject
     the appropriate flag (`-o json`, `--format json`, `--json`, etc.)
   - Map: kubectl→`-o json`, terraform→`-json`, npm→`--json`,
     eslint→`--format json`, mypy→`--output json`
   - Existing JSON detection in parsers handles the output side
   - Tests: detection + injection for each tool

2. **Tree-sitter import extraction** (~3-4 days)
   - Replace regex in `parsing/imports.rs` with tree-sitter AST queries
   - Per-language query for import nodes:
     - Rust: `use_declaration`, `mod_item`, `extern_crate_declaration`
     - Python: `import_statement`, `import_from_statement`
     - JS/TS: `import_statement`, `call_expression[callee=import]`
     - Go: `import_declaration`
     - Java: `import_declaration`
     - C/C++: `preproc_include`
     - Ruby: `call[method=require]`
   - Captures multi-line, aliased, re-export, dynamic `import()` forms
   - Existing tests updated + new fixtures for complex patterns
   - graph.rs resolution unchanged (already improved in v0.4.3)

3. **Import language coverage** (~2 days, after tree-sitter imports)
   - Add import queries for remaining grammars:
     - Bash: `source`, `.` commands
     - CSS: `@import` rules
     - HTML: `<script src>`, `<link href>`
   - JSON has no imports (document as intentionally edge-less)
   - Tests for each new language

4. **Additional grammars** (~2-3 days)
   - Add tree-sitter grammars: Kotlin, Swift, C#, PHP, Elixir
   - Add to `parsing/languages.rs` grammar registry
   - Add import extraction queries for each
   - Add outline extraction support for each
   - Tests: outline + import for each language

### v0.5.1 — Build & Security Improvements

No new features. Fix build and security issues:

1. **Self-contained build (`build.rs`)** (~2-3 days)
   - Move model download from `scripts/download-models.sh` into `build.rs`
   - Pin artifacts by SHA-256; fail build with clear message on mismatch
   - Do F16 conversion in Rust using `half` crate (already a dependency)
   - Cache in `OUT_DIR` so repeat builds don't re-download
   - Env var `PRX_MODELS_DIR` for offline/air-gapped builds
   - Remove Python build dependency entirely
   - Update README, CONTRIBUTING.md build instructions

2. **Migrate off bincode** (~1 day)
   - Replace `bincode` with `postcard` for all index serialization:
     chunks.bin, bm25.bin, symbols.bin, imports.bin, embedding_hashes.bin
   - Add `postcard` to Cargo.toml with justification comment
   - Remove `bincode` from Cargo.toml
   - Remove RUSTSEC-2025-0141 ignore from deny.toml
   - Version bump in IndexMeta forces re-index (intentional)
   - Tests: roundtrip serialization for all formats

### v0.5.2 — Distribution

No features, no fixes. Packaging only:

1. **`cargo publish`** (~0.5 day)
   - Verify `cargo package` succeeds
   - Publish to crates.io
   - Test `cargo install prx` on a clean machine

2. **Homebrew formula** (~0.5 day)
   - Create `civitas-io/homebrew-tap` repo
   - Write formula pointing to GitHub release binaries
   - Test `brew install civitas-io/tap/prx`

3. **npm wrapper** (~0.5 day, optional)
   - Thin npm package that downloads the platform binary
   - `npx prx search "query" src/`

4. **pip wrapper** (~0.5 day, optional)
   - Thin pip package with platform binary download
   - `pip install prx && prx search "query" src/`

## Critical Path

```
v0.5.0 features:
  --auto-json ──────────────────────────────────► v0.5.0
  tree-sitter imports ─► import coverage ────────► v0.5.0
  additional grammars ───────────────────────────► v0.5.0

v0.5.1 improvements (can start in parallel):
  build.rs ──────────────────────────────────────► v0.5.1
  bincode migration ─────────────────────────────► v0.5.1

v0.5.2 distribution (sequential, blocked on build.rs):
  build.rs ─► cargo publish ─► homebrew/npm/pip ─► v0.5.2
```

## Estimated Timeline

| Release | Effort | Calendar |
|---|---|---|
| v0.5.0 | ~8-10 days development | 1-2 weeks |
| v0.5.1 | ~3-4 days development | 1 week |
| v0.5.2 | ~2 days development | 2-3 days |

## What's NOT in v0.5.x

Deferred to v0.6.0+:
- Public benchmark suite (CI NDCG regression gate)
- Cross-encoder reranker
- Full symbol graph (call graph, inheritance)
- Bayesian mode predictor
- Information bottleneck filter
- Custom embeddings
