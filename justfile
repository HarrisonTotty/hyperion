_default:
    @just --list

# Build the project in debug mode
build:
    cargo build

# Build the project with release optimizations
build-release:
    cargo build --release

# Run the hyperion binary (pass args with `just run -- --help`)
run *args:
    cargo run -- {{args}}

# Run the release binary
run-release *args:
    cargo run --release -- {{args}}

# Run all tests (unit, integration, and doc tests)
test:
    cargo test

# Run a specific test by name
test-one name:
    cargo test {{name}}

# Run only doc tests
test-doc:
    cargo test --doc

# Format all source files
format:
    cargo fmt --all

# Check formatting without modifying files
format-check:
    cargo fmt --all -- --check

# Run clippy with warnings treated as errors
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Fast type/borrow check without codegen
typecheck:
    cargo check --all-targets --all-features

# Build and open API documentation
doc:
    cargo doc --no-deps --open

# Remove build artifacts
clean:
    cargo clean

# Run the full CI gate: format check, lint, typecheck, and tests
check: format-check lint typecheck test

# Update dependencies in Cargo.lock
update:
    cargo update

# Audit dependencies for known vulnerabilities (requires cargo-audit)
audit:
    cargo audit
