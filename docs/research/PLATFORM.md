# Cross-Platform Compatibility Audit

**May 2026**

---

## Supported Targets

| Target | Tier | CI Runner |
|---|---|---|
| Linux x86_64 (glibc) | 1 | ubuntu-latest |
| Linux aarch64 (glibc) | 1 | ubuntu-latest (cross) |
| macOS aarch64 (Apple Silicon) | 1 | macos-latest |
| macOS x86_64 (Intel) | 2 | macos-13 |
| Windows x86_64 (MSVC) | 1 | windows-latest |
| Linux x86_64 (musl, static) | 2 | ubuntu-latest (cross) |

---

## Dependency Audit

| Crate | Version | Pure Rust? | Build Requirement | Platform Notes |
|---|---|---|---|---|
| clap | 4.6 | Yes | None | |
| tree-sitter | 0.25 | No | C compiler (cc crate) | Pinned to 0.25.x for grammar crate compatibility. Language grammars are C compiled into binary. All CI runners have C compilers. Windows needs MSVC or MinGW. |
| ast-grep-core | 0.42 | Yes | None | |
| safetensors | 0.7 | Yes | None | Zero-copy mmap |
| ndarray | 0.17 | Yes | None | BLAS optional, not used |
| sprs | 0.11 | Yes | None | Sparse matrices |
| tokenizers | 0.23 | Mostly | None | HuggingFace tokenizer, pure Rust |
| similar | 3.1 | Yes | None | Diff algorithms |
| bloomfilter | 3.0 | Yes | None | |
| serde + serde_json | 1.x | Yes | None | |
| xxhash-rust | 0.8 | Yes | None | xxh3 feature |
| ignore | 0.4 | Yes | None | From ripgrep, battle-tested everywhere |
| regex | 1.x | Yes | None | Literal search and identifier extraction |
| thiserror | 2.0 | Yes | None | |
| anyhow | 1.0 | Yes | None | |
| rmcp | 1.x | Yes | None | Official MCP SDK. Stdio works on Windows via tokio |
| notify | 9.x | Yes | None | Linux=inotify, macOS=FSEvents, Windows=ReadDirectoryChangesW |

---

## Critical Decision: Why NOT ort (ONNX Runtime)

`ort` 2.0-rc.12 requires pre-built ONNX Runtime binaries. ONNX Runtime 1.24.1 dropped x86_64 macOS support (Microsoft decision), which would eliminate Tier 2 Intel Mac coverage.

Model2Vec inference is not a neural network. The full pipeline is: tokenize, lookup, mean pool, normalize. This reimplements cleanly in pure Rust using `safetensors` to load weights and `ndarray` for matrix ops, roughly 50 lines of code.

Result: zero external binary dependencies, works on every platform.

---

## Cross-Compilation

| From -> To | Works? | Method |
|---|---|---|
| Linux x86_64 -> Linux aarch64 | Yes | `cross build --target aarch64-unknown-linux-gnu` |
| Linux x86_64 -> Windows | Yes | `cross build --target x86_64-pc-windows-gnu` |
| macOS -> Linux | Yes | `cross build --target x86_64-unknown-linux-gnu` |
| macOS -> Windows | No | Use GitHub Actions windows-latest runner |
| Any -> musl (static) | Yes | `cross build --target x86_64-unknown-linux-musl` |

---

## Binary Size Estimates

| Configuration | Size |
|---|---|
| prx without model | ~15 MB |
| + potion-code-16M float16 | +32 MB = ~47 MB |
| + LTO + strip | ~40 MB |

Model is embedded via `include_bytes!`, no download needed at runtime.

---

## CI Matrix (GitHub Actions)

| Runner | Target |
|---|---|
| ubuntu-latest | x86_64-unknown-linux-gnu |
| ubuntu-latest (cross) | aarch64-unknown-linux-gnu |
| ubuntu-latest (cross) | x86_64-unknown-linux-musl |
| macos-latest | aarch64-apple-darwin |
| macos-13 | x86_64-apple-darwin |
| windows-latest | x86_64-pc-windows-msvc |
