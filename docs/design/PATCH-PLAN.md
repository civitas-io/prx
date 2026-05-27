# v0.4.x Patch Plan

Fixes from independent code review (`issues.md`). Each patch is a focused
fix with acceptance criteria. Ordered by impact and dependency.

## v0.4.1 — New file detection in `is_valid`

**Problem:** `is_valid()` only checks that previously-indexed files haven't
changed. New files added after indexing are invisible — `prx index` reports
"up_to_date" when the index is stale.

**Root cause:** `persist.rs:331-343` iterates `meta.file_hashes` only.
Never walks the tree to detect files not in the hash map.

**Fix:**
1. In `is_valid()`, after verifying known files, walk the tree via
   `walk::walk(root, &WalkOpts::default())`.
2. Compare the current indexable-file set against `meta.file_hashes` keys.
3. Return `false` if any new file exists that isn't in the hash map, or
   if any previously-indexed file was deleted.
4. Optimization: store `file_count` in `IndexMeta` and compare against
   walk count as a fast pre-check before full path comparison.

**Files to change:**
- `src/index/persist.rs` — `is_valid()` function
- `src/index/persist.rs` — possibly `IndexMeta` struct (add file_count)

**Tests:**
- Add `is_invalid_after_new_file_added` test
- Existing incremental tests must still pass

**Acceptance:** `prx index` (no `--rebuild`) detects new files and re-indexes.

---

## v0.4.2 — Silent embedding degradation warning

**Problem:** When `load_model()` returns None during indexing,
`compute_and_save_embeddings` silently returns 0. Search falls back to
BM25-only with no signal to the user.

**Root cause:** `persist.rs:223-226` — `let Some(mut model) = ... else { return 0; }`
with no logging or warning.

**Fix:**
1. When `load_model()` returns None in `compute_and_save_embeddings`,
   log a warning to `~/.prx/errors.jsonl` (the existing error log from
   v0.1.1's fallback system).
2. Return a warning string from `build_and_save` that can be included
   in the JSON output of `prx index`.
3. Add `warnings` field to `IndexStats` struct.
4. Surface the warning in the `prx index` command output JSON.

**Files to change:**
- `src/index/persist.rs` — `compute_and_save_embeddings`, `build_and_save`, `IndexStats`
- `src/commands/index.rs` — surface warnings in output

**Tests:**
- Test that when model is absent, index output includes a warning
- Test that search still works (BM25 fallback) but the index command
  signals degradation

**Acceptance:** When embeddings can't be produced, user sees a warning.
Search does not silently degrade with no signal.

---

## v0.4.3 — Fix import resolution bail-out

**Problem:** `resolve_import` in `graph.rs` bails to `vec![]` (no edge)
when a name matches >3 files. Common filenames (index.ts, utils.py,
mod.rs, __init__.py) trigger this constantly in large repos, making the
graph sparser as repos grow.

**Root cause:** `graph.rs:223,233` — `if ids.len() <= 3 { return ... }`
else falls through to return `vec![]`.

**Fix:**
1. Replace the bail-out with a disambiguation strategy:
   - Rank candidates by directory proximity to the importing file
     (same dir > sibling dir > ancestor).
   - Honor relative-path prefixes (`./`, `../`) as hard constraints
     instead of stripping them in `normalize_import`.
   - Keep top 1-2 candidates instead of giving up entirely.
2. Cap at a reasonable limit (e.g. 5) to prevent fan-out, but prefer
   "best guess" over "no edge".

**Files to change:**
- `src/search/graph.rs` — `resolve_import`, `normalize_import`

**Tests:**
- Test that `index.ts` imported from `src/components/` resolves to
  `src/components/index.ts` not nothing.
- Test that `../utils` resolves to parent directory's utils file.
- Test that ambiguous names with >3 matches still resolve (to closest).
- Add fixtures for common collision patterns.

**Acceptance:** A name matching >3 files resolves to best candidate(s)
using proximity, not to an empty set. `impact` finds dependents through
common filenames.

---

## v0.4.4 — Incremental embeddings

**Problem:** `compute_and_save_embeddings` re-embeds every chunk on every
index run, even when only 1 file changed. On large repos (55k chunks),
this dominates index time (~300s for embeddings vs ~30s for everything else).

**Root cause:** `persist.rs:162` calls `compute_and_save_embeddings` with
all enriched texts, not just changed ones.

**Fix:**
1. Persist a per-chunk content hash alongside `embeddings.bin` (e.g. as
   `embedding_hashes.bin` — a parallel array of xxh3 hashes).
2. On re-index, compare each chunk's enriched text hash against the
   stored hash.
3. Only embed chunks whose hash changed. Copy unchanged embeddings from
   the previous `embeddings.bin`.
4. Rebuild the full embedding matrix from the mixed
   (copied + newly-computed) vectors and save.

**Files to change:**
- `src/index/persist.rs` — `compute_and_save_embeddings`, `build_and_save`
- Possibly `src/index/dense.rs` — if batch embedding API needs to support
  partial chunks

**Tests:**
- Test that re-index after 1 file change re-embeds ~1 file's chunks
- Test that embedding quality is identical between full and incremental
- Measure time: incremental should be proportional to changed files

**Acceptance:** Re-indexing after 1 file change in an N-file repo
re-embeds ~1 file's chunks, not N.

---

## v0.4.5 — Documentation consistency

**Problem:** Several doc claims contradict the code. Contributors and
users are misled.

**Fixes:**
1. `AGENTS.md:422` — soften "Tree-sitter for all structural awareness"
   to "Tree-sitter for structural code parsing (chunking, outline, snap,
   structural search). Import extraction uses per-language regex."
   Reconcile with line 378 which correctly says regex.

2. README + CONTRIBUTING.md — add explicit build prerequisite:
   "Run `make models` before `cargo build`. Requires network access and
   Python 3 for model weight conversion."

3. `docs/architecture/SYSTEM.md` — add section on import graph subsystem
   and how `context`/`impact` consume it.

4. `docs/architecture/SEARCH.md` — add import graph proximity stage to
   the pipeline description.

**Files to change:**
- `AGENTS.md`
- `README.md`
- `docs/CONTRIBUTING.md`
- `docs/architecture/SYSTEM.md`
- `docs/architecture/SEARCH.md`

**Acceptance:** No doc claims contradict the code. Build prereqs are
documented. context/impact appear in architecture docs.

---

## Deferred to v0.5.0

These are larger efforts that belong in a minor release:

| Item | Why deferred |
|---|---|
| `build.rs` for self-contained builds (Issue 1 full) | Requires Rust F16 conversion, download caching, offline escape hatch. Multi-day effort. |
| Tree-sitter import extraction (Issue 2 full) | 200-300 lines per language × 7+ languages. Needs careful per-language AST queries. |
| `cargo publish` (Issue 6) | Blocked on self-contained build (`build.rs`). |
| Import language coverage (Issue 7) | Low ROI until tree-sitter imports land. |
| Wire `build_partial` for index-time graph (Issue 3) | Depends on tree-sitter imports for correctness. Regex extraction + partial graph = compounding inaccuracy. |
