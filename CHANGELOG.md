# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Project scaffold with documentation-first approach
- Product requirements (docs/vision/PRD.md)
- Roadmap with phased delivery plan (docs/vision/ROADMAP.md)
- System architecture documentation (docs/architecture/SYSTEM.md)
- Search subsystem architecture (docs/architecture/SEARCH.md)
- CLI interface specification (docs/design/CLI.md)
- JSON output format specification (docs/design/OUTPUT.md)
- Competitive landscape analysis (docs/research/LANDSCAPE.md)
- Cross-platform compatibility audit (docs/research/PLATFORM.md)
- Implementation plan with step-by-step build order (docs/design/IMPLEMENTATION.md)
- Testing plan covering unit, integration, and benchmarks (docs/design/TESTING.md)
- Crate reference with exact versions and API patterns (docs/design/CRATE-REFERENCE.md)
- Developer setup and contributing guide (CONTRIBUTING.md)
- Corrected tree-sitter to 0.25.x (0.26.x incompatible with grammar crates)
- Updated crate versions: ndarray 0.17, similar 3.1, bloomfilter 3.0, tokenizers 0.23, criterion 0.8
- Added regex crate dependency for literal search
- prx run: structured command runner with tool-specific parsers (docs/design/PRX-RUN.md, PRX-RUN-DESIGN.md)
- Detailed system design for all 20 subsystems (docs/design/SYSTEM-DESIGN.md)
- Three-tier integration strategy: CLI + MCP + agent definitions
- `prx init` command for agent framework setup
- Benchmarking plan and methodology (docs/design/BENCHMARKS.md)
- AGENTS.md for AI coding assistant guidance
