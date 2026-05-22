# Testing Plan

This plan covers unit tests, integration tests, and benchmark tests. Coverage target: >= 80%. Tests live alongside source (unit) or in tests/ (integration).

## Testing Strategy

Three tiers:

- **Unit tests**: inline `#[cfg(test)] mod tests` in each module. Test pure functions, data structures, algorithms.
- **Integration tests**: `tests/integration/`. Test CLI binary end-to-end via `assert_cmd`.
- **Benchmarks**: `benches/`. Performance regression detection via criterion.

## Unit Tests by Module

### src/output.rs

- Test `Envelope` serialization with mock data, verify JSON shape matches OUTPUT.md
- Test `ErrorEnvelope` serialization, verify error code/message/suggestion fields
- Test token count computation (fast mode vs exact mode)
- Test `--plain` mode produces non-JSON output

### src/hash.rs

- Test `hash_bytes` with known input/output pairs
- Test `hash_file` on a temp file
- Test determinism: same input always same hash

### src/tokens.rs

- Test `count_tokens_fast` vs `count_tokens_exact`, verify within 20% on code samples
- Test lazy tokenizer initialization (first call loads, subsequent reuse)
- Test edge cases: empty string, single character, very long string

### src/walk.rs

- Test `.gitignore` respect (create temp dir with `.gitignore`, verify skipped files)
- Test `.prxignore` support (custom ignore patterns)
- Test binary file detection (file with null byte skipped)
- Test `PRX_MAX_FILE_SIZE` enforcement
- Test language detection from file extension

### src/parsing/languages.rs

- Test every supported extension maps to correct `Language`
- Test unknown extension returns `None`

### src/parsing/outline.rs

Test symbol extraction for each language with sample files:

- **Python**: functions, classes, methods, decorators
- **Rust**: `fn`, `struct`, `enum`, `impl`, `trait`
- **JavaScript/TypeScript**: function, class, arrow functions, exports
- **Go**: `func`, `type struct`, interface
- **Java**: class, method, interface
- **C/C++**: function, struct, typedef

Also test:

- Nested symbols (methods inside classes)
- Signature extraction (first line of definition)

### src/parsing/snap.rs

- Test snap to function: given line inside function body, returns function boundaries
- Test snap to class: given line inside method, returns class boundaries
- Test snap at top level: returns file boundaries
- Test snap with nested structures

### src/chunking/treesitter.rs

- Test 3000-char Python file produces 2 chunks, neither splits a function
- Test file under 1500 chars produces 1 chunk
- Test file with no tree-sitter support falls back to line-based chunking
- Test chunk boundaries are contiguous and non-overlapping
- Test chunk line numbers are correct

### src/index/dense.rs

- Test model loading from embedded bytes
- Test `embed_text` produces 256-dim vector
- Test L2 normalization (vector magnitude = 1.0)
- Test cosine similarity: similar code > 0.5, unrelated code < 0.3
- Test empty input returns zero vector

### src/index/sparse.rs

- Test identifier tokenization:
  - `"getHTTPResponse"` -> `["gethttpresponse", "get", "http", "response"]`
  - `"my_func"` -> `["my_func", "my", "func"]`
  - `"simple"` -> `["simple"]`
- Test BM25 scoring: known term in known document gets positive score
- Test CSC matrix construction and column slicing
- Test content enrichment: file stem and dir components appended

### src/search/fusion.rs

- Test RRF score computation: rank 1 -> 1/61, rank 2 -> 1/62
- Test alpha resolution: camelCase query -> 0.3, natural language -> 0.5
- Test fusion merges results from both retrievers correctly
- Test over-fetching: `top_k=5` fetches 25 candidates internally

### src/ranking/boosting.rs

- Test definition boost: chunk with `"fn foo"` gets 3x for query `"foo"`
- Test file stem matching: query `"auth"` boosts chunks in `auth.rs`
- Test file coherence: file with 3 matching chunks gets top chunk boosted

### src/ranking/penalties.rs

- Test noise penalties: test file gets 0.3x, compat dir gets 0.3x, combined 0.09x
- Test saturation decay: 2nd chunk from same file is 0.5x, 3rd is 0.25x
- Test re-export penalty: `__init__.py` gets 0.5x
- Test `.d.ts` penalty: gets 0.7x

### src/index/bloom.rs

- Test insert and check: inserted key found, non-inserted key not found
- Test false positive rate: < 5% on 10000 random strings
- Test bloom filter size is reasonable (~75KB for 50K tokens)

## Integration Tests (tests/integration/)

All integration tests use `assert_cmd` to test the compiled binary. Test fixtures live in `tests/fixtures/` as small sample files in multiple languages.

### test_search.rs

- `test_literal_search`: search for known string, verify match in JSON output
- `test_literal_search_no_match`: search for absent string, verify empty results
- `test_semantic_search`: search for "authentication" in a project with auth code
- `test_structural_search`: search for `"fn $NAME($$$) { $$$ }"` in Rust files
- `test_search_auto_detection`: query `"authenticate("` auto-detects literal mode
- `test_search_budget`: verify total tokens in output <= budget
- `test_search_context_function`: verify snippet expanded to enclosing function
- `test_search_exists`: verify bloom filter returns `{exists: true/false}`
- `test_search_continuation`: first call returns token, second call with `--continue` returns next page
- `test_search_json_schema`: verify output matches OUTPUT.md schema exactly

### test_read.rs

- `test_read_full_file`: read a file, verify content + meta + outline in output
- `test_read_lines`: read lines 10-20, verify exact range
- `test_read_snap_function`: read line inside function, verify expanded to function boundaries
- `test_read_skeleton`: verify only signatures returned, ~10% of full file tokens
- `test_read_outline`: verify symbol table returned, no content
- `test_read_hash_only`: verify only hash returned, minimal tokens
- `test_read_budget`: verify content truncated when exceeding budget
- `test_read_nonexistent`: verify structured error with suggestion

### test_find.rs

- `test_find_all`: find all files, verify tree + flat output
- `test_find_pattern`: find `"*.ts"` files only
- `test_find_depth`: find with `--depth 2`, verify no deeper files
- `test_find_gitignore`: verify `.gitignore`'d files excluded
- `test_find_changed_since`: in a git repo, verify only recently changed files returned
- `test_find_budget`: verify output truncated within budget

### test_edit.rs

- `test_edit_dry_run`: preview changes, verify file NOT modified
- `test_edit_apply`: apply changes, verify file modified and output shows before/after
- `test_edit_literal`: literal match (no regex escaping issues)
- `test_edit_regex`: `--regex` flag with regex pattern
- `test_edit_in_function`: `--in-function` scopes to named function
- `test_edit_syntax_check`: edit that breaks syntax reports `syntax_valid: false`
- `test_edit_no_match`: verify meaningful error when `--find` doesn't match
- `test_edit_hash_changes`: verify hash before != hash after on apply

### test_diff.rs

- `test_diff_working_tree`: make uncommitted change, verify diff output
- `test_diff_since_ref`: verify diff against a specific commit
- `test_diff_stat_only`: verify summary returned in ~30 tokens
- `test_diff_functions`: verify hunks grouped by function
- `test_diff_summary`: verify heuristic summary describes the change
- `test_diff_not_git`: verify error with suggestion outside git repo

### test_batch.rs

- `test_batch_multiple`: send 3 JSONL commands (search + read + exists), verify 3 results
- `test_batch_order`: verify results come back in input order
- `test_batch_error_isolation`: one failing command doesn't block others
- `test_batch_with_ids`: verify `id` field echoed back in results

### test_init.rs

- `test_init_agents_md`: verify AGENTS.md snippet appended
- `test_init_agents_md_idempotent`: running twice doesn't duplicate snippet
- `test_init_cursor`: with `.cursor/` dir present, verify `mcp.json` written

### test_stats.rs

- `test_stats_after_searches`: run 5 searches, verify stats shows 5 calls
- `test_stats_reset`: verify `--reset` clears data

### test_output_format.rs

- `test_envelope_version`: verify version field matches Cargo.toml version
- `test_error_on_stdout`: verify errors are JSON on stdout, not stderr
- `test_exit_codes`: verify 0 for success, 1 for error, 2 for usage error

## Test Fixtures (tests/fixtures/)

Small sample files for testing. One file per language:

| File | Contents |
|---|---|
| `sample.rs` | 50 lines: 2 functions, 1 struct, 1 impl |
| `sample.py` | 50 lines: 2 functions, 1 class with 2 methods |
| `sample.ts` | 50 lines: 2 exported functions, 1 class, 1 interface |
| `sample.go` | 50 lines: 2 functions, 1 struct, 1 interface |
| `sample.java` | 50 lines: 1 class with 3 methods |
| `sample.c` | 50 lines: 3 functions, 1 struct |
| `sample.js` | 50 lines: 2 functions, 1 class |
| `sample_binary` | Binary file with null bytes, for skip detection |
| `.gitignore` | Ignores `node_modules/` and `target/` |
| `node_modules/ignored.js` | Should be excluded from all results |

## Benchmark Tests (benches/)

### benches/search.rs (criterion)

- `bench_literal_search`: search for known pattern in fixture files
- `bench_semantic_search`: full hybrid search pipeline
- `bench_bm25_indexing`: build BM25 index from 100 chunks
- `bench_embedding`: embed 100 chunks via Model2Vec
- `bench_rrf_fusion`: fuse 100 candidates from two retrievers
- `bench_reranking`: rerank 100 candidates through full pipeline

### benches/chunking.rs (criterion)

- `bench_treesitter_chunking`: chunk a 10000-char Python file
- `bench_line_chunking`: chunk same file without tree-sitter (baseline)

## CI Testing

### GitHub Actions workflow

```
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --no-default-features
cargo build --release
```

Matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`

### Pre-commit checks (developer local)

```
cargo fmt
cargo clippy
cargo test
```

## Coverage

Target: >= 80% line coverage. Tool: `cargo-tarpaulin` or `cargo-llvm-cov`. Report generated in CI; build fails if coverage drops below 70%.
