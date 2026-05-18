HANDOFF CONTEXT
===============

USER REQUESTS (AS-IS)
---------------------
- "OSS Project Idea - There are several linux tools - binutils, etc - that Coding agents use - like grep, cat, etc. Most of these were built with humans in mind. If you (a coding agent) were building such tools, what would it look like - something that retrieves answers with least amount of tokens, etc. Take top 5 examples of unix tools that coding agents use the most and lets iterate on them. Ideas only"
- "Can we take implementation ideas from it [Semble] and customize it for ag busybox. We can extend busybox with all manner of agentic centric tools."
- "No. Embed in binary. We cant always assume that the env where the tools are being used will have internet access - for sandboxing and security."
- "Create a repo folder here with the appropriate folder tree. We will start with docs first."
- "I want to you incorporate this skill into our agents.md/claude.md -> https://github.com/multica-ai/andrej-karpathy-skills"
- "Ok. Now make a detailed implementation plan and testing plan as well."
- "I want the implementation to begin. What tools must be installed before you can work?"
- "can you cover the commands/search.rs test cases?"

GOAL
----
Continue implementing Phase 1 of ag: hybrid search (semantic + BM25 + RRF fusion), reranking pipeline, prx read, prx find, prx exists, and prx outline.

WORK COMPLETED
--------------
- I researched the competitive landscape exhaustively: Semble, RTK, Hypergrep, aict, instant-grep, LeanCTX, squeez, FileSift, SWE-agent ACI
- I studied Semble's internals deeply: chunking algorithm, Model2Vec inference, BM25 with compound identifier tokenization, RRF fusion (k=60), full reranking pipeline (definition boost 3x, stem matching, file coherence 0.2x, noise penalties, saturation decay 0.5^n)
- I designed ag as a single Rust binary (busybox-style) with 12 subcommands: search, read, find, edit, diff, index, outline, exists, batch, stats, mcp, init
- I wrote 21 documentation files totaling ~4,500 lines: PRD, ROADMAP, SYSTEM.md, SEARCH.md, CLI.md, OUTPUT.md, BENCHMARKS.md, SYSTEM-DESIGN.md, IMPLEMENTATION.md, TESTING.md, CRATE-REFERENCE.md, LANDSCAPE.md, PLATFORM.md, AGENTS.md, CLAUDE.md, README.md, CONTRIBUTING.md, CHANGELOG.md, Makefile, .gitignore, architecture.svg
- I verified all 14 tree-sitter grammar crates work with tree-sitter 0.26 (the earlier librarian report saying they were incompatible was wrong -- tree-sitter-language bridge crate resolves the conflict)
- I implemented Phase 0 Foundation completely:
  - Step 0.1: Project scaffold with clap derive, Commands enum, 12 subcommand stubs
  - Step 0.2: Output envelope (src/output.rs) -- Envelope<T>, ErrorEnvelope, AgError with 7 variants
  - Step 0.3: Content hashing (src/hash.rs) -- xxh3_128, 6 tests
  - Step 0.4: File walking (src/walk.rs) -- ignore crate wrapper with .prxignore, binary skip, max size, 6 tests
  - Step 0.5: Token counting (src/tokens.rs) -- fast (len/4) + exact (cl100k_base lazy load), 5 tests
  - Step 0.6: Tree-sitter parsing (src/parsing/) -- languages.rs (14 grammars), outline.rs (symbol extraction), snap.rs (structural snapping), 21 tests
  - Step 0.7: Chunking (src/chunking/) -- tree-sitter AST chunking ported from Semble, gap-filling for contiguous chunks, line-based fallback, 12 tests
  - Step 0.8: Model2Vec embeddings (src/index/dense.rs) -- DenseIndex with embed_text, index_chunks, cosine search, 8 tests
  - Step 0.9: BM25 index (src/index/sparse.rs + src/search/tokenize.rs) -- identifier tokenizer with camelCase/snake_case splitting, BM25 scoring in CSC sparse matrix, content enrichment, 15 tests
  - Step 0.10: Literal search end-to-end -- prx search walks files, regex matches, returns structured JSON with file/line/column/match/snippet/hash, budget enforcement, 16 tests
- I incorporated Karpathy's coding guidelines into AGENTS.md and CLAUDE.md

CURRENT STATE
-------------
- 88 tests passing, 0 failures
- cargo fmt clean, cargo clippy -D warnings clean
- 76.79% coverage (526/685 lines)
- Phase 0 milestone achieved: prx search "fn main" src/ returns valid JSON envelope
- Binary works: prx --version, prx --help, prx search all functional
- Model placeholder files in place (empty) -- real potion-code-16M.safetensors needed for semantic search in Phase 1
- cl100k_base.json placeholder in place -- real tokenizer needed for exact token counting
- No git repo initialized for the ag project yet
- Multicall dropped for now (clap 4.6 multicall conflicts with global args)
- Rust 1.95.0 installed via rustup on macOS M2

PENDING TASKS
-------------
- Phase 1 Step 1.1: Hybrid search -- integrate dense + sparse indexes with RRF fusion (src/search/fusion.rs)
- Phase 1 Step 1.2: Reranking pipeline (src/ranking/) -- boosting.rs, penalties.rs, weighting.rs
- Phase 1 Step 1.3: Budget enforcement with continuation tokens
- Phase 1 Step 1.4: Structural search via ast-grep-core
- Phase 1 Step 1.5: Search auto-detection (literal vs semantic vs structural)
- Phase 1 Step 1.6: prx read -- full implementation with --lines, --snap, --skeleton, --outline, --hash, --budget
- Phase 1 Step 1.7: prx find -- full implementation with tree+flat output
- Phase 1 Step 1.8: prx exists -- bloom filter
- Phase 1 Step 1.9: prx outline -- standalone symbol extraction
- Download real potion-code-16M.safetensors model (~32MB float16) for semantic search
- Download real cl100k_base.json tokenizer for exact token counting
- Phase 2: prx edit, prx diff, prx mcp, prx index, prx batch, prx stats, prx init
- Phase 3: benchmarks, cross-platform CI, binary optimization, distribution

KEY FILES
---------
- src/main.rs - CLI entry point, clap parse + command dispatch
- src/commands/search.rs - literal search implementation (Phase 0 milestone), 16 tests
- src/commands/mod.rs - Commands enum, Cli struct, all subcommand arg definitions
- src/output.rs - JSON envelope, AgError enum, structured error output
- src/parsing/outline.rs - symbol extraction via tree-sitter AST traversal
- src/parsing/snap.rs - structural snapping (expand line range to enclosing function/class)
- src/chunking/treesitter.rs - AST-aware chunking algorithm (ported from Semble)
- src/index/dense.rs - Model2Vec embedding index (tokenize, lookup, mean pool, L2 normalize, cosine search)
- src/index/sparse.rs - BM25 index with CSC sparse matrix and content enrichment
- src/search/tokenize.rs - compound identifier tokenizer (camelCase/snake_case splitting)

IMPORTANT DECISIONS
-------------------
- tree-sitter 0.26 (not 0.25) -- confirmed all grammar crates compatible via tree-sitter-language bridge
- ast-grep-core 0.42 for structural search (requires tree-sitter 0.26, which is fine)
- Model weights embedded in binary via include_bytes! -- no internet, no downloads, works in sandboxes
- Pure Rust Model2Vec inference -- no ONNX Runtime (ONNX dropped x86_64 macOS support)
- Custom BM25 with sprs crate -- not tantivy (lighter, sufficient for our use case)
- Token counting: full response (envelope + data), not just data payload
- Tokenizer: cl100k_base for budget enforcement
- prx read includes outline by default alongside content
- Diff summary: heuristic from structural analysis, not template-based
- Three-tier integration: CLI on PATH (primary), MCP server (top-level agents), agent definitions (Claude Code sub-agents)
- Multicall deferred -- clap 4.6 multicall conflicts with global args (--plain, --quiet)
- notify downgraded to 8.x (9.x is still RC-only on crates.io)
- Karpathy's four principles (think, simplicity, surgical, goal-driven) embedded in AGENTS.md

EXPLICIT CONSTRAINTS
--------------------
- "No. Embed in binary. We cant always assume that the env where the tools are being used will have internet access - for sandboxing and security."
- AGENTS.md: "Every crate added to Cargo.toml must have a comment explaining why it is needed"
- AGENTS.md: "unwrap() and expect() are forbidden outside #[cfg(test)] modules"
- AGENTS.md: "All output must go through the JSON envelope in src/output.rs. Never println!() directly to stdout"
- AGENTS.md: "Errors are structured JSON on stdout with a non-zero exit code. stderr is reserved for debug logging only"
- AGENTS.md: "No #[cfg(target_os)] in command logic"
- OUTPUT.md: File references use repo-relative paths, not absolute paths

CONTEXT FOR CONTINUATION
------------------------
- The IMPLEMENTATION.md file at docs/design/IMPLEMENTATION.md has the full step-by-step plan through Phase 3
- The SYSTEM-DESIGN.md file has detailed design for all 20 subsystems including exact algorithms, data structures, and data flow
- The CRATE-REFERENCE.md has API patterns for every crate with code snippets
- The ranking/ directory needs to be created (exists as empty dir from initial scaffold but has no mod.rs)
- The search/ directory currently only has tokenize.rs -- needs fusion.rs, semantic.rs, literal.rs, structural.rs
- The real model file can be downloaded with: curl -L https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors -o models/potion-code-16M.safetensors
- The real tokenizer can be downloaded with: curl -L https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json -o models/cl100k_base.json
- When adding the ranking module, follow the 5-stage pipeline documented in SEARCH.md exactly: file coherence, definition boost, stem matching, noise penalties, saturation decay
- The walk.rs tests require git init in temp dirs for .gitignore to work (ignore crate requirement)
- The comment hook fires on every file write -- all clap /// doc comments and algorithm/formula comments are necessary and should be acknowledged as such

TO CONTINUE IN A NEW SESSION
-----------------------------
1. Press 'n' in OpenCode TUI to open a new session, or run 'opencode' in a new terminal
2. Paste the contents of this file as your first message
3. Add your request: "Continue from the handoff context above. Start Phase 1 -- hybrid search, reranking, prx read, prx find."
