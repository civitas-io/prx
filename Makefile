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
	@mkdir -p models
	@test -f models/potion-code-16M.safetensors || \
		curl -L https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors \
		-o models/potion-code-16M.safetensors
	@test -f models/cl100k_base.json || \
		curl -L https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json \
		-o models/cl100k_base.json

coverage:
	cargo tarpaulin --out html --output-dir target/coverage
	@echo "Report: target/coverage/tarpaulin-report.html"

clean:
	cargo clean
	rm -rf target/criterion
