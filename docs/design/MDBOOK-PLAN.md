# v0.5.8 — mdBook Documentation Site Plan

## Audit Summary

28 source files audited across `docs/`, root markdown, and `skills/`.

| State | Count | Files |
|---|---|---|
| Public-ready | 10 | USAGE, CONTRIBUTING, CLI, OUTPUT, LANDSCAPE, PLATFORM, PRD, ROADMAP, PRX-RUN, skills/agents |
| Needs work | 5 | SYSTEM, SEARCH, RUN-PARSERS, FALLBACK, BENCHMARKS |
| Internal-only | 8 | GIT-WORKFLOW, LEAN-DOWN, PATCH-PLAN, V050-PLAN, SEARCH-QUALITY, MODEL-TIERING, HANDOFF, IMPLEMENTATION |
| Redundant/stale | 5 | CLAUDE (→ AGENTS), PRX-RUN-DESIGN (→ merge), TESTING (incomplete), TELEMETRY-FINDINGS (→ merge into Performance), CRATE-REFERENCE (→ Contributing) |

## Target: SUMMARY.md

```
# Summary

[Introduction](README.md)

# User Guide

- [Quick Start](guide/quickstart.md)
- [Installation](guide/installation.md)
- [Agent Integration](guide/integration.md)
- [Token Savings](guide/token-savings.md)

# Commands

- [search](commands/search.md)
- [read](commands/read.md)
- [find](commands/find.md)
- [edit](commands/edit.md)
- [diff](commands/diff.md)
- [run](commands/run.md)
- [context](commands/context.md)
- [impact](commands/impact.md)
- [index](commands/index.md)
- [outline](commands/outline.md)
- [exists](commands/exists.md)
- [Other Commands](commands/other.md)

# Architecture

- [System Overview](architecture/overview.md)
- [Search Pipeline](architecture/search.md)
- [Ranking & Reranking](architecture/ranking.md)
- [Import Graph](architecture/import-graph.md)
- [Run Parsers](architecture/run-parsers.md)
- [Fallback System](architecture/fallback.md)

# Performance

- [Indexing Performance](performance/indexing.md)
- [Search Quality (NDCG)](performance/search-quality.md)
- [Public Benchmark Suite](performance/benchmarks.md)

# Reference

- [CLI Specification](reference/cli.md)
- [JSON Output Format](reference/output.md)
- [Platform Support](reference/platforms.md)
- [Competitive Landscape](reference/landscape.md)

# Contributing

- [Developer Setup](contributing/setup.md)
- [Coding Guidelines](contributing/guidelines.md)
- [Dependencies](contributing/dependencies.md)

# Vision

- [Product Requirements](vision/prd.md)
- [Roadmap](vision/roadmap.md)
```

## Source Mapping

Where each mdBook page comes from:

| mdBook Page | Source File | Action |
|---|---|---|
| README.md | README.md | Adapt intro (remove badges, simplify for docs site) |
| guide/quickstart.md | NEW | Extract from USAGE.md + README quick start |
| guide/installation.md | README.md Install section | Extract and expand |
| guide/integration.md | README.md Agent Integration + skills/agents.md | Merge and structure |
| guide/token-savings.md | README.md Token Savings + TELEMETRY-FINDINGS.md | Merge real-world data |
| commands/search.md | USAGE.md search section | Extract, add examples |
| commands/read.md | USAGE.md read section | Extract, add examples |
| commands/find.md | USAGE.md find section | Extract, add examples |
| commands/edit.md | USAGE.md edit section | Extract, add examples |
| commands/diff.md | USAGE.md diff section | Extract, add examples |
| commands/run.md | PRX-RUN.md + USAGE.md run section | Merge |
| commands/context.md | USAGE.md context section | Extract, add examples |
| commands/impact.md | USAGE.md impact section | Extract, add examples |
| commands/index.md | NEW | Write from scratch (index, search quality, model) |
| commands/outline.md | USAGE.md outline section | Extract |
| commands/exists.md | USAGE.md exists section | Extract |
| commands/other.md | USAGE.md (batch, stats, bench, init, mcp) | Combine minor commands |
| architecture/overview.md | SYSTEM.md + SYSTEM-DESIGN.md | Merge, add diagrams |
| architecture/search.md | SEARCH.md | Rewrite for public audience, add diagrams |
| architecture/ranking.md | SEARCH.md ranking section | Extract, add examples |
| architecture/import-graph.md | NEW | Write from SEARCH.md import section |
| architecture/run-parsers.md | RUN-PARSERS.md | Complete the parser catalog |
| architecture/fallback.md | FALLBACK.md | Reframe for public |
| performance/indexing.md | README.md Performance section | Expand with methodology |
| performance/search-quality.md | SEARCH-QUALITY.md (sanitized) | Remove internal version details |
| performance/benchmarks.md | benchmarks/results/v0.5.7-baseline.json | Format as narrative |
| reference/cli.md | CLI.md | Direct copy |
| reference/output.md | OUTPUT.md | Direct copy |
| reference/platforms.md | PLATFORM.md | Direct copy |
| reference/landscape.md | LANDSCAPE.md | Direct copy |
| contributing/setup.md | CONTRIBUTING.md | Direct copy |
| contributing/guidelines.md | AGENTS.md coding discipline section | Extract |
| contributing/dependencies.md | CRATE-REFERENCE.md | Direct copy |
| vision/prd.md | PRD.md | Direct copy |
| vision/roadmap.md | ROADMAP.md | Direct copy |

## Files to Archive (move to docs/internal/)

These are internal development docs — valuable for the team but not for
public consumption:

- `docs/design/GIT-WORKFLOW.md` — internal release process
- `docs/design/LEAN-DOWN.md` — v0.5.4 sprint plan (completed)
- `docs/design/PATCH-PLAN.md` — v0.4.x patch plan (completed)
- `docs/design/V050-PLAN.md` — v0.5.0 plan (completed)
- `docs/design/IMPLEMENTATION.md` — generic implementation plan
- `docs/design/MODEL-TIERING.md` — v0.6.0 design (not yet released)
- `docs/design/SEARCH-QUALITY.md` — internal NDCG tracking
- `HANDOFF.md` — session handoff context
- `CLAUDE.md` — redundant (just points to AGENTS.md)

## Files to Delete

- `docs/design/PRX-RUN-DESIGN.md` — content merged into architecture/overview + commands/run

## Docs Needing Rewrites (before mdBook)

### architecture/overview.md (from SYSTEM.md)
- Fix version inconsistencies ("15 languages" vs actual count)
- Add Mermaid diagram showing module relationships
- Add data flow diagram (query → search → rank → output)
- Simplify for non-IR audience

### architecture/search.md (from SEARCH.md)
- Add Mermaid diagram of the search pipeline
- Add Mermaid diagram of RRF fusion
- List all 10 language families for import extraction
- Add concrete examples (before/after query → results)
- Explain BM25, semantic, structural in accessible terms

### architecture/run-parsers.md (from RUN-PARSERS.md)
- Complete the parser catalog table (all 22 parsers)
- Add before/after examples for each parser category
- Show token savings per parser

### performance/benchmarks.md (NEW)
- Format v0.5.7 baseline data as narrative
- Include indexing time benchmarks
- Add interpretation guidance (what the numbers mean)

## mdBook Configuration

### book.toml

```toml
[book]
title = "prx Documentation"
authors = ["Civitas"]
description = "Agent-native Unix tools for AI coding agents"
language = "en"
src = "book/src"

[build]
build-dir = "book/build"

[output.html]
git-repository-url = "https://github.com/civitas-io/prx"
edit-url-template = "https://github.com/civitas-io/prx/edit/main/book/src/{path}"
default-theme = "light"
preferred-dark-theme = "navy"

[output.html.search]
enable = true

[preprocessor.admonish]
command = "mdbook-admonish"

[preprocessor.mermaid]
command = "mdbook-mermaid"

[output.html.playground]
editable = false
```

### Plugins

- **mdbook-admonish** — admonition blocks (warnings, tips, notes)
- **mdbook-mermaid** — Mermaid diagrams (architecture, data flow)

### Directory Structure

```
book/
├── book.toml
└── src/
    ├── SUMMARY.md
    ├── README.md
    ├── guide/
    ├── commands/
    ├── architecture/
    ├── performance/
    ├── reference/
    ├── contributing/
    └── vision/
```

## GitHub Actions Deployment

### .github/workflows/deploy-docs.yml

```yaml
name: Deploy Docs

on:
  push:
    branches: [main]
    paths: ['book/**', 'docs/**']
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v6
      - name: Install mdBook
        run: |
          cargo install mdbook mdbook-admonish mdbook-mermaid
      - name: Build
        run: mdbook build book
      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: book/build
      - name: Deploy
        id: deployment
        uses: actions/deploy-pages@v4
```

## Makefile & Scripts Cleanup

### Makefile

Update to reflect current build system. Remove stale `models` target
dependency from `setup`. Keep as developer convenience only.

```makefile
.PHONY: check build release test docs clean help

help:
	@echo "  make check    - fmt + clippy + tests"
	@echo "  make build    - debug build"
	@echo "  make release  - release build (~49 MB)"
	@echo "  make test     - all tests"
	@echo "  make docs     - build mdBook docs"
	@echo "  make clean    - remove artifacts"

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

docs:
	mdbook build book

clean:
	cargo clean
	rm -rf book/build
```

Remove `setup`, `models`, `test-unit`, `test-e2e`, `coverage`, `bench`
targets — they're just cargo commands and don't need wrappers.

### scripts/

- `download-models.sh` — KEEP. Useful for CI cache warming and offline
  builds. The header already explains build.rs handles this automatically.
- `install-hooks.sh` — KEEP. Move hook installation into CONTRIBUTING.md
  as a manual step. Or replace with a `cargo install` of a hook manager.

## Implementation Order

1. Create `book/` directory with `book.toml` and `src/SUMMARY.md`
2. Copy public-ready files into `book/src/` (10 files, direct copy)
3. Write new pages (quickstart, installation, integration, index command)
4. Rewrite architecture pages (overview, search — add diagrams)
5. Complete run-parsers catalog
6. Create performance/benchmarks narrative page
7. Move internal docs to `docs/internal/`
8. Update Makefile
9. Add `deploy-docs.yml` workflow
10. Enable GitHub Pages on civitas-io/prx
11. Delete redundant files (CLAUDE.md, PRX-RUN-DESIGN.md)
