set shell := ["bash", "-uc"]

# List available recipes
default:
    @just --list

# Run wend from source; extra args go to wend: just run [args]
run *args:
    cargo run -p wend -- {{ args }}

# Build the workspace
build:
    cargo build --workspace

# Build the binary we ship (fat LTO, stripped) into target/dist/wend
dist:
    cargo build -p wend --profile dist --locked

# Format every Rust file
fmt:
    cargo fmt --all

# Apply clippy's automatic fixes, then format
fix:
    cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets
    cargo fmt --all

# The CI gate: run it before you push
check: fmt-check clippy doc test

# Fail on any file `just fmt` would change
fmt-check:
    cargo fmt --all -- --check

# Fail on any lint
clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Build the docs; any rustdoc warning fails
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked

# Run every test in the workspace
test:
    cargo test --workspace --locked

# Build the docs, private items included, and open a crate's page: just docs [crate]
docs crate="wend_core":
    cargo doc --workspace --no-deps --document-private-items
    open target/doc/{{ crate }}/index.html

# Install the pinned toolchain from rust-toolchain.toml
toolchain:
    rustup toolchain install
