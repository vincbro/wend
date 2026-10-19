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

# Format every Rust and TOML file, and sort Cargo.toml dependency tables
fmt:
    cargo fmt --all
    cargo sort --workspace --no-format
    taplo fmt

# Apply clippy's automatic fixes, then format
fix:
    cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets
    cargo fmt --all

# The CI gate: run it before you push
check: fmt-check clippy doc test deps

# Fail on any file `just fmt` would change
fmt-check:
    cargo fmt --all -- --check
    cargo sort --workspace --no-format --check
    taplo fmt --check

# Fail on any lint
clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Build the docs; any rustdoc warning fails
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked

# Run every test in the workspace
test:
    cargo test --workspace --locked

# Fail on unused dependencies, or on any cargo-deny check in deny.toml:
# advisories, licenses, dependencies not shared through the workspace, sources
deps:
    cargo machete
    cargo deny --locked check

# Build the docs, private items included, and open a crate's page: just docs [crate]
docs crate="wend_core":
    cargo doc --workspace --no-deps --document-private-items
    open target/doc/{{ crate }}/index.html

# Set every crate's version, internal requirements included: just bump 0.1.0
bump version:
    cargo set-version --workspace {{ quote(version) }}

# Install the pinned toolchain from rust-toolchain.toml, and off CI the cargo
# tools other recipes call (CI installs those prebuilt)
toolchain:
    rustup toolchain install
    @[ -n "${CI:-}" ] || just _tools

# Each tool only if it's missing: command:crate
_tools:
    for tool in cargo-set-version:cargo-edit cargo-machete:cargo-machete cargo-deny:cargo-deny cargo-sort:cargo-sort taplo:taplo-cli; do command -v "${tool%%:*}" >/dev/null || cargo install --locked "${tool#*:}"; done
