# Competitive Landscape

This page describes the problem prx addresses, the existing tools in this space, and how prx relates to them.

## The Problem

AI coding agents waste between 30% and 93% of their token budget on exploration work that produces no code changes. The root cause is a mismatch: Unix tools were designed for human eyes, and agents must re-parse their output to extract structured meaning.

The canonical failure mode is the grep-read-grep loop:

1. Agent runs `grep` to find a symbol. Gets file paths and line numbers.
2. Agent runs `cat` on each file to read context. Gets entire files.
3. Agent runs `grep` again to narrow down. Gets the same noise.

A single grep-read-grep loop consumes roughly 11,300 tokens, of which about 800 are useful. That's 93% waste per loop.

The pattern compounds. The SWE-bench token study (arxiv 2604.22750) found that 50% of file reads are re-reads of files the agent already loaded earlier in the session. Context cost grows O(n²) over a session, not O(n), because every new token must attend to every prior token.

From the SWE-chat dataset (355K tool calls), the most-used tools are:

| Tool | Share of calls |
|---|---|
| Read | 19.8% |
| Grep | 10.1% |
| Bash:file | 6.9% |

These three tools account for roughly a third of all agent tool calls. They're also the tools with the worst token efficiency.

## Existing Tools

| Project | Approach | Token Savings | Quality (NDCG@10) | Language | Limitation |
|---|---|---|---|---|---|
| Semble | Hybrid search: embeddings + BM25 + reranking | 98% | 0.854 | Python | Search only. No read, edit, or diff. Python dependency. |
| RTK | Proxy wrapper over existing tools with 60-90% compression | 60-90% | — | — | Wrapper, not replacement. Still spawns shells. No structural awareness. |
| Hypergrep | Indexed daemon with call graphs | 87% | — | Rust | Heavy daemon. Call graphs are Rust-only. Research stage. |
| aict | 22 Go reimplementations of coreutils with JSON/XML output | ~60% | — | Go | MIME detection overhead. Slower than the tools it replaces. |
| instant-grep | Trigram-indexed search | 93.5% | — | — | Search only. |
| LeanCTX | Context compression OS | 99% file read compression | — | — | Compression layer, not native tools. |
| squeez | PreToolUse hook compression | 95% bash reduction | — | — | Post-hoc compression. Doesn't change the underlying tool calls. |
| FileSift | Semantic file search: BM25 + FAISS | — | — | Python | Search only. Python. Requires indexing step. |
| SWE-agent ACI | Custom commands: search_file, open, edit | — | — | Python | Tightly coupled to SWE-agent. Not standalone. |

Semble's retrieval quality (NDCG 0.854) is the strongest published number in this space. aict's philosophy of reimplementing coreutils for structured output is the right instinct, but the Go implementation trades speed for structure in a way that hurts in practice. The compression-layer tools (LeanCTX, squeez, RTK) reduce token counts without changing the underlying access pattern, which limits how far they can go.

## LSP vs Grep

A measurement comparing LSP and grep for identical operations found:

- LSP saves 5-34x tokens vs grep for the same code navigation tasks
- LSP rename: 1,441x fewer tokens than the equivalent grep + read + replace sequence

The gap is real. LSP operates on the semantic structure of code rather than its text representation, so it can answer "find all references to this function" in a single round-trip instead of a grep loop.

The catch is setup cost. LSP requires a running language server, per-language configuration, and startup latency. For agents that need to work across polyglot repos or ephemeral environments, that's a meaningful barrier.

prx occupies the middle ground: structural awareness without a running LSP server. It understands file structure, symbol relationships, and content semantics natively, without requiring language-specific infrastructure.

## Where prx Fits

prx is not a wrapper. RTK, squeez, and LeanCTX all sit in front of existing tools and compress their output. prx replaces the tools.

prx is not search-only. Semble, instant-grep, FileSift, and Hypergrep all solve the retrieval problem well. None of them read, edit, or diff files. An agent still needs other tools to act on what it finds.

prx is not Python. Python dependencies add friction in CI, containers, and minimal environments.

prx is a single Rust binary that replaces five core tools (read, grep, find, edit, diff) with native structured output, embedded semantic search, and zero runtime dependencies.

The closest analog is aict: same philosophy of reimplementing coreutils for agent consumption. prx differs in three ways. It's written in Rust, so it's faster than the tools it replaces rather than slower. It adds semantic search natively rather than treating retrieval as a separate concern. And it covers the full read-search-edit-diff loop rather than stopping at structured output.

prx uses a similar hybrid retrieval architecture to Semble (embeddings + BM25 + reranking) but is a separate implementation. Semble's published NDCG of 0.854 is a reference point, not a claim about prx's quality — prx has not yet run formal NDCG benchmarks against the same datasets.

## References

- SWE-bench token study: https://arxiv.org/pdf/2604.22750
- Semble: https://github.com/MinishLab/semble
- RTK: https://github.com/rtk-ai/rtk
- Hypergrep: https://marjoballabani.github.io/hypergrep/
- LSP vs grep measurement: https://dev.to/daynablackwell/we-measured-it-lsp-saves-ai-agents-5-34x-tokens-vs-grep-427
