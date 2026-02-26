.PHONY: build test lint fmt fmt-check clippy doc clean

# Build the crate with default features
build:
	cargo build

# Build with all features
build-all:
	cargo build --all-features

# Run tests
test:
	cargo +nightly nextest run --locked --all-features --no-fail-fast

# Run clippy lints
clippy:
	cargo +nightly clippy --all-targets --all-features -- -D warnings

# Format source code
fmt:
	cargo +nightly fmt

# Run all lints (fmt + clippy)
lint: fmt clippy

# Build documentation
docs:
	cargo +nightly doc --document-private-items

# Remove build artifacts
clean:
	cargo clean

# Run everything CI would run
pr: lint test docs
