# JSON Output Format

All prx output is JSON by default. Every response uses a common envelope. This page documents the envelope, error format, per-command data schemas, and error codes.

Use `--plain` for human-readable output. Use `--budget N` to cap token usage.

## Common Envelope

Every response uses this structure. `status` is `"ok"` or `"error"`.

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": {}
}
```

| Field | Type | Description |
|---|---|---|
| `version` | string | prx version (semver). Use this for programmatic compatibility detection. |
| `command` | string | Subcommand that produced this response. |
| `status` | string | `"ok"` or `"error"`. |
| `tokens` | number | Estimated token count of the entire JSON response (envelope + data). |
| `data` | object | Command-specific payload. Absent on error. |

**Token counting:** uses `byte_count / 4` when `--budget` is not specified, exact cl100k_base count when `--budget` is active.

## Error Envelope

On error, `data` is absent and `error` is present.

```json
{
  "version": "0.2.0",
  "command": "read",
  "status": "error",
  "error": {
    "code": "file_not_found",
    "message": "File not found: src/auth.ts",
    "suggestion": "Check the file path. Use `prx find` to discover files."
  }
}
```

| Field | Type | Description |
|---|---|---|
| `error.code` | string | Stable machine-readable error code. |
| `error.message` | string | Human-readable description. |
| `error.suggestion` | string | Optional. Actionable recovery hint. |

Errors always go to stdout. stderr is reserved for `RUST_LOG` debug logging only.

## Fallback Envelope

When prx fails internally and falls back to a Unix tool, the envelope includes `"fallback": true`:

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 1250,
  "fallback": true,
  "data": {
    "raw": "src/auth.rs:42:fn authenticate(...)\n",
    "source": "grep -rn \"pattern\" path/"
  }
}
```

## prx search

```json
{
  "data": {
    "matches": [
      {
        "file": "src/auth.ts",
        "line": 42,
        "column": 7,
        "match": "verifyToken",
        "context_type": "function",
        "context_name": "verifyToken",
        "context_signature": "async function verifyToken(token: string): Promise<User>",
        "snippet": "export async function verifyToken(token: string): Promise<User> {\n  ...\n}",
        "relevance": 0.94,
        "language": "typescript"
      }
    ],
    "total_matches": 7,
    "returned": 1,
    "budget_used": 612,
    "truncated": true,
    "continuation_token": "eyJvZmZzZXQiOjF9"
  }
}
```

With `--exists`: `data` contains only `exists` (bool) and `confidence` (`"exact"` or `"probable"`).

To fetch the next page, pass `--continue <continuation_token>`.

## prx read

```json
{
  "data": {
    "file": "src/auth.ts",
    "meta": {
      "language": "typescript",
      "lines": 198,
      "bytes": 5421,
      "modified": 1747526400,
      "hash": "a3f1c9e2b84d7f0e1c2a9b3d5e7f8a1b2c4d6e8f"
    },
    "content": {
      "range": { "start": 1, "end": 198 },
      "snap": null,
      "snap_reason": null,
      "text": "import jwt from 'jsonwebtoken';\n...",
      "tokens": 1043
    },
    "outline": [
      {
        "name": "verifyToken",
        "kind": "function",
        "lines": { "start": 42, "end": 55 },
        "signature": "async function verifyToken(token: string): Promise<User>"
      }
    ]
  }
}
```

`outline` is included by default alongside content. One call returns content, symbol table, metadata, and hash.

`--skeleton` replaces function bodies with `// ...`. `--outline` nulls `data.content`. `--hash` nulls both `data.content` and `data.outline`.

`snap` is a label when the file was too large and a section was selected (e.g., `"top_of_file"`). `snap_reason` explains why.

## prx find

```json
{
  "data": {
    "tree": {
      "src": {
        "auth.ts": { "lines": 198, "symbols": 12, "language": "typescript" },
        "middleware": {
          "cors.ts": { "lines": 34, "symbols": 3, "language": "typescript" }
        }
      }
    },
    "flat": [
      {
        "path": "src/auth.ts",
        "lines": 198,
        "symbols": 12,
        "language": "typescript",
        "relevance": 0.91
      }
    ],
    "stats": {
      "total_files": 47,
      "returned": 2,
      "budget_used": 204
    }
  }
}
```

`--tree` nulls `data.flat`. `--flat` nulls `data.tree`. Default populates both. `relevance` is `null` when no `--related-to` query was provided.

## prx edit

```json
{
  "data": {
    "file": "src/auth.ts",
    "dry_run": false,
    "changes": [
      {
        "line": 44,
        "function": "verifyToken",
        "before": "  const decoded = jwt.verify(token, process.env.JWT_SECRET);",
        "after": "  const decoded = jwt.verify(token, config.jwtSecret);"
      }
    ],
    "total_replacements": 1,
    "syntax_valid": true,
    "syntax_error": null
  }
}
```

`dry_run: true` means no file was written. `syntax_error` is a string when `syntax_valid` is `false`.

## prx diff

```json
{
  "data": {
    "summary": "Replaced hardcoded JWT secret with config lookup in verifyToken",
    "stats": {
      "additions": 2,
      "deletions": 1,
      "files_changed": 1,
      "functions_changed": ["verifyToken"]
    },
    "semantic_notes": ["No signature changes", "New import: config"],
    "hunks": [
      {
        "file": "src/auth.ts",
        "function": "verifyToken",
        "old_range": { "start": 44, "end": 44 },
        "new_range": { "start": 44, "end": 45 },
        "changes": [
          { "type": "deletion", "old": "  const decoded = ...", "new": null },
          { "type": "addition", "old": null, "new": "  const decoded = ..." }
        ]
      }
    ]
  }
}
```

`--stat-only` nulls `data.hunks`. `change.type` is `"modification"` when both `old` and `new` are present.

## prx outline

```json
{
  "data": {
    "file": "src/auth.ts",
    "language": "typescript",
    "symbols": [
      {
        "name": "AuthService",
        "kind": "class",
        "lines": { "start": 60, "end": 140 },
        "signature": "class AuthService",
        "children": [
          {
            "name": "login",
            "kind": "method",
            "lines": { "start": 65, "end": 88 },
            "signature": "async login(email: string, password: string): Promise<Session>",
            "children": []
          }
        ]
      }
    ]
  }
}
```

`kind` is one of: `function`, `class`, `method`, `struct`, `enum`, `trait`, `type`, `const`. `children` is always an array.

## prx index

```json
{
  "data": {
    "path": "/project/src",
    "files_indexed": 47,
    "chunks": 312,
    "duration_ms": 1840,
    "languages": { "typescript": 38, "json": 6, "markdown": 3 }
  }
}
```

## prx exists

```json
{
  "data": {
    "exists": false,
    "confidence": "exact",
    "pattern": "src/payments/stripe.ts"
  }
}
```

`confidence` is `"exact"` for literal path lookups and confirmed literal searches. `"probable"` for bloom filter results that haven't been confirmed.

## prx stats

```json
{
  "data": {
    "periods": [
      { "label": "last_hour",  "calls": 14,   "tokens_saved": 18420,   "savings_percent": 73.4 },
      { "label": "last_24h",   "calls": 89,   "tokens_saved": 104300,  "savings_percent": 68.1 },
      { "label": "all_time",   "calls": 1204, "tokens_saved": 1382900, "savings_percent": 71.2 }
    ]
  }
}
```

## prx batch

Output is JSONL: one complete envelope per line, in input order. Each line is self-contained.

```jsonl
{"version":"0.2.0","command":"search","status":"ok","id":"q1","tokens":612,"data":{...}}
{"version":"0.2.0","command":"read","status":"error","id":"q2","error":{"code":"file_not_found","message":"File not found: src/payments/stripe.ts","suggestion":"Check the file path. Use `prx find` to discover files."}}
```

Input commands with an `"id"` field have it echoed in their output line.

## Error Codes

| Code | Meaning |
|---|---|
| `file_not_found` | Path does not exist or is not readable |
| `parse_error` | File could not be parsed for the requested language |
| `budget_exceeded` | Request would exceed the token budget |
| `invalid_range` | Line range is out of bounds for the file |
| `index_missing` | No index found for the requested path |
| `invalid_command` | Unrecognized subcommand in a batch request |
| `syntax_error` | Edit produced syntactically invalid output |
| `permission_denied` | File exists but cannot be read or written |
