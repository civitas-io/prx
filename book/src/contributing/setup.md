# Developer Setup

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | >= 1.85 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| C compiler | gcc, clang, or MSVC | Required by tree-sitter grammars at build time |
| Git | >= 2.x | For `prx diff` and `--changed-since` |
| Python | >= 3.10 | For model conversion script (float32 → float16) |

### Platform-Specific Setup

**macOS:**
```bash
xcode-select --install
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt install build-essential python3
```

**Windows:**
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

## Quick Start

```bash
git clone https://github.com/civitas-io/prx.git
cd prx
make setup
```

`make setup` downloads the model files (~35MB), converts the model to float16, and runs a test build. Takes about 2 minutes on first run.

## What `make setup` Does

1. Downloads three files into `models/` (gitignored):
   - `potion-retrieval-32M.safetensors` — Model2Vec embedding weights (61MB float32 from HuggingFace, converted to float16)
   - `model2vec_tokenizer.json` — Model2Vec vocabulary (1MB, 61,826 tokens)
   - `cl100k_base.json` — cl100k tokenizer for `--budget` enforcement (4MB)
2. Converts the model from float32 to float16 (61MB → 31MB)
3. Builds the debug binary
4. Runs unit tests to verify everything works

The model files are embedded into the binary at compile time via `include_bytes!`. They must be present before `cargo build`. The `models/` directory is gitignored because the files are too large for git.

## Build

```bash
make build          # debug build (~160MB, fast compile)
make release        # release build (~48MB, slow compile, optimized)
```

### Build Variants

```bash
# Without MCP server (drops tokio + rmcp, faster compile)
cargo build --no-default-features

# With MCP server (default)
cargo build

# With file watching for prx index --watch
cargo build --features watch
```

### Build Without Model

If you're working on commands that don't use semantic search (edit, diff, run, stats, init), you can skip the model download:

```bash
mkdir -p models
touch models/potion-retrieval-32M.safetensors
touch models/model2vec_tokenizer.json
touch models/cl100k_base.json
cargo build --no-default-features
```

The binary compiles but `prx search --semantic` won't produce meaningful results.

## Development Workflow

### Daily Commands

```bash
make check          # fmt + clippy + all tests (run before every commit)
make test           # all tests (unit + E2E)
make test-unit      # unit tests only (fast, ~1s)
make test-e2e       # E2E tests only (slower, ~3s, tests the compiled binary)
```

### Running Individual Tests

```bash
cargo test test_literal_search              # by test name
cargo test commands::search                 # by module
cargo test --test e2e search                # E2E tests matching "search"
```

### Debug Logging

```bash
RUST_LOG=prx=debug cargo run -- search "test" src/
```

Log output goes to stderr. stdout is reserved for JSON output.

## Pre-Commit Hook

Install the pre-commit hook to run `make check` automatically before every commit:

```bash
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The hook runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`. All three must pass before the commit proceeds.

## IDE Setup

**rust-analyzer** works out of the box. No special configuration needed.

For VS Code, install the `rust-analyzer` extension. For IntelliJ/CLion, install the Rust plugin.

One note: the model files in `models/` are large binary files. Some IDEs index everything in the project directory. Add `models/` to your IDE's exclusion list if indexing is slow.

## Adding a New Command

1. Create `src/commands/new_cmd.rs` with an Args struct and `run()` function
2. Add the variant to `Commands` enum in `src/commands/mod.rs`
3. Add dispatch arm in `src/main.rs`
4. Add `name()` match in `src/commands/mod.rs`
5. Write unit tests in the module
6. Write E2E tests in `tests/e2e.rs`
7. Update `docs/design/CLI.md`, `docs/design/OUTPUT.md`, and `AGENTS.md`

## Adding a New Language Grammar

1. Add `tree-sitter-<lang>` crate to `Cargo.toml` (must provide `LanguageFn` compatible with tree-sitter 0.26.x)
2. Add extension mapping in `src/parsing/languages.rs`
3. Add outline test in `src/parsing/outline.rs`

## Adding a New Run Parser

1. Create `src/runner/new_tool.rs` implementing `pub fn parse(output: &str) -> ParsedResult`
2. Add module in `src/runner/mod.rs`
3. Add detection pattern in `detect_tool()` (more specific patterns before general ones)
4. Add dispatch in `parse_output()`
5. Add tests with real captured output

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. `make check`
4. `git commit`
5. `git tag v0.X.0`
6. `git push && git push --tags`
7. GitHub Actions builds release binaries automatically for all 6 targets
