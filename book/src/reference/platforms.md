# Platform Support

prx is a single static binary with no runtime dependencies. It works on Linux, macOS, and Windows without installation, configuration, or internet access.

## Supported Targets

| Target | Tier | CI Runner |
|---|---|---|
| Linux x86_64 (glibc) | 1 | ubuntu-latest |
| Linux aarch64 (glibc) | 1 | ubuntu-latest (cross) |
| macOS aarch64 (Apple Silicon) | 1 | macos-latest |
| Windows x86_64 (MSVC) | 1 | windows-latest |
| macOS x86_64 (Intel) | 2 | macos-13 |
| Linux x86_64 (musl, static) | 2 | ubuntu-latest (cross) |

Tier 1 targets are tested on every commit. Tier 2 targets are tested on releases.

## Why Pure Rust (No ONNX, No Python)

The embedding model (potion-retrieval-32M) is embedded directly in the binary. Inference runs in pure Rust: tokenize, lookup, mean pool, normalize. About 50 lines of code.

The alternative was ONNX Runtime via the `ort` crate. That was rejected for two reasons:

1. ONNX Runtime 1.24.1 dropped x86_64 macOS support (a Microsoft decision), which would have eliminated Tier 2 Intel Mac coverage.
2. `ort` 2.0 requires pre-built ONNX Runtime binaries, adding a runtime dependency that breaks the "download one file, run it" promise.

Model2Vec inference is not a neural network in the transformer sense. There's no forward pass, no attention mechanism. It's a table lookup followed by averaging — fast enough on CPU, no GPU required.

## Dependency Audit

| Crate | Pure Rust? | Build Requirement | Platform Notes |
|---|---|---|---|
| clap | Yes | None | |
| tree-sitter | No | C compiler (cc crate) | Pinned to 0.25.x for grammar crate compatibility. Language grammars are C compiled into binary. All CI runners have C compilers. Windows needs MSVC or MinGW. |
| ast-grep-core | Yes | None | |
| safetensors | Yes | None | Zero-copy mmap |
| ndarray | Yes | None | BLAS optional, not used |
| sprs | Yes | None | Sparse matrices |
| tokenizers | Mostly | None | HuggingFace tokenizer, pure Rust |
| similar | Yes | None | Diff algorithms |
| bloomfilter | Yes | None | |
| serde + serde_json | Yes | None | |
| xxhash-rust | Yes | None | xxh3 feature |
| ignore | Yes | None | From ripgrep, battle-tested everywhere |
| regex | Yes | None | Literal search and identifier extraction |
| thiserror | Yes | None | |
| anyhow | Yes | None | |
| rmcp | Yes | None | Official MCP SDK. Stdio works on Windows via tokio |
| notify | Yes | None | Linux=inotify, macOS=FSEvents, Windows=ReadDirectoryChangesW |

The only non-pure-Rust dependency is tree-sitter, which requires a C compiler at build time. All CI runners have one. The compiled grammars are statically linked into the binary — no C runtime dependency at runtime.

## Tree-sitter Grammar Compatibility

All grammars are pinned to tree-sitter 0.25.x. This version was chosen because it has the broadest grammar crate compatibility — only 1 of 15 grammar crates supports 0.26.x, while all support 0.25.x.

Supported languages (15 grammars compiled into the binary):

Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C, C++, Ruby, Bash, JSON, TOML, YAML, HTML, CSS

Additional grammars can be added as crate dependencies. The grammar crate must be compatible with tree-sitter 0.25.x.

## Cross-Compilation

| From → To | Works? | Method |
|---|---|---|
| Linux x86_64 → Linux aarch64 | Yes | `cross build --target aarch64-unknown-linux-gnu` |
| Linux x86_64 → Windows | Yes | `cross build --target x86_64-pc-windows-gnu` |
| macOS → Linux | Yes | `cross build --target x86_64-unknown-linux-gnu` |
| macOS → Windows | No | Use GitHub Actions windows-latest runner |
| Any → musl (static) | Yes | `cross build --target x86_64-unknown-linux-musl` |

## Binary Size

| Configuration | Size |
|---|---|
| prx without model | ~15 MB |
| + potion-retrieval-32M float16 | +32 MB = ~47 MB |
| + LTO + strip | ~40 MB |

The model is embedded via `include_bytes!`. No download needed at runtime.

## CI Matrix

| Runner | Target |
|---|---|
| ubuntu-latest | x86_64-unknown-linux-gnu |
| ubuntu-latest (cross) | aarch64-unknown-linux-gnu |
| ubuntu-latest (cross) | x86_64-unknown-linux-musl |
| macos-latest | aarch64-apple-darwin |
| macos-13 | x86_64-apple-darwin |
| windows-latest | x86_64-pc-windows-msvc |

## Known Platform-Specific Behavior

**File watching (`prx index --watch`):** uses platform-native APIs. Linux uses inotify, macOS uses FSEvents, Windows uses ReadDirectoryChangesW. Behavior is consistent across platforms, but the underlying mechanism differs.

**Path separators:** prx normalizes path separators internally. JSON output always uses forward slashes, even on Windows.

**Binary files:** prx skips files with a null byte in the first 8KB. This heuristic works on all platforms.

**Large files:** files over 1MB are skipped by default. Override with `PRX_MAX_FILE_SIZE` environment variable.
