# Contributing to wend

Thanks for taking an interest. wend has strong opinions (see the [README](README.md)), so for anything bigger than a bug fix, please open an issue first. That way we can check it fits before you put time into it.

## Setup

You need [rustup](https://rustup.rs) and [just](https://github.com/casey/just).

The coding rules are in the `code-standards/` submodule: clone with `--recurse-submodules`, or run `git submodule update --init` in a clone you already have. Building wend doesn't need it, reading and reviewing code does.

```sh
just toolchain   # installs the pinned Rust, nightly rustfmt, the cargo tools and the git hooks
just run FILE    # runs wend from source, opening FILE
just check       # what CI runs: fmt, clippy, docs, tests, dependencies
just check-full  # the same, with the property tests run for 40000 cases
just theme NAME  # a new built-in theme: a copy of themes/default.toml to edit
```

Run `just` on its own to list every recipe.

## Layout

The workspace lives in `crates/`, one flat folder per crate, each named after its crate:

- `wend-core`: the editor itself, with buffers, editing and state, and no terminal attached.
- `wend-cli`: the terminal frontend. Its package is named plain `wend`, so `cargo install wend` installs the binary.

Every other crate is named after its folder, with the `wend-` prefix. Every crate inherits its version, edition, lints and shared dependencies from the root `Cargo.toml`.

## Style

The coding rules are shared by every project through the `code-standards/` submodule. [code-standards/rust/STYLE.md](code-standards/rust/STYLE.md) is how Rust is written: layout, naming, errors, API shape and tests, each rule with its reason. Most rules are enforced by the checks below. The rest are flagged in review, citing the rule's `S-…` id.

[code-standards/rust/PERFORMANCE.md](code-standards/rust/PERFORMANCE.md) is the same for the code that runs on every key press and every frame: what may allocate, what is borrowed and what is measured, each rule with an id (`P-…`) and a reason.

[STANDARDS.local.md](STANDARDS.local.md) holds what is specific to wend: the gate, the hot paths, the performance problems that are measured and not yet fixed, and the overrides of the shared rules.

To check a change against the rules tools can't enforce, ask your coding agent to run the `review` skill (`.claude/skills/review/SKILL.md`). It runs `just check`, then checks the code against both rule files, and lists what it found without changing anything. The `commit` skill does the same before it commits. Any agent that reads [AGENTS.md](AGENTS.md) finds them.

## Before you push

`just check` needs to pass before you push. It checks formatting, clippy and rustdoc warnings, tests, and dependencies (below). On a pull request, CI also checks every commit message (see [Commits](#commits)).

## Benchmarks

`just bench` runs the benchmarks with [criterion](https://github.com/criterion-rs/criterion.rs). They live in `crates/wend-core/benches/session.rs` and time a headless editing session: `Editor` driven the way a terminal client drives it, a key and then a frame. Each phase (open, a redraw on its own, every motion in both directions, typing, deleting, reverting) is timed per action on files of 100 KB, 1 MB and 10 MB, so you can see what one key press costs and how it scales with file size.

`just bench-latency` times what the frontend adds, with no terminal attached: a key press through `Editor::apply`, the frame it sends, `draw::render_frame` and one write of the escape sequences to `/dev/null`. `input_delay` is the time from a key to its written frame (typing, a line down and a page down, on 100 KB and 1 MB files, with Rust highlighting on). `fps` redraws an unchanged frame, and the throughput criterion prints is frames per second, the ceiling for the rest. It lives in `crates/wend-cli/benches/latency.rs`.

`just bench-e2e` runs the real release binary on a pseudo terminal with `scripts/e2e_latency.py` (Python 3, no packages): it types keys at wend and times each one until the frame that answers it has been written back, so threads, channels and the real terminal write are included. It leaves out the terminal emulator painting those bytes.

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

- clippy's `all`, `pedantic` and `cargo` groups are on, plus a set of `restriction` lints chosen for an editor and to enforce [STYLE.md](code-standards/rust/STYLE.md). Each is listed with its reason, or the `S-…` rule it enforces. Thresholds, such as nesting depth and the banned names, live in `clippy.toml`.
- **No printing.** The terminal is the UI, and a stray `println!` or `eprintln!` corrupts the screen. Log or return errors instead.
- **No `unwrap()`, `panic!` or `std::process::exit` outside tests.** A panic loses the user's unsaved work, and `exit` skips the code that restores the terminal. Use `?`, or `expect("...")` stating the invariant when one really holds.
- **Docs where they add insight, not everywhere.** Document what a type is for, what a function promises, and when it panics (`# Panics` is linted). Don't restate what names and types already say (code-standards/rust/STYLE.md S-DOCS).
- **Fields are all public or all private.** Stateful structs keep every field private (code-standards/rust/STYLE.md S-FIELDS).
- **Stateful types state, check and fuzz their invariants.** A doc comment, a debug-only `check_invariants` called by every mutating method, and a property test (code-standards/rust/STYLE.md S-INVARIANTS).
- **Code stays flat.** Nothing nests deeper than 4 blocks (code-standards/rust/STYLE.md S-FLAT).
- **Unsafe is opt-in per site.** Mark it with `#[allow(unsafe_code, reason = "...")]` and put a `// SAFETY:` comment on every `unsafe` block.
- **Escape hatches are local.** Silence a lint with `#[allow(lint)]` on the smallest item that needs it, never crate-wide.

The toolchain is pinned in `rust-toolchain.toml`, so everyone sees the same lints. A toolchain bump goes in its own change, which fixes or allows (with a reason) any new lints it brings.

## Formatting

`just fmt` formats everything, and `just check` fails on anything it would change:

- Rust: rustfmt with the options in `rustfmt.toml`. The import options (code-standards/rust/STYLE.md S-IMPORTS) are nightly-only, so `just fmt` runs the nightly pinned as `fmt_toolchain` in the `justfile`, and `just toolchain` installs it. A plain `cargo fmt` on stable warns and skips them. Point your editor at the pinned nightly; `.helix/languages.toml` already does for Helix.
- `Cargo.toml` dependency tables: sorted alphabetically by `cargo sort`.
- Every TOML file: formatted by `taplo` (`taplo.toml`). It never reorders keys, so hand-grouped tables such as the lints keep their order.

## Commits

Commit messages follow code-standards/COMMITS.md S-COMMITS: `area: lowercase imperative summary`, at most 72 characters with no trailing period, then a blank line and a body saying why.

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
