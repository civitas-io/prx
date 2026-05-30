# find

Codebase mapping with tree and flat output, inline metadata, and optional semantic scoring.

## Usage

```bash
prx find [options] [path]
```

## Options

| Flag | Description |
|---|---|
| `--pattern <glob>` | Filter by glob pattern (e.g. `"*.ts"`) |
| `--depth N` | Limit directory depth |
| `--changed-since <ref>` | Only files modified since a git ref |
| `--tree-only` | Return tree structure only |
| `--flat-only` | Return flat list only |
| `--budget N` | Cap output at N tokens |
| `--plain` | Human-readable output |

## Examples

```bash
# Find all TypeScript files up to 3 levels deep
prx find src/ --pattern "*.ts" --depth 3

# Find recently modified files
prx find src/ --changed-since HEAD~3

# Tree structure only
prx find . --tree-only

# Flat list only
prx find . --flat-only
```

Example output (flat):

```json
{
  "data": {
    "files": [
      {
        "path": "src/auth/handler.ts",
        "lines": 245,
        "language": "typescript",
        "modified": "2026-05-29T10:23:00Z"
      },
      {
        "path": "src/auth/middleware.ts",
        "lines": 89,
        "language": "typescript",
        "modified": "2026-05-28T14:11:00Z"
      }
    ],
    "total": 2
  }
}
```

## Tips

- `prx find` returns structured JSON with metadata (lines, language, modification time) that `find`+`wc`+`file` would require multiple follow-up commands to produce.
- Use `--changed-since HEAD~3` at the start of a task to scope your work to recently modified files.
- Use `--depth` to avoid pulling in deeply nested vendor or generated directories.
- Combine with `prx context` to get a full module picture: `prx find src/auth/ --flat-only` gives you the file list, `prx context src/auth/` gives you the full module shape.

See also: [context](context.md), [index](index.md)
