# Agent Integration

prx supports three integration tiers. They're not mutually exclusive. Most setups use all three.

## Integration tiers

| Tier | How | Best for |
|---|---|---|
| **CLI on PATH** | `prx search ...` in bash | Any agent, CI, scripts, sub-agents |
| **MCP server** | `prx mcp` | Top-level agents that prefer typed tool calls |
| **Agent definition** | `prx init --agent claude-code` | A dedicated retrieval sub-agent |

### Tier 1: CLI on PATH

Install the binary and add prx commands to your project's AGENTS.md or CLAUDE.md. This is the most portable path. It works for top-level agents, sub-agents, scripts, CI, and humans.

```bash
prx init --agents-md    # appends a usage snippet to AGENTS.md
```

Sub-agents in Claude Code and Codex CLI cannot call MCP tools. CLI on PATH is the only option for sub-agents.

### Tier 2: MCP server

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

The MCP server exposes prx over stdio with typed parameters and auto-discovery. Works with Claude Code, Cursor, Codex, and OpenCode.

**Limitation:** sub-agents cannot call MCP tools. If you're building a multi-agent system, use CLI on PATH for any agent that runs as a sub-agent.

### Tier 3: Agent definition

```bash
prx init --agent claude-code
```

Writes `.claude/agents/prx-search.md`, creating a dedicated sub-agent with optimized workflow guidance. The sub-agent uses prx via bash (Tier 1), not MCP.

## Per-framework config

### Claude Code

MCP config in `.claude/settings.json`:

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

Or generate a sub-agent definition:

```bash
prx init --agent claude-code
```

### Cursor

MCP config in `.cursor/mcp.json`:

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

### Codex CLI

Add to your Codex config:

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

Note: Codex sub-agents cannot call MCP. Use CLI on PATH for sub-agent access.

### OpenCode

Add to `opencode.json`:

```json
{
  "mcp": {
    "servers": {
      "prx": {
        "command": "prx",
        "args": ["mcp"]
      }
    }
  }
}
```

## Auto-detect all frameworks

```bash
prx init
```

Detects which frameworks are present in your project and writes all relevant configs in one pass.

## AGENTS.md snippet

For any agent that reads an AGENTS.md or CLAUDE.md, the most effective integration is a usage snippet that tells the agent when and how to use prx. Run:

```bash
prx init --agents-md
```

This appends a concise reference to your project's AGENTS.md covering the core workflow, command substitution table, and output format.

## Output format

All prx commands return the same JSON envelope regardless of integration tier:

```json
{
  "version": "0.3.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": { ... }
}
```

Errors are also JSON on stdout, never stderr:

```json
{
  "status": "error",
  "error": {
    "code": "file_not_found",
    "message": "File not found: src/missing.ts",
    "suggestion": "Use `prx find` to discover files."
  }
}
```

Use `--plain` for human-readable terminal output.

## Reliability and fallback

If an internal operation fails, prx falls back to the equivalent Unix command and returns results in the same JSON envelope, flagged so the caller can tell a fallback occurred. Errors are logged to `~/.prx/errors.jsonl`. The intent is that prx never hard-breaks an agent's workflow.

Because a fallback silently trades semantic search for plain matching, agents that depend on retrieval quality should check the `fallback` flag in the response rather than assume every result is a full-quality prx result.
