# ag — Agent-Native Unix Tools

**Status:** Draft  
**Date:** 2026-05-18  
**Owner:** Product

---

## Problem Statement

AI coding agents waste between 30% and 93% of their token budget on exploration work that produces no code changes. The root cause is a mismatch: Unix tools were designed for human eyes, and agents must re-parse their output to extract structured meaning.

The canonical failure mode is the grep-read-grep loop:

1. Agent runs `grep` to find a symbol. Gets file paths and line numbers.
2. Agent runs `cat` on each file to read context. Gets entire files.
3. Agent runs `grep` again to narrow down. Gets the same noise.

This loop alone accounts for 93% of consumed tokens in typical agent sessions. The tools aren't broken for humans. They're wrong for agents.

**What agents actually need:**

- One call that returns metadata, content, and context together
- Output sized to a token budget, not a terminal window
- Structured data they can act on without re-parsing
- Content hashes so they know when nothing has changed

No existing tool provides this. `ripgrep` is fast but still human-shaped. `jq` requires the data to already be structured. LSP servers require a daemon and a protocol handshake. Agents are left duct-taping Unix tools together and paying the token tax on every call.

---

## Target Users

### Primary: AI Coding Agents

| Agent | Usage Pattern |
|---|---|
| Claude Code | File exploration, symbol search, targeted edits |
| Cursor | Context gathering for autocomplete and chat |
| OpenCode | Full agentic coding sessions |
| Aider | Diff-based editing workflows |
| SWE-agent | Benchmark task execution |
| Devin | Long-horizon autonomous coding |
| Codex | Code generation with repo context |

These agents share a common constraint: every token spent on tool output is a token not spent on reasoning or code generation.

### Secondary: Agent Toolchain Developers

Engineers building agent frameworks, MCP servers, or coding assistants who need a reliable, structured interface to the filesystem. They want a single dependency that handles search, read, edit, and diff without requiring them to wrap and normalize five different Unix tools.

---

## Product Vision

`prx` is a single Rust binary that ships as one file and replaces the five Unix tools agents use most. It's not a wrapper around existing tools. It's built from the ground up with structured output, token budgets, and agent workflows as the primary design constraints.

Every subcommand returns JSON. Every content-returning command accepts `--budget N` to cap token usage intelligently. Every response includes content hashes so agents can skip re-reads. The binary includes everything it needs: no runtime dependencies, no internet, no daemon for basic usage.

The name is intentional. `prx` is shorter than `grep`, `cat`, `find`, `sed`, and `diff` combined.

---

## Core Subcommands

Priority order reflects agent usage frequency.

### `prx search` — replaces grep / rg

Hybrid search across three modes, fused into a single ranked result set:

- **Literal:** exact string and regex matching, same speed as ripgrep
- **Semantic:** static embeddings (256-dim, float16, embedded in binary) with BM25 + Reciprocal Rank Fusion. No external model server required.
- **Structural:** ast-grep patterns for language-aware matching (find all callers of a function, all implementations of an interface)

Output includes: match location, surrounding context, relevance score, file hash. Budget-aware: returns the highest-ranked results that fit within `--budget N` tokens.

### `prx read` — replaces cat / head / tail

Reads files with structural awareness:

- `--snap function` snaps the read window to the nearest enclosing function boundary
- `--skeleton` returns signatures only (no bodies), for fast symbol discovery
- `--outline` returns the full symbol table with line numbers
- Every response includes a content hash; agents can skip re-reads when the hash matches

Budget-aware: prioritizes the most relevant sections rather than truncating arbitrarily.

### `prx find` — replaces find / ls / tree

Filesystem traversal with agent-friendly output:

- Dual output modes: tree structure and flat list, in the same response
- Inline metadata: size, modification time, language, line count
- `.gitignore`-aware by default
- Semantic file relevance scoring when a query is provided (ranks files by likely relevance to the task)

### `prx edit` — replaces sed / awk

Structured file editing with safety defaults:

- Literal match by default (no accidental regex interpretation)
- Dry-run by default (shows diff, does not apply)
- Syntax validation via tree-sitter before writing
- `--in-function` scopes replacements to a named function
- Returns a structured diff of changes made, with content hashes before and after

### `prx diff` — replaces diff / git-diff

Diff output shaped for agent consumption:

- Semantic summaries: "function X was renamed, body unchanged"
- Function-level attribution: which logical unit each change belongs to
- Move detection: distinguishes refactors from deletions
- Budget-aware: summarizes large diffs rather than dumping raw hunks

---

## Utility Subcommands

| Subcommand | Purpose |
|---|---|
| `prx index` | Builds the local search index for a repo |
| `prx outline` | Returns the symbol table for a file or directory |
| `prx exists` | Bloom filter check: does this symbol/string exist anywhere in the repo? Sub-millisecond. |
| `prx mcp` | Starts an MCP server over stdio for direct agent integration |
| `prx stats` | Token savings dashboard: shows estimated tokens saved vs raw Unix tools |
| `prx batch` | Accepts a JSONL file of commands, executes them, returns JSONL results |

---

## Non-Functional Requirements

### Distribution

- Single static binary, approximately 47MB (includes float16 model weights)
- No runtime dependencies
- No internet required
- No daemon required for basic usage
- Zero-setup: download, run, works

### Platform Support

| Platform | Architectures |
|---|---|
| Linux | x86_64, aarch64 |
| macOS | x86_64, aarch64 |
| Windows | x86_64 |

### Output

- JSON or JSONL on all commands by default
- `--plain` flag for human-readable fallback
- Errors returned in stdout as structured JSON, never on stderr, never exit-code-only
- Content hashes on every response that includes file content

### Performance

- Sub-millisecond overhead over raw tools for literal operations
- `--budget N` on all content-returning commands (N = token count)
- Intelligent selection within budget, not arbitrary truncation

### Integration

- MCP server mode (`prx mcp`) for direct agent integration without shell subprocess overhead
- `prx batch` for high-throughput agent workflows

---

## Success Metrics

| Metric | Target |
|---|---|
| Token reduction vs grep+read loops | 60-90% (measured across benchmark tasks) |
| Semantic search quality (NDCG@10) | >= 0.85 |
| Index time for average repo | < 500ms |
| Query latency (p50) | < 5ms |
| Setup time from download to first query | 0 (no configuration required) |

---

## Design Principles

1. **One call = full answer.** Metadata, content, and context come back together. Agents don't make follow-up calls to get what they should have received the first time.

2. **Budget, don't truncate.** When output exceeds the token budget, select the highest-value content. Never cut off mid-result.

3. **Structure over compression.** Never generate wasteful output in the first place. A structured response is smaller than a human-readable one that an agent must parse.

4. **Errors in stdout, structured.** Agents don't read stderr. Exit codes alone carry no context. Every error is a JSON object with a code, message, and recovery hint.

5. **Content hashes everywhere.** Every response that includes file content includes a hash. Agents use hashes to skip re-reads. This alone eliminates a significant fraction of redundant tool calls.

6. **Dry-run by default for edits.** `prx edit` shows what it would do before doing it. Agents opt in to applying changes explicitly.

---

## Out of Scope (v1)

- External embeddings or vector databases
- LSP integration
- Daemon requirement for any feature
- AI or LLM components inside the tool itself
- IDE plugins or GUI
- Remote filesystem support
- Authentication or access control
