# diff

Semantic diffs with function-level attribution and natural-language summaries.

## Usage

```bash
prx diff [options] [file]
```

## Options

| Flag | Description |
|---|---|
| `--since <ref>` | Compare against a git ref (default: HEAD) |
| `--staged` | Show staged changes |
| `--stat-only` | Summary only (~30 tokens) |
| `--budget N` | Cap output at N tokens |
| `--plain` | Human-readable output |

## Examples

```bash
# All changed files vs HEAD
prx diff

# Single file
prx diff src/auth.ts

# Compare against a specific ref
prx diff --since HEAD~3

# Staged changes only
prx diff --staged

# Cheap summary (~30 tokens)
prx diff --stat-only
```

Example output:

```json
{
  "data": {
    "files_changed": 2,
    "insertions": 15,
    "deletions": 8,
    "hunks": [
      {
        "file": "src/auth/handler.ts",
        "function": "handleLogin",
        "added": ["+    const token = jwt.sign(payload, secret);"],
        "removed": ["-    const token = createToken(payload);"]
      }
    ]
  }
}
```

## Tips

- Use `--stat-only` for a cheap change summary at the start of a task. It costs ~30 tokens and tells you which files changed and how much.
- `prx diff` attributes hunks to the enclosing function, which is more useful than raw line numbers when reviewing changes.
- For seeing what changed in a single file without loading the whole file, `prx read src/file.ts --mode diff` is often more convenient.

See also: [read](read.md), [edit](edit.md)
