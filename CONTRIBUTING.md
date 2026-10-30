# Contributing to wend

Thanks for taking an interest. wend is heavily opinionated (see the [README](README.md)), so for anything bigger than a bug fix, please open an issue first to check it fits before you put time into it.

## Setup

You need [rustup](https://rustup.rs) and [just](https://github.com/casey/just).

```sh
just toolchain   # installs the pinned Rust, nightly rustfmt, the cargo tools and the git hooks
just run         # runs wend from source
just check       # what CI runs: fmt, clippy, docs, tests
```

Run `just` on its own to list every recipe.

## Layout

The workspace lives in `crates/`, one flat folder per crate, each named after its crate:

- `wend-core`: the editor itself, with buffers, editing and state, and no terminal attached.
- `wend-cli`: the terminal frontend. Its package is named plain `wend`, so `cargo install wend` installs the binary.

Every other crate is named after its folder, with the `wend-` prefix. Every crate inherits its version, edition, lints and shared dependencies from the root `Cargo.toml`.

## Style

[STYLE.md](STYLE.md) is how wend's Rust is written: layout, naming, errors, API shape, tests and commits, each rule with its reason. Most rules are enforced by the checks below. The rest are flagged in review, citing the rule's `S-…` id.

To check a change against the rules tools can't enforce, ask your coding agent to run the `style-review` skill (`.claude/skills/style-review/SKILL.md`). Any agent that reads [AGENTS.md](AGENTS.md) finds it.

## Before you push

`just check` has to pass. It fails on any formatting difference, any clippy or rustdoc warning, any failing test, and any dependency problem (below). On a pull request, CI also checks every commit message (see [Commits](#commits)).

## Benchmarks

`just bench` runs the benchmarks with [criterion](https://github.com/criterion-rs/criterion.rs). They live in `crates/wend-core/benches/session.rs` and time a headless editing session: `Editor` driven the way a terminal client drives it, a key and then a frame. Each phase (open, a redraw on its own, every motion in both directions, typing, deleting, reverting) is timed per action on files of 100 KB, 1 MB and 10 MB, so you can see what one key press costs and how it scales with file size.

criterion compares each run with the previous one on the same machine. To compare against a fixed point, save a baseline and run against it later:

```sh
just bench --save-baseline before
just bench --baseline before
```

Benchmarks aren't part of `just check`, but clippy lints them like any other code.

## Dependencies

Declare every dependency once, in `[workspace.dependencies]` in the root `Cargo.toml`, and pull it into a crate with `name.workspace = true`. That way two crates can never sit on different versions of the same dependency. `just deps` enforces it:

- `cargo machete` fails on dependencies a crate declares but never uses.
- `cargo deny` (configured in `deny.toml`) fails on crates with known vulnerabilities, licenses that don't fit MIT OR Apache-2.0, dependencies used by more than one crate but not shared through the workspace, and sources other than crates.io. It also reports duplicate versions in the tree, without failing.

`just fix` applies clippy's automatic fixes and formats, which clears most lints on its own. If you're stuck on a lint, push anyway and say so in the PR: a maintainer can fix it up, and a lint should never be the reason a good change doesn't land.

## Lints

The lint policy lives in `[workspace.lints]` in the root `Cargo.toml`. In short:

- clippy's `all`, `pedantic` and `cargo` groups are on, plus a set of `restriction` lints chosen for an editor and to enforce [STYLE.md](STYLE.md). Each is listed with its reason, or the `S-…` rule it enforces. Thresholds, such as nesting depth and the banned names, live in `clippy.toml`.
- **No printing.** The terminal is the UI, and a stray `println!` or `eprintln!` corrupts the screen. Log or return errors instead.
- **No `unwrap()`, `panic!` or `std::process::exit` outside tests.** A panic loses the user's unsaved work, and `exit` skips the code that restores the terminal. Use `?`, or `expect("...")` stating the invariant when one really holds.
- **Docs where they add insight, not everywhere.** Document what a type is for, what a function promises, and when it panics (`# Panics` is linted). Don't restate what names and types already say (STYLE.md S-DOCS).
- **Fields are all public or all private.** Stateful structs keep every field private (STYLE.md S-FIELDS).
- **Code stays flat.** Nothing nests deeper than 4 blocks (STYLE.md S-FLAT).
- **Unsafe is opt-in per site.** Mark it with `#[allow(unsafe_code, reason = "...")]` and put a `// SAFETY:` comment on every `unsafe` block.
- **Escape hatches are local.** Silence a lint with `#[allow(lint)]` on the smallest item that needs it, never crate-wide.

The toolchain is pinned in `rust-toolchain.toml`, so everyone sees the same lints. A toolchain bump goes in its own change, which fixes or allows (with a reason) any new lints it brings.

## Formatting

`just fmt` formats everything, and `just check` fails on anything it would change:

- Rust: rustfmt with the options in `rustfmt.toml`. The import options (STYLE.md S-IMPORTS) are nightly-only, so `just fmt` runs the nightly pinned as `fmt_toolchain` in the `justfile`, and `just toolchain` installs it. A plain `cargo fmt` on stable warns and skips them. Point your editor at the pinned nightly; `.helix/languages.toml` already does for Helix.
- `Cargo.toml` dependency tables: sorted alphabetically by `cargo sort`.
- Every TOML file: formatted by `taplo` (`taplo.toml`). It never reorders keys, so hand-grouped tables such as the lints keep their order.

## Commits

Commit messages follow STYLE.md S-COMMITS: `area: lowercase imperative summary`, at most 72 characters with no trailing period, then a blank line and a body saying why.

```
view: scroll by row instead of by line

Scrolling moved whole lines, so a line that wrapped past the bottom of
the screen could never be scrolled through. Scroll one row at a time.
```

`just hooks`, run by `just toolchain`, installs a commit-msg hook that rejects anything else, and CI runs the same check (`just commits`) on every commit in a pull request.

## Releases

Every crate shares one version, set in `[workspace.package]` in the root `Cargo.toml`. Change it with `just bump <version>` (`cargo set-version`), never by hand: that also updates the exact `=` requirements between internal crates. Publish `wend-core` before `wend`.

The shipped binary is built with `just dist` (the `dist` profile: fat LTO, one codegen unit, stripped). `cargo build --release` stays quick for day-to-day profiling.

## License

Unless you say otherwise, any contribution you submit for inclusion in wend is dual-licensed as MIT OR Apache-2.0, as described in the README, with no additional terms or conditions.
