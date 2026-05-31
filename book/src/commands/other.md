# Other Commands

Briefer coverage of the remaining commands: `batch`, `stats`, `bench`, `bench-ndcg`, `init`, and `mcp`.

## batch

Execute multiple commands in parallel via JSONL on stdin. One round-trip instead of N.

```bash
echo '{"cmd":"read","file":"src/auth.ts","skeleton":true}
{"cmd":"exists","pattern":"redis","path":"src/"}' | prx batch
```

Each line of input is a JSON object with a `cmd` field and command-specific parameters. Results are returned as a JSONL stream, one result per input line.

Use `prx batch` when you have multiple independent queries to run. It's more efficient than running them sequentially because they execute in parallel.

## stats

Token-savings dashboard. Shows how much prx has saved across recorded calls.

```bash
prx stats                  # total savings
prx stats --compare        # per-command breakdown
```

Example output:

```json
{
  "data": {
    "total_calls": 200,
    "total_tokens_saved": 36114,
    "by_command": {
      "search": { "calls": 56, "savings_pct": 34.9 },
      "read":   { "calls": 24, "savings_pct": 46.3 },
      "run":    { "calls": 13, "savings_pct": 52.9 }
    }
  }
}
```

## bench

Synthetic benchmark comparing prx vs grep+cat on your codebase.

```bash
prx bench .
```

Runs a set of representative queries against your codebase using both prx and the equivalent Unix commands, then reports token counts side by side.

## bench-ndcg

NDCG@10 search quality benchmark against labeled datasets.

```bash
prx bench-ndcg dataset.json
prx bench-ndcg dataset.json --plain    # human-readable output
```

Loads the index once and runs all queries against cached data. A 50-query suite runs in 0.23 seconds (55x faster than the previous per-query approach).

See [Public Benchmark Suite](../performance/benchmarks.md) for methodology and the standard 200-query dataset.

## init

Detects agent frameworks in your project and generates integration configs.

```bash
prx init                      # detect frameworks, generate all configs
prx init --agents-md          # append usage snippet to AGENTS.md
prx init --agent claude-code  # generate a Claude Code sub-agent definition
```

`prx init` looks for `.claude/`, `.cursor/`, `opencode.json`, and other framework markers. For each framework it finds, it writes the appropriate config file.

## mcp

Starts prx as an MCP server over stdio.

```bash
prx mcp
```

You don't invoke this directly. It's the command your agent framework calls when it starts the MCP server. Add it to your framework's MCP config:

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

The MCP server exposes all prx commands as typed tool calls. See [Agent Integration](../guide/integration.md) for per-framework setup.
