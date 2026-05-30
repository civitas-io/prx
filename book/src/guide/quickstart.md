# Quick Start

Get prx working in five minutes.

## Install

Download the binary for your platform from [GitHub Releases](https://github.com/civitas-io/prx/releases) and put it on your PATH:

```bash
# Linux x86_64
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv prx /usr/local/bin/

# Verify
prx --version
```

The binary already contains the embedded model. Nothing else to install.

Full installation options (macOS, Windows, build from source): [Installation](installation.md).

## Your first search

```bash
prx search "authentication flow" src/
```

prx auto-detects that this is a natural language query and runs semantic search. The result is ranked JSON with relevance scores and token counts:

```json
{
  "tokens": 487,
  "data": {
    "matches": [
      {
        "file": "src/auth/handler.ts",
        "line": 42,
        "context_name": "handleLogin",
        "snippet": "async handleLogin(req: Request)...",
        "relevance": 0.94
      }
    ],
    "total_matches": 23,
    "returned": 3
  }
}
```

For exact matches, use `--literal`. For AST patterns, use `--structural`:

```bash
prx search --literal "authenticate(" src/
prx search --structural 'fn $NAME($$$) { $$$ }' src/
```

## Read a file efficiently

Don't `cat` a whole file when you only need its shape:

```bash
# Signatures only — about 10% of the tokens of a full read
prx read src/auth/handler.ts --skeleton

# Read just the function at line 42
prx read src/auth/handler.ts --lines 42 --snap function

# Full file with metadata and symbol outline
prx read src/auth/handler.ts
```

Every read response includes a `meta.hash`. Pass it back on the next read to skip re-reading unchanged files:

```bash
# First read — note the hash in meta.hash
prx read src/auth/handler.ts

# Subsequent reads — returns a 50-byte stub if nothing changed
prx read src/auth/handler.ts --if-changed a3f9b2c1...
```

## Understand a module

Instead of running `find`, then reading each file, then chasing imports:

```bash
prx context src/auth/
```

Returns stats, documentation, top entrypoints ranked by reference count, per-file skeletons, and the 1-hop import graph. One call, one response.

## Check impact before changing

Before touching a file, see what depends on it:

```bash
prx impact src/auth/handler.ts
```

Returns a list of dependent files with hop distance and which symbols they use.

## Make a safe edit

```bash
# Preview the change (dry-run by default)
prx edit src/auth/handler.ts --find "old_api()" --replace "new_api()"

# Apply it
prx edit src/auth/handler.ts --find "old_api()" --replace "new_api()" --apply
```

## Run tests without the noise

```bash
prx run cargo test
```

A 164-test suite that outputs ~1,200 tokens raw becomes ~15 tokens through prx. Only failures are returned. Passing tests are omitted.

## The full workflow in order

This is the recommended sequence for any coding task:

```bash
# 1. Quick existence check before committing to a search
prx exists "authenticate" src/

# 2. Find relevant code
prx search "authentication flow" src/

# 3. Understand the module
prx context src/auth/

# 4. Read structure before content
prx read src/auth/handler.ts --skeleton

# 5. Read specific functions
prx read src/auth/handler.ts --lines 42 --snap function

# 6. Check what depends on the file you're about to change
prx impact src/auth/handler.ts

# 7. Preview and apply the edit
prx edit src/auth/handler.ts --find "old_api()" --replace "new_api()"
prx edit src/auth/handler.ts --find "old_api()" --replace "new_api()" --apply

# 8. Verify with minimal output
prx run cargo test

# 9. Build a persistent index for faster repeated searches
prx index .
```

## Output format

Every command returns the same JSON envelope:

```json
{
  "version": "0.3.0",
  "command": "search",
  "status": "ok",
  "tokens": 487,
  "data": { ... }
}
```

Use `--plain` for human-readable terminal output. Use `--budget N` to cap token usage on any command.

## Next steps

- [Installation](installation.md) — all platforms, build from source, MCP setup
- [Agent Integration](integration.md) — connect prx to Claude Code, Cursor, Codex, OpenCode
- [Token Savings](token-savings.md) — measured data on what you actually save
- [Commands](../commands/search.md) — full reference for every command
