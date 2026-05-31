# context

Module context package: stats, documentation, entrypoints, per-file skeletons, and import edges. One call instead of four.

## Usage

```bash
prx context [options] <directory>
```

## Options

| Flag | Description |
|---|---|
| `--budget N` | Cap output at N tokens |
| `--no-edges` | Skip import graph edges |
| `--plain` | Human-readable output |

## What it returns

A single structured response containing:

- **Stats** — file count, total lines, language breakdown
- **Documentation** — README or doc content if present
- **Entrypoints** — top files ranked by reference count (most-imported files first)
- **Skeletons** — per-file symbol signatures without bodies
- **Import edges** — 1-hop import graph connecting the files in the directory

## Examples

```bash
# Full module context
prx context src/auth/

# With a token cap
prx context src/auth/ --budget 2000

# Skip import graph (faster, fewer tokens)
prx context src/auth/ --no-edges
```

## Why this matters

Without `prx context`, understanding a module requires:

```bash
prx find src/auth/ --flat-only          # file list
cat src/auth/README.md                  # documentation
prx outline src/auth/handler.ts         # symbols in each file
prx outline src/auth/middleware.ts
prx outline src/auth/types.ts
# ... and then manually tracing imports
```

`prx context` collapses that into one call. The entrypoints ranking tells you which files are most central to the module (highest reference count), so you know where to start reading.

## Token savings

Replacing 4-5 manual exploration calls with one `prx context` call saves 60-80% of the tokens, depending on module size.

## Tips

- Use `prx context` at the start of any task that involves an unfamiliar module. It gives you the mental model you need to start working without reading every file.
- Use `--no-edges` when you only need the file structure and don't need to trace imports.
- Use `--budget` to control output size on large modules. The response is ranked by relevance, so the most important information comes first.
- For a single file, `prx read src/file.ts --skeleton` is more appropriate than `prx context`.

See also: [impact](impact.md), [outline](outline.md), [find](find.md)
