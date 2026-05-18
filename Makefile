.PHONY: check build test bench models clean coverage

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
	@bash scripts/download-models.sh

coverage:
	cargo tarpaulin --out html --output-dir target/coverage
	@echo "Report: target/coverage/tarpaulin-report.html"

clean:
	cargo clean
	rm -rf target/criterion
