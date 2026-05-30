# Installation

## Prebuilt binary (recommended)

Download the binary for your platform from [GitHub Releases](https://github.com/civitas-io/prx/releases). The prebuilt binary already contains the embedded model. Nothing else to install.

| Platform | File |
|---|---|
| Linux x86_64 | `prx-x86_64-unknown-linux-gnu.tar.gz` |
| Linux aarch64 | `prx-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `prx-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `prx-x86_64-pc-windows-msvc.zip` |

```bash
# Linux x86_64
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv prx /usr/local/bin/
prx --version

# macOS Apple Silicon
curl -L https://github.com/civitas-io/prx/releases/latest/download/prx-aarch64-apple-darwin.tar.gz | tar xz
sudo mv prx /usr/local/bin/
prx --version
```

## Build from source

Requirements: Rust 1.85 or later, a C compiler (for tree-sitter grammars), and network access on first build. The build script downloads model weights automatically.

```bash
git clone https://github.com/civitas-io/prx.git
cd prx
cargo build --release
```

First build takes 1-2 minutes: model download (~35 MB), float16 conversion, compilation. Subsequent builds are fast. The model weights are baked into the binary via `include_bytes!`. No downloads at runtime.

For offline or air-gapped builds, set `PRX_MODELS_DIR` to point to pre-downloaded weights:

```bash
PRX_MODELS_DIR=/path/to/weights cargo build --release
```

## cargo install

```bash
cargo install prx
```

## Auto-setup

After installing, run `prx init` to detect your agent framework and generate integration configs automatically:

```bash
prx init
```

This writes config files for Claude Code, Cursor, Codex, or OpenCode depending on what it finds in your project. Use `--agents-md` to append a usage snippet to your project's AGENTS.md:

```bash
prx init --agents-md
```

## MCP server setup

To use prx as an MCP server (for agents that support the Model Context Protocol), add this to your agent's config:

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

The `prx` binary must be on PATH. The MCP server exposes all prx commands as typed tool calls over stdio.

For Claude Code specifically, this goes in `.claude/settings.json` or your global Claude config. For Cursor, it goes in `.cursor/mcp.json`. For OpenCode, it goes in `opencode.json`.

See [Agent Integration](integration.md) for per-framework config snippets and guidance on when to use MCP vs CLI.

## Verifying the install

```bash
prx --version
prx search "hello" .
```

If the second command returns JSON with a `data.matches` array, the binary and embedded model are working correctly.
