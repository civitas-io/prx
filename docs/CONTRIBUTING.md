# Contributing to ag

Developer setup guide, build instructions, and workflow for contributing.

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | >= 1.85 (edition 2024) | https://rustup.rs |
| C compiler | gcc, clang, or MSVC | Required by tree-sitter grammar crates |
| Git | >= 2.x | For prx diff and --changed-since |
| Python | >= 3.10 (optional) | Only for benchmark scripts |

### Platform-Specific Notes

**macOS**: Xcode Command Line Tools provides the C compiler.
```bash
xcode-select --install
```

**Linux**: Install build-essential (Debian/Ubuntu) or base-devel (Arch).
```bash
sudo apt install build-essential    # Debian/Ubuntu
sudo pacman -S base-devel           # Arch
```

**Windows**: Install Visual Studio Build Tools with "Desktop development with C++".
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

---

## Setup

### Clone and Build

```bash
git clone https://github.com/civitas-io/prx.git
cd ag

# Debug build (fast compilation, slow binary)
cargo build

# Release build (slow compilation, optimized binary)
cargo build --release

# Verify
./target/debug/prx --version
```

### Model Weights

The embedding model (potion-code-16M) must be present at `models/potion-code-16M.safetensors` before building. It is embedded into the binary at compile time via `include_bytes!`.

```bash
# Download from HuggingFace (one-time, ~32MB for float16)
curl -L https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors \
  -o models/potion-code-16M.safetensors
```

The cl100k_base tokenizer config must also be present:

```bash
# Download tokenizer vocabulary (~2MB)
curl -L https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json \
  -o models/cl100k_base.json
```

These files are gitignored (too large for the repo). CI downloads them as a build step. A `Makefile` target automates this:

```bash
make models    # downloads both files if missing
```

### Build Without MCP

To build without the MCP server (drops tokio dependency, smaller binary):

```bash
cargo build --no-default-features
```

### Build Without Model (literal/structural search only)

For development on non-search subsystems, you can skip the model download. The binary will compile but `prx search --semantic` will return an error.

```bash
# Use an empty placeholder
touch models/potion-code-16M.safetensors
touch models/cl100k_base.json
cargo build
```

---

## Development Workflow

### Daily Commands

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Run all tests
cargo test

# Run specific test
cargo test test_literal_search

# Run integration tests only
cargo test --test integration

# Run with debug logging
RUST_LOG=ag=debug cargo run -- search "test" src/
```

### Pre-Commit Checklist

Before every commit:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Or use the Makefile:

```bash
make check    # runs fmt, clippy, test
```

### Recommended IDE Setup

**VS Code / Cursor / Zed**: install rust-analyzer. No additional configuration needed. The Cargo.toml is at the project root.

**IntelliJ / RustRover**: open the project root. The IDE auto-detects Cargo.toml.

---

## Running Tests

### Unit Tests

Inline in each module. Run all:

```bash
cargo test --lib
```

### Integration Tests

Test the compiled binary end-to-end:

```bash
cargo test --test '*'
```

Integration tests use `assert_cmd` to invoke the `prx` binary and `predicates` to assert on output. Test fixtures live in `tests/fixtures/`.

### Single Integration Test

```bash
cargo test --test test_search -- test_literal_search
```

### Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench --bench search

# Run with HTML report
cargo bench -- --output-format=bencher
```

Benchmark results are written to `target/criterion/`.

### Coverage

```bash
# Install coverage tool
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out html --output-dir target/coverage

# View report
open target/coverage/tarpaulin-report.html
```

Target: >= 80% line coverage.

---

## Project Layout (for contributors)

```
ag/
├── src/
│   ├── main.rs              # Entry point: clap parse + dispatch
│   ├── lib.rs               # Public API surface
│   ├── output.rs            # JSON envelope serialization
│   ├── tokens.rs            # Token counting (fast + exact)
│   ├── hash.rs              # Content hashing (xxh3)
│   ├── walk.rs              # File walking (ignore crate)
│   ├── commands/             # One file per subcommand
│   ├── search/               # Search engine (fusion, semantic, literal, structural)
│   ├── chunking/             # Tree-sitter code chunking
│   ├── ranking/              # Reranking pipeline (boosts + penalties)
│   ├── index/                # Index management (dense, sparse, bloom)
│   └── parsing/              # Tree-sitter integration (languages, outline, snap)
├── models/                   # Embedding model + tokenizer (gitignored)
├── tests/
│   ├── integration/          # CLI end-to-end tests
│   └── fixtures/             # Sample source files
├── benches/                  # Criterion benchmarks
└── docs/                     # All documentation
```

See AGENTS.md for the full layout with file-level descriptions.

---

## Adding a New Subcommand

1. Add variant to `Commands` enum in `src/main.rs`
2. Create args struct in `src/commands/new_cmd.rs`
3. Create handler function that returns `Result<Box<dyn Serialize>, AgError>`
4. Add dispatch arm in `main.rs`
5. Define output schema in `docs/design/OUTPUT.md`
6. Add CLI flags in `docs/design/CLI.md`
7. Write unit tests in the module
8. Write integration test in `tests/integration/test_new_cmd.rs`
9. Update AGENTS.md repo layout

---

## Adding a New Language Grammar

1. Add `tree-sitter-<lang>` crate to `Cargo.toml` (verify 0.25.x compat)
2. Add extension mapping in `src/parsing/languages.rs`
3. Create `src/parsing/queries/<lang>_symbols.scm` for outline extraction
4. Create `src/parsing/queries/<lang>_definitions.scm` for search ranking
5. Add test fixture `tests/fixtures/sample.<ext>`
6. Add outline unit test for the new language
7. Update AGENTS.md and CRATE-REFERENCE.md

---

## Makefile

```makefile
.PHONY: check build test bench models clean

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test

build:
	cargo build --release

test:
	cargo test

bench:
	cargo bench

models:
	@mkdir -p models
	@test -f models/potion-code-16M.safetensors || \
		curl -L https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors \
		-o models/potion-code-16M.safetensors
	@test -f models/cl100k_base.json || \
		curl -L https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json \
		-o models/cl100k_base.json

clean:
	cargo clean
	rm -rf target/criterion
```

---

## Debugging

### Debug Logging

```bash
RUST_LOG=ag=debug cargo run -- search "test" src/
RUST_LOG=ag::search=trace cargo run -- search --semantic "auth" src/
```

Log output goes to stderr (never stdout, which is reserved for JSON output).

### Common Issues

**"model file not found"**: Run `make models` to download the embedding model and tokenizer.

**"C compiler not found"**: Tree-sitter grammar crates require a C compiler at build time. See Prerequisites above.

**"cargo build slow"**: First build compiles tree-sitter grammars (C code). Subsequent builds use cached artifacts. Use `cargo build` (debug) for development, `cargo build --release` only for final testing.

**"tests fail on Windows"**: Check line endings. Git may convert LF to CRLF in test fixtures. Add to `.gitattributes`:
```
tests/fixtures/** -text
```

---

## Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Run full check: `make check`
4. Run benchmarks: `make bench`
5. Tag: `git tag v0.1.0`
6. Push: `git push --tags`
7. CI builds release binaries for all platforms
8. Create GitHub release with binaries
