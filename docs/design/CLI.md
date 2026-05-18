# CLI Interface Specification

> This document describes the CLI interface for ag v0.1.x. Flags and behavior may change between minor versions. Use `prx --version` and the JSON output `version` field for programmatic detection.

---

## Global Flags

Apply to all subcommands.

| Flag | Description |
|------|-------------|
| `--json` | JSON output (default) |
| `--plain` | Human-readable plain text output |
| `--budget N` | Maximum tokens in response (default: unlimited) |
| `--version` | Print version and exit |
| `--help` | Print help and exit |
| `-q, --quiet` | Suppress non-essential output |

---

## Subcommands

### `prx search <query> [path]`

Search the codebase by query.

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

---

### `prx read <file> [--lines START-END]`

Read file content with optional range and structural expansion.

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
| `--budget N` | Maximum tokens of file content |
| `--meta` | Include file metadata (language, lines, bytes, modified timestamp) |

---

### `prx find [path]`

List and filter files in the workspace.

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

### `prx edit <file>`

Find and replace content in a file.

| Argument | Description |
|----------|-------------|
| `file` | File path (required) |

| Flag | Description |
|------|-------------|
| `--find STRING` | Text to find (literal by default) |
| `--replace STRING` | Replacement text |
| `--regex` | Interpret `--find` as regex |
| `--dry-run` | Preview changes without applying (default) |
| `--apply` | Apply changes to file |
| `--in-function NAME` | Scope replacement to named function |
| `--in-class NAME` | Scope replacement to named class |
| `--all` | Replace all occurrences (default: first only) |
| `--syntax-check` | Validate syntax after edit (default: true) |

---

### `prx diff [file]`

Show git diffs with token-aware truncation.

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

### `prx index [path]`

Build or update the search index.

| Argument | Description |
|----------|-------------|
| `path` | Root path to index (default: `.`) |

| Flag | Description |
|------|-------------|
| `--watch` | Watch for file changes and re-index |
| `--rebuild` | Force full re-index |
| `--stats` | Print index statistics |

---

### `prx outline <file|dir>`

Print the symbol table for a file or directory.

| Argument | Description |
|----------|-------------|
| `file\|dir` | File or directory path (required) |

| Flag | Description |
|------|-------------|
| `--depth N` | For directories, max depth |
| `--kind function\|class\|method\|all` | Filter by symbol kind |

---

### `prx exists <pattern> [path]`

Probabilistic existence check for a pattern.

| Argument | Description |
|----------|-------------|
| `pattern` | Pattern to check (required) |
| `path` | Root path (default: `.`) |

Returns `{"exists": true/false, "confidence": "exact"|"probable"}`.

Uses bloom filter for O(1) probable check, falls back to literal search for exact confirmation.

---

### `prx mcp`

Start the MCP server on stdio.

No arguments. Exposes all ag tools as MCP tools. Designed for agent framework integration.

---

### `prx batch`

Execute multiple commands in parallel from stdin.

Reads JSONL from stdin. Each line is a command object. Executes commands in parallel. Writes JSONL to stdout, one result per line.

**Input format:**
```
{"cmd": "search", "query": "auth", "budget": 300}
```

---

### `prx stats`

Print token savings dashboard.

| Flag | Description |
|------|-------------|
| `--verbose` | Per-command breakdown |
| `--reset` | Clear saved statistics |

---

## prx init

Generate integration files for agent frameworks.

| Flag | Description |
|------|-------------|
| `--agent FRAMEWORK` | Target framework: `claude-code`, `cursor`, `codex`, `opencode`, `all` |
| `--agents-md` | Append ag usage snippet to AGENTS.md in current directory |

**Without flags**: auto-detects installed frameworks and writes appropriate configs.

**What it writes per framework:**

| Framework | File Written | Content |
|---|---|---|
| Claude Code | `.claude/agents/ag-search.md` | Dedicated search sub-agent definition |
| Claude Code | Runs `claude mcp add ag` | MCP server registration |
| Cursor | `.cursor/mcp.json` | MCP server entry |
| Codex | `~/.codex/config.toml` | MCP server entry |
| OpenCode | `~/.opencode/config.json` | MCP server entry |
| Any | Appends to `AGENTS.md` | Usage snippet with workflow guidance |

**Rationale**: MCP integration only works for top-level agents. Sub-agents
(Claude Code explore agents, Codex sub-agents) cannot call MCP tools and must
invoke ag via bash. The AGENTS.md snippet and Claude Code sub-agent definition
ensure ag is available at every level of agent delegation.

---

## prx run

Run a command and return structured output with only actionable items.

| Argument | Description |
|----------|-------------|
| `command` | Command to run (required, captures all remaining args) |

| Flag | Description |
|------|-------------|
| `--raw` | Bypass parsing, return full output |
| `--full` | Return parsed summary AND full output |
| `--budget N` | Token budget for output |
| `--timeout N` | Command timeout in seconds (default: 300) |

Auto-detects the tool from the command string (cargo test, pytest, go test,
etc.) and applies tool-specific parsing. Unknown commands fall back to exit
code + last N lines. See docs/design/PRX-RUN.md for full specification.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (details in stdout JSON) |
| `2` | Usage error (invalid arguments) |
