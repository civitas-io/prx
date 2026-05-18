# ag Roadmap

A phased delivery plan from zero to v0.1.0 public release.

---

## Phase 0 — Foundation (Weeks 1-2)

| Deliverable | Notes |
|---|---|
| Project scaffold | Cargo workspace, CI, clippy/fmt configs |
| Tree-sitter integration | Chunking, AST parsing, 15 language grammars |
| Model2Vec inference | Pure Rust, safetensors + ndarray |
| BM25 implementation | Compound identifier tokenization, sparse matrix scoring |
| JSON/JSONL output framework | Shared output layer for all tools |
| Token counting | tokenizers crate |

**Milestone:** `prx search --literal "pattern" src/` works end-to-end.

---

## Phase 1 — Core Tools (Weeks 3-5)

### `prx search`
- Literal, semantic, and structural modes
- RRF fusion across modes
- Full reranking pipeline: definition boost 3x, stem matching, file coherence 0.2x, noise penalties, saturation decay 0.5^n
- `--budget` and `--context function|class|block`

### `prx read`
- `--lines`, `--snap function|class`, `--skeleton`, `--outline`, `--hash`, `--budget`
- Inline metadata in output

### `prx find`
- Dual tree + flat output
- `--pattern`, `--depth`, `--related-to`, `--changed-since`, `--outline`
- .gitignore-aware

### `prx exists`
- Bloom filter O(1) presence check

### `prx outline`
- Standalone symbol outline

**Milestone:** Search, Read, and Find work end-to-end with full JSON output.

---

## Phase 2 — Edit, Diff, Integration (Weeks 6-8)

### `prx edit`
- Literal match by default, `--regex` opt-in
- `--dry-run` default, `--apply` to commit
- `--in-function` scoping, syntax validation, multi-edit batching

### `prx diff`
- Semantic summary, function-level attribution
- `--stat-only`, `--budget`, move detection
- No ANSI codes in output

### `prx mcp`
- MCP server over stdio exposing all tools (rmcp crate)

### `prx index`
- Optional persistent index with `--watch` mode (notify crate)

### `prx batch`
- JSONL batch execution

### `prx stats`
- Token savings dashboard

**Milestone:** Full tool suite works. MCP integration tested with Claude Code, Cursor, and OpenCode.

---

## Phase 3 — Polish, Benchmark, Release (Weeks 9-12)

| Area | Details |
|---|---|
| Benchmarks | NDCG@10 vs Semble/ripgrep/CodeRankEmbed, token savings, latency profiling |
| Cross-platform CI | Linux x86_64 + aarch64, macOS x86_64 + aarch64, Windows x86_64 |
| Binary optimization | LTO, strip, float16 model |
| Documentation | Man pages, --help text, usage examples |
| Agent integration guides | AGENTS.md for Claude Code, Cursor, Codex, OpenCode |
| Distribution | Homebrew formula, cargo install, GitHub releases with prebuilt binaries |

**Milestone:** v0.1.0 public release.

---

## Future (post v0.1.0)

| Tool | Purpose |
|---|---|
| `prx context` | Assemble context packages ("everything about module X") |
| `prx impact` | Static call graph analysis ("what breaks if I change X?") |
| `prx deps` | Import and dependency graph |
| `prx blame` | Structured git blame per function |
| `prx test` | Test discovery related to functions and files |
| Additional grammars | Expand language coverage beyond initial 15 |
| Custom embeddings | Support for custom or fine-tuned embedding models |
| Incremental indexing | Index persistence and incremental updates |

---

## Version Compatibility

As tools evolve between versions, CLI flags and JSON output schemas may change. All breaking changes will be documented in CHANGELOG.md with migration guides. JSON output includes a `version` field for programmatic detection.
