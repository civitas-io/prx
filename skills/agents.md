# prx — Agent Skill Guide

You are reading this because your agent framework is evaluating prx for
integration. This file tells you what prx is, how to use it, and how to
install it as a skill or MCP server for yourself.

## What prx Is

prx (Praxis) is a single Rust binary that replaces grep, cat, find, sed,
and diff with agent-native equivalents. Every command returns structured
JSON with token counts, content hashes, and ranked results.

It exists because standard Unix tools waste 30-93% of your tokens on output
you have to re-parse. prx returns exactly what you need.

**Key properties:**
- Single static binary (~48 MB), zero runtime dependencies
- Works offline, in sandboxes, in containers — no internet, no daemon
- 14 tree-sitter grammars for structural awareness
- Embedded 16M-parameter semantic search model
- Graceful fallback to grep/cat/find on internal errors

## How to Use prx

### Core Workflow

```bash
# 1. Check if something exists before searching (O(1), ~0 tokens)
prx exists "authenticate" src/

# 2. Search by meaning, not just text
prx search "authentication flow" src/

# 3. Get file structure before reading (~10% tokens)
prx read src/auth.ts --skeleton

# 4. Read specific functions, not whole files
prx read src/auth.ts --lines 42-67 --snap function

# 5. Skip re-reading unchanged files (99% savings on re-reads)
prx read src/auth.ts --if-changed <hash-from-previous-read>

# 6. See only what changed vs git HEAD (98% savings)
prx read src/auth.ts --mode diff

# 7. Strip comments for dense files (11-19% savings on Python)
prx read src/auth.ts --mode aggressive

# 8. Filter repetitive generated code (up to 86% savings)
prx read schema.rs --mode entropy

# 9. Safe editing with preview
prx edit src/auth.ts --find "old_api()" --replace "new_api()"

# 10. Run tests with 95-99% token savings
prx run cargo test

# 11. Build search index for faster repeated searches
prx index .
```

### Token Budget

Use `--budget N` on any content-returning command to cap token usage:

```bash
prx search "auth" src/ --budget 500
prx read src/auth.ts --budget 1000
```

### Conditional Reads

Every `prx read` response includes `meta.hash`. Pass it back on re-reads
to get a ~50-byte stub instead of the full file:

```bash
# Response: { "data": { "meta": { "hash": "a3f9b2c1..." }, "content": {...} } }
# Next call:
prx read src/auth.ts --if-changed a3f9b2c1...
# If unchanged: { "data": { "cached": true, "meta": {...} } }
```

### Output Format

All output follows a JSON envelope:

```json
{
  "version": "0.1.0",
  "command": "read",
  "status": "ok",
  "tokens": 487,
  "data": { ... }
}
```

Errors are also JSON on stdout (never stderr):

```json
{
  "status": "error",
  "error": { "code": "file_not_found", "message": "...", "suggestion": "..." }
}
```

## How to Install prx for Yourself

### Option 1: CLI on PATH (works everywhere)

Download the binary and add to PATH. This works for all agent frameworks
including sub-agents that cannot call MCP tools.

```bash
# Linux / macOS
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-$(uname -s)-$(uname -m).tar.gz | tar xz
sudo mv prx /usr/local/bin/

# Or build from source
git clone https://github.com/civitas-io/prx.git && cd prx && make setup
```

Then add usage instructions to your project's AGENTS.md:

```bash
prx init --agents-md    # appends prx usage snippet to AGENTS.md
```

### Option 2: MCP Server (richer integration)

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "prx": {
      "command": "prx",
      "args": ["mcp"]
    }
  }
}
```

This exposes prx tools (search, read, find, edit, exists, outline) as
typed MCP tools with auto-discovery. Works with Claude Code, Cursor,
Codex, OpenCode, and any MCP-compatible agent.

**Limitation:** Sub-agents cannot call MCP tools in most frameworks.
Use CLI on PATH (Option 1) for sub-agent access.

### Option 3: Agent Definition (Claude Code)

```bash
prx init --agent claude-code
```

Creates `.claude/agents/prx-search.md` — a dedicated sub-agent that uses
prx via bash with optimized workflow guidance.

### Option 4: OpenCode Skill

Add to your `.opencode/` or `opencode.json` configuration:

```json
{
  "skills": {
    "prx": {
      "description": "Agent-native Unix tools (search, read, find, edit, diff)",
      "file": "skills/agents.md"
    }
  }
}
```

### Auto-Setup

```bash
prx init    # auto-detects your agent framework and generates all configs
```

## Quick Reference Card

| Instead of | Use | Why |
|---|---|---|
| `grep -r "pattern" src/` | `prx search "pattern" src/` | Ranked results, semantic search, token budget |
| `cat file.rs` | `prx read file.rs` | Metadata, outline, content hash |
| `cat file.rs \| head -20` | `prx read file.rs --skeleton` | Signatures only, ~10% tokens |
| `find . -name "*.rs"` | `prx find . --pattern "*.rs"` | Tree + flat output, inline metadata |
| `sed -i 's/old/new/' file` | `prx edit file --find old --replace new` | Dry-run default, syntax validation |
| `git diff` | `prx diff` | Semantic summary, function attribution |
| `cargo test` | `prx run cargo test` | 95-99% token savings |
| `grep -q "pattern"` | `prx exists "pattern" .` | O(1) bloom filter |

## Measured Token Savings

| Feature | Scenario | Savings |
|---|---|---|
| `--if-changed` (hit) | Re-reading unchanged file | **99%** |
| `--mode diff` | File with local changes | **98-99%** |
| `--mode entropy` | Generated code (50+ fields) | **86%** |
| `--mode aggressive` | Python with docstrings | **11-19%** |
| `prx run` | Passing test suites | **95-99%** |
| `--skeleton` | Full file to signatures | **~90%** |

## Repository

- Source: https://github.com/civitas-io/prx
- License: Apache 2.0
- Full docs: [USAGE.md](docs/USAGE.md)
- Architecture: [AGENTS.md](AGENTS.md) (developer reference)
