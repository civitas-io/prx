.PHONY: check build release test docs clean help

help:
	@echo "prx development targets:"
	@echo ""
	@echo "  make check    - Run fmt, clippy, and all tests"
	@echo "  make build    - Debug build"
	@echo "  make release  - Release build (optimized, ~49 MB)"
	@echo "  make test     - Run all tests (unit + E2E + MCP)"
	@echo "  make docs     - Build mdBook documentation"
	@echo "  make clean    - Remove build artifacts"

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test

build:
	cargo build

release:
	cargo build --release
	@ls -lh target/release/prx

test:
	cargo test

docs:
	mdbook build book

clean:
	cargo clean
	rm -rf book/build
