# prx — Completion & Correctness Issues (code-verified)

> Status: findings from a direct read of the source tree at the current `main`
> (Cargo `version = 0.4.0`). Every claim below cites a concrete file/line. These
> are **not** taken from the docs — where docs and code disagree, that disagreement
> is itself logged as a fix (see Issue 5).
>
> **Verification caveat:** there is no Rust toolchain available in the environment
> where this review was done, and network access was restricted, so the code was
> **not compiled or test-run**. Findings are from static reading of source +
> tests. The single most valuable thing to confirm first is **Issue 1** (does a
> fresh `git clone` actually build?), because nothing else matters if it doesn't.

---

## Priority summary

| # | Issue | Severity | Area | Blocks downstream use? |
|---|---|---|---|---|
| 1 | Fresh clone cannot build; model weights gitignored + fetched at build time; silent failure modes | **P1 — blocker** | build / packaging | Yes — can't build reliably |
| 2 | Import graph uses regex extraction + lossy resolution; under-links real codebases | **P1 — quality** | `parsing/imports.rs`, `search/graph.rs` | Yes — degrades `context`/`impact` |
| 3 | `index` rebuilds graph/BM25/embeddings fully every run; `build_partial` exists but is never called; silent zero-embeddings | **P2** | `commands/index.rs`, `index/persist.rs` | Partially — perf + silent degradation |
| 4 | `is_valid` does not detect newly-added files → stale "up_to_date" | **P2 — correctness trap** | `index/persist.rs` | Yes for agent loops that create files |
| 5 | Doc/architecture inconsistencies (claims "tree-sitter for all structural awareness"; build prereq undocumented; `context`/`impact` absent from arch docs) | **P3 — docs** | `docs/`, `AGENTS.md` | No, but misleads contributors |
| 6 | Not published to crates.io; install = build-from-source (compounds Issue 1) | **P3 — distribution** | release | No |
| 7 | Import-extraction language coverage (7) lags parser coverage (15) | **P3 — coverage** | `parsing/imports.rs` | Only for polyglot repos |

---

## Issue 1 — A fresh clone cannot be built without an undocumented network+Python step (P1, blocker)

### WHAT
The embedding model and tokenizers are compiled **into the binary** at build time
via `include_bytes!`, but the files they point at are **gitignored** and only
exist after running a separate download script.

- `src/index/dense.rs:107` — `include_bytes!("../../models/potion-code-16M.safetensors")`
- `src/index/dense.rs:6` — `include_bytes!("../../models/model2vec_tokenizer.json")`
- `src/tokens.rs:5` — `include_bytes!("../models/cl100k_base.json")`
- `.gitignore` — `models/` is excluded ("too large for git, downloaded via `make models`")
- `scripts/download-models.sh` — `curl`s three files from Hugging Face, then runs an
  **inline `python3`** script to convert the safetensors `embeddings` tensor F32→F16.
- `Makefile` — `models:` target just calls the script.

So the real build chain is: `git clone` → `make models` (needs network **and**
`python3`) → `cargo build`. None of `cargo build`/`cargo install` does this for you.

### WHY this is a problem
1. **`cargo install prx` (once published) or `cargo build` on a clean clone fails**
   with a confusing compile-time error: `include_bytes!` points at a file that does
   not exist, so the failure is "file not found" deep in a macro, not a friendly
   "run `make models` first."
2. **Hidden runtime dependency on the build env**: the F16 conversion requires
   Python at build time. A Rust project that needs Python to compile is a surprising
   and brittle prerequisite for contributors and CI on minimal images.
3. **External URL fragility**: build correctness depends on three Hugging Face URLs
   (`minishlab/potion-code-16M`, `Xenova/gpt-4`) staying live and stable. If they
   move/rename, every fresh build breaks with no pinned fallback and no checksum.
4. **Silent runtime degradation when the model is absent/corrupt.** Even if the
   build somehow links a bad/empty model, `index/persist.rs:224`
   (`let Some(mut model) = crate::index::dense::load_model() else { return 0 };`)
   makes indexing **silently produce zero embeddings** and write `embeddings_dim: 0`.
   `load_embeddings` (`index/persist.rs:243`) then returns `None`, and search quietly
   falls back to BM25-only — **no error surfaced to the user**. Search quality
   silently halves and nothing says why.

### HOW to fix (proposed)
Pick one of these, in rough order of preference:

- **(Preferred) `build.rs` that fetches + verifies + caches.** Move the download
  logic out of the Makefile into a `build.rs` so `cargo build` is self-contained.
  - Pin each artifact by **SHA-256**; fail the build loudly with a clear message if
    the hash mismatches.
  - Cache into `OUT_DIR` (or a versioned cache dir) so repeat builds don't re-download.
  - Do the F16 conversion **in Rust** (the `half` crate is already a dependency —
    `Cargo.toml`), eliminating the Python build dependency entirely.
  - Provide an offline escape hatch: env var `PRX_MODELS_DIR` pointing at
    pre-downloaded files, so air-gapped/CI builds can supply weights without network.
- **(Alternative) Vendor weights via Git LFS.** Simpler, but bloats clones and many
  CI caches/mirrors handle LFS poorly; less preferred.
- **Regardless of the above, fix the silent-failure path.** `load_model()` returning
  `None` should be distinguishable between "model intentionally absent" and "model
  load failed." At minimum, when `index` computes zero embeddings because the model
  failed to load, emit a structured warning into the JSON envelope (and the
  `~/.prx/errors.jsonl` fallback log that already exists per the v0.1.1 changelog)
  rather than silently writing `embeddings_dim: 0`.

### Acceptance criteria
- `git clone … && cargo build --release` succeeds on a clean machine **with no manual
  `make models` step** (network allowed) and on an offline machine when
  `PRX_MODELS_DIR` is set.
- Build fails with a clear, actionable message if a weight artifact is missing or its
  checksum doesn't match.
- No `python3` requirement to build.
- When embeddings can't be produced at index time, the user sees an explicit warning;
  search does not silently degrade with no signal.

---

## Issue 2 — Import graph extraction is regex-based with lossy resolution; it under-links real repos (P1, quality)

This is the issue that most affects the agent-orchestration use case, because
`prx context` and `prx impact` are built directly on the persisted import graph.

### WHAT
**Extraction is regex, not tree-sitter** — despite tree-sitter being available and
used everywhere else:

- `src/parsing/imports.rs:3` — `use regex::Regex;`
- `imports.rs:17` `extract_imports(source, ext)` dispatches per-extension to
  hand-written regexes, e.g. Go: `^\s*"([^"]+)"`, Java:
  `^\s*import\s+(?:static\s+)?([\w.]+)\s*;`, plus bespoke `extract_python`/`extract_js`.
- Only **7 language families** are handled: `rs`, `py`, `js/jsx/ts/tsx/mjs/cjs`, `go`,
  `java`, `c/h/cpp/...`, `rb`. Everything else hits `_ => vec![]` (`imports.rs:36`).

**Resolution is suffix-match with a hard give-up** — `src/search/graph.rs`:

- `resolve_import` (`graph.rs:219`) normalizes the import string
  (`normalize_import`, `graph.rs:200`: `::`→`/`, strip `./ ../ crate/ self/ super/`,
  `.`→`/`) and looks it up in a suffix index.
- **It bails to `vec![]` (no edge) whenever a name matches more than 3 files:**
  `if ids.len() <= 3 { return ids.clone(); }` — both at the full-path attempt and the
  trimmed-parent attempt. If `ids.len() > 3`, the import resolves to **nothing**.
- `build_path_index` (`graph.rs:172`) indexes every path suffix by basename, so common
  filenames collide heavily.

### WHY this is a problem
1. **Regex misses real import forms**: re-exports, dynamic `import()`, conditional/
   aliased imports, multi-line import blocks, `export … from`, barrel files, etc.
   These are common in TS/JS and Python codebases and produce **no edge**.
2. **The `<= 3` bail-out silently drops edges exactly where repos are biggest.**
   Real projects have many `index.ts`, `utils.py`, `mod.rs`, `__init__.py`,
   `types.ts`. Any import resolving to such a basename across >3 files yields **zero**
   edges — so the graph gets *sparser* as the repo grows, which is backwards.
3. **Direct downstream impact on the tools the orchestration layer depends on.**
   `prx impact` walks `reverse` edges; missing edges mean it reports
   "nothing depends on this file" when things actually do — a dangerous false
   negative for change-impact analysis. `prx context`'s 1-hop import neighborhood is
   correspondingly thin, so the assembled context package misses relevant files.
4. **Inconsistent with the project's own stated principle** ("tree-sitter for all
   structural awareness", `AGENTS.md:422`) — see Issue 5.

### HOW to fix (proposed)
- **Replace regex extraction with tree-sitter queries.** The grammars are already
  compiled in (`parsing/languages.rs:5-19`). Add per-language import-node queries
  (e.g. `import_declaration`, `use_declaration`, `import_from_statement`,
  `call_expression` for dynamic `import()`), mirroring how `outline.rs` already uses
  tree-sitter queries. This captures multi-line/aliased/re-export forms the regex can't.
- **Relax / replace the `<= 3` bail-out with disambiguation instead of surrender.**
  When a name matches multiple files, rank candidates by:
  1. directory proximity to the importing file (same dir > sibling > ancestor),
  2. language-aware path conventions (Python package dirs, Rust `mod.rs`/`lib.rs`,
     JS/TS index resolution and `tsconfig`/`jsconfig` `paths` if cheaply available),
  3. honoring relative-path prefixes (`./`, `../`) as hard constraints rather than
     stripping them in `normalize_import` (`graph.rs:201-205` currently discards the
     directionality that would disambiguate).
  Keep a cap to avoid fan-out explosions, but prefer "best 1–2 candidates" over
  "give up at >3".
- **Add edge-confidence metadata** (optional, but useful for the orchestrator): mark
  edges as resolved-exact vs resolved-heuristic so `impact`/`context` can expose
  confidence.
- **Add fixtures from real-world structures** to the test suite: collisions on
  `index.ts`/`utils.py`/`mod.rs`, re-exports, aliased imports, dynamic imports.
  Existing tests (`graph.rs:258+`, `imports.rs:106+`) only cover simple happy paths.

### Acceptance criteria
- Import extraction is driven by tree-sitter for all currently-supported languages,
  with tests for multi-line/aliased/re-export/dynamic forms.
- A name that collides across >3 files resolves to the most likely candidate(s) using
  proximity/convention, not to an empty set.
- New fixtures demonstrate `impact` correctly finding dependents through common
  filenames and re-exports.
- (If confidence metadata added) `impact`/`context` output distinguishes exact vs
  heuristic edges.

---

## Issue 3 — `index` rebuilds derived structures fully on every run; the incremental graph path exists but is dead code (P2)

### WHAT
`index` advertises incrementality, and **chunk reuse is genuinely incremental** —
`index/persist.rs:123-150` hashes each file (xxh3) against `meta.file_hashes` and
reuses old chunks for unchanged files (well covered:
`incremental_skips_unchanged_files`, `incremental_rechunks_changed_file`,
`incremental_handles_new_file`, `incremental_handles_deleted_file`,
`persist.rs:419-471`).

**But three of the four derived artifacts are full rebuilds every run, regardless of
what changed:**

- **BM25**: `persist.rs:156` comment — "BM25 index is global — always rebuild from all
  chunks"; `enriched_texts` recomputed over all chunks (`persist.rs:157-160`).
- **Embeddings**: `compute_and_save_embeddings` (`persist.rs:223`) re-embeds **every**
  chunk on every index, changed or not.
- **Import graph**: `persist.rs:201` always calls `ImportGraph::build_full(...)`,
  re-reading and re-parsing **every** file's imports each run.

Meanwhile, **`ImportGraph::build_partial` exists** (`search/graph.rs:53`) and has a
test (`graph.rs:365`), but **nothing in the codebase calls it** — `index` never uses
it. It's effectively dead code.

### WHY this is a problem
- For an agent loop that re-indexes after every task (the implementer edits 1–2 files
  per task), the per-task index cost is dominated by **re-embedding the entire repo**
  and re-parsing every import, even though almost nothing changed. On a large repo
  this turns a cheap incremental update into an expensive full pass.
- Combined with Issue 1's silent zero-embeddings path, a model-load hiccup during one
  of these frequent full re-embeds silently wipes embedding quality for the whole repo.
- The presence of an unused `build_partial` suggests an intended-but-unfinished
  integration — a real, narrow completion gap rather than a design choice.

### HOW to fix (proposed)
- **Wire `index` to `build_partial`** for the import graph: only re-extract imports for
  files whose hash changed (the changed-set is already computed in the chunk loop),
  patch their forward edges, and recompute `reverse` from the patched forward set
  (`build_reverse`, `graph.rs:241`). Persist the updated graph.
- **Make embeddings incremental**: persist a per-chunk content hash alongside
  `embeddings.bin`; only embed new/changed chunks and copy the rest from the previous
  `embeddings.bin`. (Embeddings are per-chunk and order-stable, so this is tractable.)
- **BM25**: BM25 statistics (IDF) are corpus-global, so a full rebuild of scores is
  defensible — but the *enriched text* generation can reuse unchanged chunks. At
  minimum, document why BM25 stays global so it doesn't read as an oversight.
- If full rebuild is intentionally kept for some artifact, **say so explicitly** in
  both the comment and the design doc, and explain the tradeoff.

### Acceptance criteria
- A re-index after changing 1 file in an N-file repo re-embeds ~1 file's chunks, not N.
- `index` updates the import graph via `build_partial` (or `build_partial` is removed
  if truly not wanted — no dead code).
- Behavior is covered by a test asserting embedding/graph work is proportional to the
  changed set, not the repo size.

---

## Issue 4 — `is_valid` ignores newly-added files, so `index` reports "up_to_date" when it isn't (P2, correctness trap)

### WHAT
`is_valid` (`index/persist.rs:313-344`) iterates **only over `meta.file_hashes`**
(the files known at last index) and checks each still matches. It never walks the tree
to detect files that exist now but weren't indexed before. The `index` command
short-circuits on it: `commands/index.rs:74` —
`if !args.rebuild && persist::is_valid(root) { return … "up_to_date" … }`.

### WHY this is a problem
- In an agent loop that **creates** files (very common — new modules, new tests),
  running `prx index` (without `--rebuild`) after a task that added a file returns
  `"up_to_date"` and does nothing, because every *previously known* file still matches.
  The new file is invisible to search/graph until a forced `--rebuild` or a `--watch`
  filesystem event happens to catch it.
- This is a silent staleness bug: the agent believes the index reflects the working
  tree when it doesn't.

### HOW to fix (proposed)
- In `is_valid`, after verifying known files, **walk the tree** (reuse
  `walk::walk(root, &WalkOpts::default())`, already used by `build_and_save`) and
  return `false` if the current indexable-file set differs from `meta.file_hashes`'
  key set (new files added, or tracked files deleted).
- Cheap optimization: store and compare a count + a combined hash of the sorted path
  list in `meta` to detect set changes without re-hashing contents.

### Acceptance criteria
- After adding a new indexable file, `prx index` (no `--rebuild`) detects it and
  re-indexes rather than reporting `"up_to_date"`.
- Existing incremental tests still pass; add a test:
  `is_invalid_after_new_file_added`.

---

## Issue 5 — Documentation / architecture inconsistencies (P3, docs)

The design docs are mostly accurate, but a few statements now contradict the code and
should be corrected so contributors aren't misled.

### WHAT / WHY (each is a doc edit)
1. **"Tree-sitter for all structural awareness"** — `AGENTS.md:422` states this as a
   core design principle, but the import graph (the structural feature behind
   `context`/`impact`) is **regex-based** (Issue 2). Either fix the code (Issue 2) and
   keep the claim, or soften the claim until then. As written it's misleading.
   - Note `AGENTS.md:378` is **correct and honest** ("Per-language regex import
     extraction (7 languages)") — so the doc contradicts *itself*. Reconcile the two.
2. **Build prerequisite undocumented.** `docs/research/PLATFORM.md:74` says "Model is
   embedded via `include_bytes!`, no download needed at runtime" — true at runtime, but
   it omits that weights **must be downloaded at build time** via `make models`
   (Issue 1). No doc states "run `make models` before `cargo build`." Add an explicit
   Build-from-source prerequisite section to the README and `docs/CONTRIBUTING.md`.
3. **`context` and `impact` are absent from the architecture docs.** They're the two
   newest and (for agent use) most important commands, present in code
   (`commands/context.rs` ~1062 lines, `commands/impact.rs` ~898 lines) and in the
   CHANGELOG, but `docs/architecture/SYSTEM.md` and `docs/design/SYSTEM-DESIGN.md` don't
   describe them or the import-graph subsystem they depend on. Add an architecture
   section covering the import graph (extraction → resolution → persistence →
   `context`/`impact` consumption).
4. **Graph proximity / `imports.bin` underdocumented in SEARCH.md.** The ranking boost
   from the import graph (`ranking/proximity.rs`) and the `imports.bin` artifact are
   only mentioned in passing; SEARCH.md's pipeline description should include the graph
   stage and its limitations.

### HOW
Make the four edits above. Prefer fixing code first (Issues 1–2) and then aligning
docs to the fixed behavior, so the docs don't promise a future state.

---

## Issue 6 — Not published to crates.io; install is build-from-source (P3, distribution)

### WHAT
The roadmap lists `cargo publish` and a Homebrew formula as "Planned." Today the only
install path is build-from-source, which means every user hits Issue 1.

### WHY
Distribution friction compounds the build blocker: there's no `cargo install prx` or
`brew install` that "just works," so adoption (and your own orchestrator's
provisioning) requires the full toolchain + model-download dance.

### HOW
- Land Issue 1 first (self-contained build), **then** `cargo publish`. Publishing while
  the build needs `make models` would ship a crate that fails to compile for everyone.
- Consider shipping prebuilt binaries with the model already embedded via the existing
  GitHub Actions release pipeline (5 targets per roadmap) as the primary install path,
  with crates.io as secondary.

### Acceptance criteria
- `cargo install prx` works end-to-end (depends on Issue 1).
- Release artifacts are documented as the recommended install in the README.

---

## Issue 7 — Import-extraction language coverage (7) lags parser coverage (15) (P3, coverage)

### WHAT
`parsing/languages.rs:5-19` registers grammars for ~15 extensions (rs, py, js, ts, tsx,
jsx, go, java, c, cpp, rb, sh/bash, json, html, css). But `parsing/imports.rs:17-36`
only extracts imports for 7 of them. Files in unsupported-for-imports languages are
chunked/searched fine but contribute **zero** graph edges.

### WHY
For mainstream Rust/Python/TS/Go repos this is a non-issue. For polyglot repos (e.g.
shell + config + web assets, or languages like C# / Kotlin / PHP the roadmap plans to
add grammars for), the import graph — and therefore `impact`/`context` — has blind
spots with no warning.

### HOW
- After Issue 2 moves extraction to tree-sitter, add import queries for the remaining
  registered languages where the concept exists.
- For languages with no meaningful import concept (json/html/css), document that they
  are intentionally edge-less so it's not mistaken for a bug.

### Acceptance criteria
- Import extraction covers all registered languages that have an import/include concept.
- Intentionally-unsupported languages are documented as such.

---

## Suggested sequencing for the coding agent

1. **Issue 1** (build-from-clone) — unblocks everything; verify a clean build first.
2. **Issue 2** (tree-sitter imports + resolution) — the core quality fix for
   `context`/`impact`.
3. **Issue 4** (new-file detection in `is_valid`) — small, high-value correctness fix.
4. **Issue 3** (incremental graph/embeddings; wire `build_partial`) — perf + dead-code.
5. **Issue 5** (docs) — align docs to the now-fixed behavior.
6. **Issues 6 & 7** — distribution and coverage, once the above land.

Each issue above lists acceptance criteria; treat them as the definition of done.

