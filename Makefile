.PHONY: setup check build release test test-unit test-e2e bench models clean coverage help

help:
	@echo "prx development targets:"
	@echo ""
	@echo "  make setup      - First-time setup: download models, verify build"
	@echo "  make check      - Run fmt, clippy, and all tests"
	@echo "  make build      - Debug build"
	@echo "  make release    - Release build (optimized, ~48MB)"
	@echo "  make test       - Run all tests (unit + E2E)"
	@echo "  make test-unit  - Run unit tests only"
	@echo "  make test-e2e   - Run E2E integration tests only"
	@echo "  make bench      - Run criterion benchmarks"
	@echo "  make models     - Download and convert model files"
	@echo "  make coverage   - Generate HTML coverage report"
	@echo "  make clean      - Remove build artifacts"

setup: models
	cargo build --no-default-features
	cargo test --no-default-features --lib
	@echo ""
	@echo "Setup complete. Run 'make check' to verify everything."

check:
	cargo fmt --check
	cargo clippy --no-default-features -- -D warnings
	cargo test --no-default-features

build:
	cargo build --no-default-features

release:
	cargo build --release
	@ls -lh target/release/prx
	@echo "Binary ready at target/release/prx"

test:
	cargo test --no-default-features

test-unit:
	cargo test --no-default-features --lib

test-e2e:
	cargo test --no-default-features --test e2e

bench:
	cargo bench

models:
	@bash scripts/download-models.sh

coverage:
	cargo tarpaulin --no-default-features --out html --output-dir target/coverage --skip-clean
	@echo "Report: target/coverage/tarpaulin-report.html"

clean:
	cargo clean
	rm -rf target/criterion target/coverage
