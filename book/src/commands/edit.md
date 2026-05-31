# edit

Safe file editing with literal matching, dry-run by default, and tree-sitter syntax validation.

## Usage

```bash
prx edit [options] <file> --find <text> --replace <text>
```

## Options

| Flag | Description |
|---|---|
| `--find <text>` | Text to find (required) |
| `--replace <text>` | Replacement text (required) |
| `--apply` | Write the change to disk (default: dry-run) |
| `--regex` | Treat `--find` as a regex pattern |
| `--in-function <name>` | Scope the edit to a specific function |
| `--plain` | Human-readable output |

## Examples

```bash
# Preview a change (dry-run — default)
prx edit src/auth.ts --find "old_api()" --replace "new_api()"

# Apply the change
prx edit src/auth.ts --find "old_api()" --replace "new_api()" --apply

# Regex mode
prx edit src/auth.ts --find "TODO.*" --replace "" --regex

# Scope to a specific function
prx edit src/auth.ts --find "x" --replace "y" --in-function "handleLogin"
```

Dry-run output shows what would change before anything is written:

```json
{
  "data": {
    "applied": false,
    "changes": [
      {
        "line": 42,
        "before": "    return old_api(result);",
        "after": "    return new_api(result);"
      }
    ],
    "total_changes": 1
  }
}
```

## Dry-run by default

`prx edit` never writes to disk unless you pass `--apply`. This lets you preview every change before committing it. The dry-run output shows exactly which lines would change and what they'd look like after.

## Syntax validation

After applying a change, prx validates the result with tree-sitter. If the edit produces a syntax error, the change is rejected and the original file is left intact.

## Tips

- Always run without `--apply` first to see what will change.
- Use `--in-function` to scope edits when the same string appears in multiple places but you only want to change it in one function.
- For multi-file renames, use `prx batch` to send multiple edit commands in one call.
- If you need to make the same change across many files, `prx batch` with a JSONL file of edit commands is more efficient than running `prx edit` in a loop.

See also: [diff](diff.md), [batch](other.md)
