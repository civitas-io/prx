# prx Roadmap

## v0.1.0 — RELEASED

All phases complete. Released at https://github.com/civitas-io/prx/releases/tag/v0.1.0

### Phase 0 — Foundation [DONE]

| Deliverable | Status |
|---|---|
| Project scaffold (Cargo, CI, clippy/fmt) | Done |
| Tree-sitter integration (14 grammars, chunking, AST parsing) | Done |
| Model2Vec inference (pure Rust, safetensors + ndarray, float16) | Done |
| BM25 implementation (compound identifier tokenization, CSC sparse matrix) | Done |
| JSON/JSONL output framework | Done |
| Token counting (cl100k_base, fast + exact modes) | Done |
| Content hashing (xxh3) | Done |
| File walking (ignore crate, .prxignore) | Done |

### Phase 1 — Core Tools [DONE]

| Command | Status |
|---|---|
| `prx search` (literal + semantic + structural, RRF fusion, 5-stage reranking) | Done |
| `prx read` (--lines, --snap, --skeleton, --outline, --hash, --budget) | Done |
| `prx find` (tree+flat, --pattern, --depth, --changed-since, --related-to) | Done |
| `prx exists` (bloom filter O(1)) | Done |
| `prx outline` (file + directory mode) | Done |
| Search auto-detection (literal vs semantic vs structural) | Done |
| Continuation tokens for pagination | Done |
| Budget enforcement | Done |

### Phase 2 — Edit, Diff, Integration [DONE]

| Command | Status |
|---|---|
| `prx edit` (literal/regex, dry-run, --apply, --in-function, syntax validation) | Done |
| `prx diff` (git diff, function attribution, semantic notes, --stat-only) | Done |
| `prx run` (9 parsers: cargo test/build/clippy, pytest, go test, jest/vitest, tsc, eslint) | Done |
| `prx index` (persistent to .prx/index/, --rebuild, --stats, --watch) | Done |
| `prx batch` (JSONL stdin dispatch) | Done |
| `prx stats` (token savings dashboard, PRX_STATS_FILE env) | Done |
| `prx init` (AGENTS.md snippet, cursor/codex/opencode/claude-code configs) | Done |
| `prx mcp` (MCP server over stdio, 6 tools) | Done |

### Phase 3 — Polish, Benchmark, Release [DONE]

| Area | Status |
|---|---|
| Cross-platform CI (Linux, macOS, Windows) | Done |
| Float16 model conversion (77MB -> 48MB binary) | Done |
| Model2Vec vocabulary loading (real tokenizer, 61,826 tokens) | Done |
| GitHub Actions release pipeline (5 targets) | Done |
| Apache 2.0 license | Done |
| Documentation (21 docs, ~5,000 lines) | Done |
| 300 tests (256 unit + 44 E2E), 84% coverage | Done |

---

## v0.1.0 Stats

| Metric | Value |
|---|---|
| Commands | 13 |
| Tests | 300 |
| Coverage | 84% |
| Languages | 14 (tree-sitter grammars) |
| Release binary | ~48 MB |
| Tool parsers (prx run) | 9 |
| Repository | https://github.com/civitas-io/prx |

---

## v0.2.0 — Next

| Item | Priority | Description |
|---|---|---|
| Benchmarks | High | NDCG@10 measurement, token efficiency curves, latency profiling |
| `cargo publish` | High | Publish to crates.io for `cargo install prx` |
| Homebrew formula | High | `brew install civitas-io/tap/prx` |
| More run parsers | Medium | bun test, deno test, dotnet test, ruff |
| Additional grammars | Medium | Kotlin, Swift, C#, PHP, Elixir |
| Float16 native inference | Low | f16 math without f32 conversion |

## Future (post v0.2.0)

| Tool | Purpose |
|---|---|
| `prx context` | Assemble context packages ("everything about module X") |
| `prx impact` | Static call graph analysis ("what breaks if I change X?") |
| `prx deps` | Import and dependency graph |
| `prx blame` | Structured git blame per function |
| `prx test` | Test discovery related to functions and files |
| Custom embeddings | Support for user-provided or fine-tuned models |

---

## Version Compatibility

CLI flags and JSON output schemas may change between minor versions. All breaking
changes are documented in CHANGELOG.md with migration guides. JSON output includes
a `version` field for programmatic detection.
