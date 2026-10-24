set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes
default:
    @just --list

# Check formatting
fmt:
    cargo fmt --all -- --check

# Clippy with warnings as errors
clippy:
    cargo clippy --all-targets --all-features --locked -- -D warnings

# Run the test suite
test:
    cargo test --locked --all-features --all-targets

# Build docs; broken docs are errors
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked

# Licenses, advisories, bans and sources (requires cargo-deny)
deny:
    cargo deny check

# Type-check against Rust 1.85 (requires: rustup toolchain install 1.85)
msrv:
    cargo +1.85 check --locked --all-features --all-targets

# Full local gate; run before pushing
check: fmt clippy test doc deny msrv

# Gate plus a release build
release-check: check
    cargo build --release --locked
