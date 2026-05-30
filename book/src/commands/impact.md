# impact

Reverse dependency analysis: what depends on a given file or symbol.

## Usage

```bash
prx impact [options] <file>
```

## Options

| Flag | Description |
|---|---|
| `--symbol <name>` | Narrow to a specific exported symbol |
| `--hops N` | Limit traversal depth (default: all reachable) |
| `--budget N` | Cap output at N tokens |
| `--plain` | Human-readable output |

## What it returns

- **Target exports** — what the file exports
- **Dependent files** — files that import the target, with hop distance
- **Symbol attribution** — which symbols each dependent uses
- **Stats** — direct count, transitive count, test file count

## Examples

```bash
# What depends on this file?
prx impact src/auth/handler.ts

# What uses this specific function?
prx impact src/auth/handler.ts --symbol authenticate

# Direct dependents only (1 hop)
prx impact src/auth/handler.ts --hops 1
```

Example output:

```json
{
  "data": {
    "target": "src/auth/handler.ts",
    "exports": ["handleLogin", "handleLogout", "authenticate"],
    "dependents": [
      {
        "file": "src/routes/api.ts",
        "hops": 1,
        "symbols_used": ["handleLogin", "authenticate"]
      },
      {
        "file": "src/middleware/auth.ts",
        "hops": 1,
        "symbols_used": ["authenticate"]
      },
      {
        "file": "src/tests/auth.test.ts",
        "hops": 1,
        "symbols_used": ["handleLogin", "handleLogout"]
      }
    ],
    "stats": {
      "direct": 3,
      "transitive": 7,
      "test_files": 1
    }
  }
}
```

## How it works

`prx impact` does a reverse walk of the import graph built by `prx index`. Import edges are extracted from the AST using tree-sitter across 10 language families.

When an import name is ambiguous across many files, resolution falls back to a directory-proximity heuristic and returns the most likely candidates. Treat the output as a high-quality map, not a formal proof of completeness.

## Tips

- Run `prx impact` before any refactor that touches a shared file. It tells you the blast radius before you make the change.
- Use `--symbol` to narrow the analysis when you're only changing one export. A file might have 10 dependents, but only 2 of them use the symbol you're changing.
- Use `--hops 1` for a quick check of direct dependents. The transitive closure can be large on central files.
- The `test_files` count in stats tells you how many test files will need updating.
- Run `prx index .` first to build the import graph. Without an index, impact analysis falls back to a slower on-demand extraction.

See also: [context](context.md), [index](index.md), [search](search.md)
