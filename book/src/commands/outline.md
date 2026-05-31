# outline

Symbol table for a file or directory. Extracts function definitions, type definitions, classes, constants, and other named symbols using tree-sitter.

## Usage

```bash
prx outline [options] <file-or-directory>
```

## Options

| Flag | Description |
|---|---|
| `--depth N` | Limit directory traversal depth |
| `--kind <kind>` | Filter by symbol kind (function, class, struct, etc.) |
| `--budget N` | Cap output at N tokens |
| `--plain` | Human-readable output |

## Examples

```bash
# Single file
prx outline src/auth.ts

# Directory
prx outline src/ --depth 2

# Filter by kind
prx outline src/ --kind function
```

Example output:

```json
{
  "data": {
    "symbols": [
      {
        "name": "handleLogin",
        "kind": "function",
        "file": "src/auth/handler.ts",
        "line": 42,
        "exported": true
      },
      {
        "name": "AuthConfig",
        "kind": "interface",
        "file": "src/auth/types.ts",
        "line": 8,
        "exported": true
      }
    ],
    "total": 2
  }
}
```

## Tips

- `prx outline` is the ctags equivalent. Use it when you need a symbol table without reading full file content.
- For a single file, `prx read src/file.ts --outline` returns the same symbol table as part of the read response.
- Use `--kind function` to find all function definitions in a directory quickly.
- `prx context` includes per-file outlines as part of its module context package. If you need both the file structure and the symbols, `prx context` is more efficient than running `prx outline` separately.

See also: [read](read.md), [context](context.md), [search](search.md)
