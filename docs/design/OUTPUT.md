# Output Format Specification

All JSON output includes a top-level `version` field (semver string) for programmatic compatibility detection. Schemas described here apply to prx v0.2.x.

---

## Design Principles

- All output is JSON by default (`--plain` for human fallback)
- Errors go to stdout as structured JSON, never stderr
- Every content response includes a `tokens` field (estimated token count)
- Every file reference includes repo-relative path, line numbers, and content hash
- JSONL streaming for large result sets (one JSON object per line)

---

## Common Envelope

Every response uses this envelope. `status` is `"ok"` or `"error"`.

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": {}
}
```

On error, `data` is absent and `error` is present. `error.code` is a stable machine-readable string. `error.suggestion` may be absent.

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "error",
  "error": {
    "code": "file_not_found",
    "message": "File not found: src/auth.ts",
    "suggestion": "Check the file path. Use `prx find` to discover files."
  }
}
```

---

## prx search

```json
{
  "version": "0.2.0",
  "command": "search",
  "status": "ok",
  "tokens": 612,
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
        "snippet": "export async function verifyToken(token: string): Promise<User> {\n  const decoded = jwt.verify(token, process.env.JWT_SECRET);\n  return db.users.findById(decoded.sub);\n}",
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

With `--exists`: `data` contains only `exists` (bool) and `confidence` (`"exact"` when the literal string was found, `"probable"` for a semantic match).

---

## prx read

```json
{
  "version": "0.2.0",
  "command": "read",
  "status": "ok",
  "tokens": 1043,
  "data": {
    "file": "src/auth.ts",
    "meta": {
      "language": "typescript",
      "lines": 198,
      "bytes": 5421,
      "modified": 1747526400,
      "hash": "sha256:a3f1c9e2b84d7f0e1c2a9b3d5e7f8a1b2c4d6e8f"
    },
    "content": {
      "range": { "start": 1, "end": 198 },
      "snap": null,
      "snap_reason": null,
      "text": "import jwt from 'jsonwebtoken';\nimport { db } from './db';\n",
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

`snap` is a label when the file was too large and a section was selected (e.g., `"top_of_file"`); `snap_reason` explains why. `--skeleton` replaces bodies with `// ...`. `--outline` nulls `data.content`. `--hash` nulls both `data.content` and `data.outline`.

---

## prx find

```json
{
  "version": "0.2.0",
  "command": "find",
  "status": "ok",
  "tokens": 204,
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
      },
      {
        "path": "/Users/jeryn/project/src/middleware/cors.ts",
        "lines": 34,
        "symbols": 3,
        "language": "typescript",
        "relevance": null
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

`--tree` nulls `data.flat`; `--flat` nulls `data.tree`; default populates both. `relevance` is `null` when no query was provided.

---

## prx edit

```json
{
  "version": "0.2.0",
  "command": "edit",
  "status": "ok",
  "tokens": 89,
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

---

## prx diff

```json
{
  "version": "0.2.0",
  "command": "diff",
  "status": "ok",
  "tokens": 341,
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
          { "type": "deletion", "old": "  const decoded = jwt.verify(token, process.env.JWT_SECRET);", "new": null },
          { "type": "addition", "old": null, "new": "  const decoded = jwt.verify(token, config.jwtSecret);" }]
      }
    ]
  }
}
```

`--stat-only` nulls `data.hunks`. `change.type` is `"modification"` when both `old` and `new` are present.

---

## prx outline

```json
{
  "version": "0.2.0",
  "command": "outline",
  "status": "ok",
  "tokens": 156,
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
          { "name": "login", "kind": "method", "lines": { "start": 65, "end": 88 }, "signature": "async login(email: string, password: string): Promise<Session>", "children": [] }
        ]
      }
    ]
  }
}
```

`kind` is one of: `function`, `class`, `method`, `struct`, `enum`, `trait`, `type`, `const`. `children` is always an array.

---

## prx index

```json
{
  "version": "0.2.0",
  "command": "index",
  "status": "ok",
  "tokens": 42,
  "data": {
    "path": "/Users/jeryn/project/src",
    "files_indexed": 47,
    "chunks": 312,
    "duration_ms": 1840,
    "languages": { "typescript": 38, "json": 6, "markdown": 3 }
  }
}
```

---

## prx exists

```json
{
  "version": "0.2.0",
  "command": "exists",
  "status": "ok",
  "tokens": 14,
  "data": {
    "exists": false,
    "confidence": "exact",
    "pattern": "src/payments/stripe.ts"
  }
}
```

`confidence` is `"exact"` for literal path lookups, `"probable"` for fuzzy or semantic matches.

---

## prx batch

Output is JSONL: one complete envelope per line, in input order. Each line is self-contained.

```jsonl
{"version":"0.2.0","command":"search","status":"ok","id":"q1","tokens":612,"data":{"matches":[{"file":"src/auth.ts","line":42,"column":7,"match":"verifyToken","context_type":"function","context_name":"verifyToken","context_signature":"async function verifyToken(token: string): Promise<User>","snippet":"export async function verifyToken(token: string): Promise<User> {","relevance":0.94,"language":"typescript"}],"total_matches":1,"returned":1,"budget_used":612,"truncated":false,"continuation_token":null}}
{"version":"0.2.0","command":"read","status":"error","id":"q2","error":{"code":"file_not_found","message":"File not found: src/payments/stripe.ts","suggestion":"Check the file path. Use `prx find` to discover files."}}
```

Input commands with an `"id"` field have it echoed in their output line. Commands without one omit the field.

---

## prx stats

```json
{
  "version": "0.2.0",
  "command": "stats",
  "status": "ok",
  "tokens": 67,
  "data": {
    "periods": [
      { "label": "last_hour",  "calls": 14,   "tokens_saved": 18420,   "savings_percent": 73.4 },
      { "label": "last_24h",   "calls": 89,   "tokens_saved": 104300,  "savings_percent": 68.1 },
      { "label": "all_time",   "calls": 1204, "tokens_saved": 1382900, "savings_percent": 71.2 }
    ]
  }
}
```

---

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
