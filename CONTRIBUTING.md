# Contributing to wend

Thanks for taking an interest. wend is heavily opinionated (see the [README](README.md)), so for anything bigger than a bug fix, please open an issue first to check it fits before you put time into it.

## Setup

You need [rustup](https://rustup.rs) and [just](https://github.com/casey/just).

```sh
just toolchain   # installs the pinned Rust and the cargo tools the recipes use
just run         # runs wend from source
just check       # what CI runs: fmt, clippy, docs, tests
```

Run `just` on its own to list every recipe.

## Layout

The workspace lives in `crates/`, one flat folder per crate, each named after its crate:

- `wend-core`: the editor itself, with buffers, editing and state, and no terminal attached.
- `wend-cli`: the terminal frontend. Its package is named plain `wend`, so `cargo install wend` installs the binary.

Every other crate is named after its folder, with the `wend-` prefix. Every crate inherits its version, edition, lints and shared dependencies from the root `Cargo.toml`.

## Before you push

`just check` has to pass. It fails on any formatting difference, any clippy or rustdoc warning, any failing test, and any dependency problem (below).

## Dependencies

Declare every dependency once, in `[workspace.dependencies]` in the root `Cargo.toml`, and pull it into a crate with `name.workspace = true`. That way two crates can never sit on different versions of the same dependency. `just deps` enforces it:

- `cargo machete` fails on dependencies a crate declares but never uses.
- `cargo deny` (configured in `deny.toml`) fails on crates with known vulnerabilities, licenses that don't fit MIT OR Apache-2.0, dependencies used by more than one crate but not shared through the workspace, and sources other than crates.io. It also reports duplicate versions in the tree, without failing.

`just fix` applies clippy's automatic fixes and formats, which clears most lints on its own. If you're stuck on a lint, push anyway and say so in the PR: a maintainer can fix it up, and a lint should never be the reason a good change doesn't land.

## Lints

The lint policy lives in `[workspace.lints]` in the root `Cargo.toml`. In short:

- clippy's `all`, `pedantic` and `cargo` groups are on, plus a small set of `restriction` lints chosen for an editor. Each is listed with its reason.
- **No printing.** The terminal is the UI, and a stray `println!` or `eprintln!` corrupts the screen. Log or return errors instead.
- **No `unwrap()` and no `std::process::exit` outside tests.** A panic loses the user's unsaved work, and `exit` skips the code that restores the terminal. Use `?`, or `expect("...")` when an invariant really holds.
- **Public items need docs.**
- **Unsafe is opt-in per site.** Mark it with `#[expect(unsafe_code, reason = "...")]` and put a `// SAFETY:` comment on every `unsafe` block.
- **Escape hatches give reasons.** Use `#[expect(lint, reason = "...")]`, not `#[allow]`, so the escape fails once it's no longer needed.

The toolchain is pinned in `rust-toolchain.toml`, so everyone sees the same lints. A toolchain bump goes in its own change, which fixes or allows (with a reason) any new lints it brings.

## Formatting

`just fmt` formats everything, and `just check` fails on anything it would change:

- Rust: `cargo fmt` with the stable options in `rustfmt.toml`, so any toolchain formats the same way.
- `Cargo.toml` dependency tables: sorted alphabetically by `cargo sort`.
- Every TOML file: formatted by `taplo` (`taplo.toml`). It never reorders keys, so hand-grouped tables such as the lints keep their order.

## Releases

Every crate shares one version, set in `[workspace.package]` in the root `Cargo.toml`. Change it with `just bump <version>` (`cargo set-version`), never by hand: that also updates the exact `=` requirements between internal crates. Publish `wend-core` before `wend`.

The shipped binary is built with `just dist` (the `dist` profile: fat LTO, one codegen unit, stripped). `cargo build --release` stays quick for day-to-day profiling.

## License

Unless you say otherwise, any contribution you submit for inclusion in wend is dual-licensed as MIT OR Apache-2.0, as described in the README, with no additional terms or conditions.
