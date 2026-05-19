#!/bin/bash
# Install git pre-commit hook that mirrors CI checks
set -e

HOOK=.git/hooks/pre-commit

cat > "$HOOK" << 'EOF'
#!/bin/bash
set -e

echo "pre-commit: checking format..."
cargo fmt --check 2>&1 || {
    echo "FAILED: cargo fmt --check"
    echo "Run 'cargo fmt' to fix."
    exit 1
}

echo "pre-commit: running clippy..."
cargo clippy --no-default-features -- -D warnings 2>&1 || {
    echo "FAILED: cargo clippy"
    exit 1
}

echo "pre-commit: running tests..."
cargo test --no-default-features --lib 2>&1 || {
    echo "FAILED: cargo test"
    exit 1
}

echo "pre-commit: all checks passed"
EOF

chmod +x "$HOOK"
echo "Pre-commit hook installed at $HOOK"
