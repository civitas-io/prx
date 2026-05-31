# CLI Reference

This page documents all prx subcommands, flags, and arguments. Flags and behavior may change between minor versions. Use `prx --version` and the JSON output `version` field for programmatic detection.

## Global Flags

These flags apply to all subcommands.

| Flag | Description |
|------|-------------|
| `--json` | JSON output (default) |
| `--plain` | Human-readable plain text output |
| `--budget N` | Maximum tokens in response (default: unlimited) |
| `--version` | Print version and exit |
| `--help` | Print help and exit |
| `-q, --quiet` | Suppress non-essential output |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (details in stdout JSON) |
| `2` | Usage error (invalid arguments) |

---

## prx search

Search the codebase by query.

```
prx search <query> [path]
```

| Argument | Description |
|----------|-------------|
| `query` | Search query (required) |
| `path` | Root path to search (default: `.`) |

| Flag | Description |
|------|-------------|
| `--literal` | Force literal/regex matching |
| `--semantic` | Force semantic search |
| `--structural` | Force ast-grep structural matching |
| `--mode hybrid\|semantic\|bm25\|literal\|structural` | Explicit mode selection (default: auto-detect) |
| `--top-k N` | Number of results (default: 5) |
| `--budget N` | Token budget for results |
| `--context function\|class\|block\|none` | Return enclosing structural unit (default: none) |
| `--exists` | Bloom filter quick check — returns `{"exists": true/false}` only |
| `--continue TOKEN` | Resume paginated results |
| `--alpha FLOAT` | Override RRF alpha weight (0.0 = pure BM25, 1.0 = pure semantic) |

**Auto-detection:** when no mode flag is provided, the query is classified automatically. Fewer than 3 tokens or regex metacharacters → `--literal`. Contains `$VAR`-style metavariables → `--structural`. Otherwise → `--semantic`.

---

## prx read

Read file content with optional range and structural expansion.

```
prx read <file> [flags]
```

| Argument | Description |
|----------|-------------|
| `file` | File path (required) |

| Flag | Description |
|------|-------------|
| `--lines START-END` | Line range, 1-indexed, inclusive |
| `--snap function\|class\|block` | Expand range to enclosing structure |
| `--skeleton` | Return signatures, types, and exports only |
| `--outline` | Return symbol table (name, kind, line range, signature) |
| `--hash` | Return content hash only (for change detection) |
| `--if-changed HASH` | Return 48-token stub if file hash matches (skip re-read) |
| `--mode aggressive\|diff\|entropy` | Content reduction mode |
| `--budget N` | Maximum tokens of file content |
| `--meta` | Include file metadata (language, lines, bytes, modified timestamp) |

**Read modes:**
- `--mode aggressive` — strip comments and collapse blank lines (1-19% savings)
- `--mode diff` — changed lines vs git HEAD only (80-97% savings on modified files)
- `--mode entropy` — filter repetitive/generated code (5-87% savings)

---

## prx find

List and filter files in the workspace.

```
prx find [path] [flags]
```

| Argument | Description |
|----------|-------------|
| `path` | Root path (default: `.`) |

| Flag | Description |
|------|-------------|
| `--pattern GLOB` | Filter by glob pattern (e.g., `*.ts`) |
| `--depth N` | Maximum directory depth (default: unlimited) |
| `--related-to QUERY` | Semantic relevance scoring for files |
| `--changed-since REF` | Files modified since git ref or timestamp |
| `--outline` | Include per-file symbol counts |
| `--tree` | Tree output only (no flat list) |
| `--flat` | Flat list only (no tree) |
| `--budget N` | Token budget |

---

## prx edit

Find and replace content in a file. Dry-run by default.

```
prx edit <file> --find STRING --replace STRING [flags]
```

| Argument | Description |
|----------|-------------|
| `file` | File path (required) |

| Flag | Description |
|------|-------------|
| `--find STRING` | Text to find (literal by default) |
| `--replace STRING` | Replacement text |
| `--regex` | Interpret `--find` as regex |
| `--apply` | Apply changes to file (default: dry-run preview) |
| `--in-function NAME` | Scope replacement to named function |
| `--in-class NAME` | Scope replacement to named class |
| `--all` | Replace all occurrences (default: first only) |
| `--syntax-check` | Validate syntax after edit (default: true) |

`--find` and `--replace` can be specified multiple times. All replacements are applied atomically.

---

## prx diff

Show git diffs with token-aware truncation.

```
prx diff [file] [flags]
```

| Argument | Description |
|----------|-------------|
| `file` | File path (optional, default: all changed files) |

| Flag | Description |
|------|-------------|
| `--since REF` | Compare against git ref (default: HEAD) |
| `--staged` | Compare staged changes |
| `--stat-only` | Summary and stats only (~30 tokens) |
| `--budget N` | Token budget for hunks |
| `--functions` | Group hunks by function |

---

## prx index

Build or update the search index.

```
prx index [path] [flags]
```

| Argument | Description |
|----------|-------------|
| `path` | Root path to index (default: `.`) |

| Flag | Description |
|------|-------------|
| `--watch` | Watch for file changes and re-index |
| `--rebuild` | Force full re-index |
| `--stats` | Print index statistics |

The index is written to `.prx/index/`. Subsequent searches use the cached index automatically.

---

## prx outline

Print the symbol table for a file or directory.

```
prx outline <file|dir> [flags]
```

| Argument | Description |
|----------|-------------|
| `file\|dir` | File or directory path (required) |

| Flag | Description |
|------|-------------|
| `--depth N` | For directories, max depth |
| `--kind function\|class\|method\|all` | Filter by symbol kind |

---

## prx exists

Probabilistic existence check for a pattern.

```
prx exists <pattern> [path]
```

| Argument | Description |
|----------|-------------|
| `pattern` | Pattern to check (required) |
| `path` | Root path (default: `.`) |

Returns `{"exists": true/false, "confidence": "exact"|"probable"}`.

Uses a bloom filter for O(1) probable check. Falls back to literal search for exact confirmation when `--exact` is passed.

---

## prx run

Run a command and return structured output with only actionable items.

```
prx run <command> [flags]
```

| Argument | Description |
|----------|-------------|
| `command` | Command to run (required, captures all remaining args) |

| Flag | Description |
|------|-------------|
| `--raw` | Bypass parsing, return full output |
| `--full` | Return parsed summary AND full output |
| `--auto-json` | Inject JSON flags for tools that support structured output |
| `--budget N` | Token budget for output |
| `--timeout N` | Command timeout in seconds (default: 300) |

Auto-detects the tool from the command string and applies tool-specific parsing. Unknown commands fall back to exit code + last N lines. See [Run Parsers](../architecture/run-parsers.md) for the full parser catalog.

---

## prx batch

Execute multiple commands in parallel from stdin.

```
prx batch
```

Reads JSONL from stdin. Each line is a command object. Executes commands in parallel. Writes JSONL to stdout, one result per line, in input order.

**Input format:**

```json
{"cmd": "search", "query": "auth", "budget": 300}
{"cmd": "read", "file": "src/auth.ts", "id": "q2"}
```

The optional `"id"` field is echoed in the output line for request correlation.

---

## prx context

Assemble a context package for a module or directory.

```
prx context <path> [flags]
```

Returns stats, documentation, entrypoints, file skeletons, and 1-hop import edges in a single call. Uses the symbol index for entrypoint ranking.

---

## prx impact

Reverse dependency analysis.

```
prx impact <file> [flags]
```

| Flag | Description |
|------|-------------|
| `--symbol NAME` | Narrow analysis to a specific symbol |

Walks the import graph backwards to find all files that depend on the given file or symbol.

---

## prx mcp

Start the MCP server on stdio.

```
prx mcp
```

No arguments. Exposes all prx tools as MCP tools. Designed for agent framework integration. See the [integration guide](../guide/mcp.md) for configuration.

---

## prx init

Generate integration files for agent frameworks.

```
prx init [flags]
```

| Flag | Description |
|------|-------------|
| `--agent FRAMEWORK` | Target framework: `claude-code`, `cursor`, `codex`, `opencode`, `all` |
| `--agents-md` | Append prx usage snippet to AGENTS.md in current directory |

Without flags, auto-detects installed frameworks and writes appropriate configs.

| Framework | File Written | Content |
|---|---|---|
| Claude Code | `.claude/agents/ag-search.md` | Dedicated search sub-agent definition |
| Claude Code | Runs `claude mcp add ag` | MCP server registration |
| Cursor | `.cursor/mcp.json` | MCP server entry |
| Codex | `~/.codex/config.toml` | MCP server entry |
| OpenCode | `~/.opencode/config.json` | MCP server entry |
| Any | Appends to `AGENTS.md` | Usage snippet with workflow guidance |

---

## prx stats

Print token savings dashboard.

```
prx stats [flags]
```

| Flag | Description |
|------|-------------|
| `--verbose` | Per-command breakdown |
| `--reset` | Clear saved statistics |

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `PRX_MAX_FILE_SIZE` | 1MB | Maximum file size to process |
| `PRX_CHUNK_SIZE` | 1500 | Chunk target in characters |
| `RUST_LOG` | — | Debug logging level (output goes to stderr) |

## Ignore Files

prx respects `.gitignore` by default. Add a `.prxignore` file alongside `.gitignore` for prx-specific exclusions. The format is identical to `.gitignore`.
