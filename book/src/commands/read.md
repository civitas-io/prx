# read

Structured file reading with metadata, content hashing, and multiple modes for reducing token usage.

## Usage

```bash
prx read [options] <file>
```

## Options

| Flag | Description |
|---|---|
| `--skeleton` | Return signatures and exports only (~10% of tokens) |
| `--outline` | Return symbol table only |
| `--lines N` or `--lines N-M` | Read a specific line or range |
| `--snap function` | Expand line range to enclosing function boundary |
| `--snap class` | Expand line range to enclosing class boundary |
| `--if-changed <hash>` | Return cached stub if file hasn't changed |
| `--hash` | Return content hash only |
| `--mode aggressive` | Strip comments using tree-sitter |
| `--mode diff` | Return only lines changed vs git HEAD |
| `--mode entropy` | Filter repetitive lines |
| `--budget N` | Cap output at N tokens |
| `--plain` | Human-readable output |

## Default read

```bash
prx read src/auth.ts                    # full file + metadata + outline
```

Every response includes `meta.hash` (xxh3 content hash), line count, language, and a symbol outline.

## Skeleton mode

Returns function signatures, type definitions, and exports without bodies. About 10% of the tokens of a full read.

```bash
prx read src/auth.ts --skeleton
```

Use this before reading a full file to understand what's in it.

## Reading specific lines

```bash
prx read src/auth.ts --lines 42-67       # line range
prx read src/auth.ts --lines 42 --snap function  # expand to enclosing function
prx read src/auth.ts --lines 42 --snap class     # expand to enclosing class
```

`--snap` is useful when you know a line number from a search result but want the full function context.

## Conditional read (--if-changed)

Pass the `meta.hash` from a previous read. If the file hasn't changed, prx returns a tiny stub instead of the full content.

```bash
# First read — note the hash in meta.hash
prx read src/auth.ts
# Response: { "meta": { "hash": "a3f9b2c1..." }, ... }

# Subsequent reads — skip if unchanged
prx read src/auth.ts --if-changed a3f9b2c1...
# Unchanged: { "cached": true, "meta": {...} } — ~50 bytes
# Changed: full content returned normally
```

Benchmark on an 845-line Rust file:

| Scenario | Tokens | Savings |
|---|---|---|
| Full read | 6,531 | — |
| `--if-changed` (cache hit) | 57 | 99.1% |
| `--if-changed` (cache miss) | 6,531 | 0% (full content) |

## Aggressive mode

Strips comments using tree-sitter (14 grammars) and collapses blank lines. Preserves all functional code and strings containing comment-like syntax.

```bash
prx read src/auth.ts --mode aggressive
```

| File type | Savings |
|---|---|
| Clean Rust code (few comments) | 1-7% |
| Python with docstrings | 11-19% |
| Heavily commented config files | 13-19% |
| Code with inline comments | 5-14% |

## Diff mode

Returns only lines that changed vs git HEAD. Falls back to full content for untracked files or files outside a git repo.

```bash
prx read src/auth.ts --mode diff
```

Output uses `+`/`-` prefixes with line numbers:

```
+L42: fn new_function() {
+L43:     let x = 1;
+L44: }
-L50:     let old_value = 0;
+L50:     let new_value = 1;
```

Benchmark on an 845-line Rust file with 10 lines changed:

| Scenario | Tokens | Savings |
|---|---|---|
| Full read | 6,603 | — |
| `--mode diff` | 89 | 98.7% |
| No changes vs HEAD | 5 | 99.9% |

## Entropy mode

Filters repetitive lines by normalizing patterns (digits replaced, whitespace trimmed). Allows 3 occurrences of each pattern, suppresses the rest. Appends a count of filtered lines.

```bash
prx read generated/schema.rs --mode entropy
```

| File type | Savings |
|---|---|
| Generated structs (50+ fields) | 86% |
| Repetitive test assertions | 15-18% |
| Config files with similar entries | 3-6% |
| Normal source code | 0% |

## Combining modes

`--if-changed` takes priority. On a cache miss, `--mode` applies normally:

```bash
# If unchanged: cached stub (57 tokens)
# If changed: aggressive mode applied to new content
prx read src/auth.ts --if-changed abc123... --mode aggressive
```

## Tips

- Always use `--skeleton` or `--outline` before reading a full file. It costs ~10% of the tokens and tells you what's in the file.
- Store `meta.hash` from every read and pass it back with `--if-changed` on subsequent reads. Re-reads of unchanged files are the single highest-ROI optimization.
- Use `--snap function` when you have a line number from a search result. It gives you the full function without the rest of the file.
- Use `--mode diff` when you want to see what changed, not the whole file.
- Use `--mode entropy` on generated code, migration files, or anything with repetitive structure.

See also: [search](search.md), [outline](outline.md), [diff](diff.md)
