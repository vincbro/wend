# wend Rust style

How wend's Rust is written. Most of it is enforced: `just check` fails on anything rustfmt, clippy, rustdoc or cargo-deny can catch. The rest is checked in review, by people or by the `style-review` agent skill (`.claude/skills/style-review/SKILL.md`). Every rule has an ID (`S-…`) so a review can cite it.

Each rule says what to do, **why**, and what enforces it:

- `fmt`: `just fmt-check`, rustfmt with the options in `rustfmt.toml`
- `lint`: `just clippy`, configured in `[workspace.lints]` and `clippy.toml`
- `deps`: `just deps`, cargo-deny with `deny.toml`
- `hook`: the commit-msg hook and the `commits` CI job
- `review`: nothing automatic. Reviewers and the style-review skill check it.

A rule that a tool enforces is not up for debate in review: fix the code. A rule marked `review` is the one place judgment comes in, so a reviewer flags it with the rule ID and the author either fixes it or explains why this case is the exception.

---

## Layout

### S-FLAT: Keep code flat

No line of code sits deeper than **4 blocks**. Every enclosing block counts: `mod`, `impl`, `fn`, `if`, `match`, loops and closures. In a method, the body is depth 2, so you get one branch (3) with at most one more branch inside it (4). Nothing deeper.

To stay flat:

- **Return early.** Use `let ... else { return }`, `?`, and guard `if`s that `return`/`continue`/`break` first.
- **Match once.** Use one `match` on a tuple or enum instead of nested `if`/`if let`.
- **Extract.** Give the inner branch its own function with a name that says what it decides.
- **Name the loop body.** A loop whose body branches twice becomes a loop that calls a function.

*Why:* every level of nesting is one more condition the reader has to hold in their head to know when a line runs. Flat code reads top to bottom: each guard rules out a case, and what's left is the main path.

*Enforced:* lint (`clippy::excessive_nesting`, `excessive-nesting-threshold = 4`), plus `collapsible_if`, `collapsible_else_if`, `manual_let_else` and `needless_continue` from pedantic. Review flags code that passes the limit only by being awkward, e.g. deep closures used to dodge it.

### S-MOD-FILES: `foo.rs` next to `foo/`, never `mod.rs`

A module with submodules is `foo.rs` plus a `foo/` folder.

*Why:* every file is named after its module, so editor tabs and the fuzzy finder never show five files called `mod.rs`.

*Enforced:* lint (`clippy::mod_module_files`).

### S-MOD-CONTENT: A parent module may hold its main type

`view.rs` holds `View`, and `view/` holds the pieces `View` is built from. A parent file isn't limited to `mod` lines and re-exports.

*Why:* the most important code lives in the most obvious file, with no indirection.

*Enforced:* none needed, since it's a permission.

### S-IMPORTS: One nested `use` per crate, grouped std → external → crate

```rust
use std::{
    fs::{self, File},
    io::{self, Write},
};

use crop::Rope;

use crate::{area::Area, grapheme::Grapheme};
```

*Why:* the grouping reads from the most general to the most local. One block per crate keeps the top of the file short, and because rustfmt writes it, nobody hand-sorts imports.

*Enforced:* fmt (`imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`). These options need nightly rustfmt, which `just fmt` runs at a pinned date. See CONTRIBUTING.

### S-QUALIFIED-FNS: Call other modules' functions through their module

Write `mem::swap(..)`, `fs::read(..)`, `area::saturating_u16(..)`. Import the module, not the function. Types and traits are imported and used bare: `Rope`, `Buffer`.

*Why:* a bare lowercase call always means "defined in this file". The prefix tells you where everything else comes from, without jumping to the definition.

*Enforced:* lint (`clippy::absolute_paths` with `absolute-paths-max-segments = 2` stops `std::mem::swap(..)` written out in full). Review catches functions imported by name (`use std::mem::swap;`).

---

## Naming

### S-NAMES-TYPES: Never abbreviate type names

`CellPosition`, `ScrollOffset`, `SelectionError`, not `CellPos` or `ScrollOff`.

*Why:* a type name is read everywhere the type is used, far from its definition.

*Enforced:* review.

### S-NAMES-ABBREVIATIONS: Elsewhere, only abbreviations every Rust reader knows

These are allowed in variables, fields, functions and parameters:

| Abbreviation | Meaning |
|---|---|
| `buf` | buffer of bytes or chars |
| `len` | length |
| `cx` | context, as in std's `Context` |
| `tx`, `rx` | channel ends |
| `err` | an error value |
| `args` | arguments |
| `f` | a `fmt::Formatter` or a closure parameter |
| `c` | a `char` |
| `i`, `j`, `n`, `x`, `y` | counters and coordinates |
| `pos`, `col`, `cols`, `row`, `rows` | positions and terminal coordinates |
| `min`, `max`, `prev`, `next` | bounds and neighbours |

Anything else is spelled out: `index` not `idx`, `value` not `val`, `result` not `res`.

*Why:* the abbreviations every Rust codebase uses cost a reader nothing. Project-invented ones cost every new reader a moment of decoding, and names are read far more often than they're typed.

*Enforced:* lint for whole names (`clippy::disallowed_names`: `idx`, `ptr`, `tmp`, `temp`, `val`, `res`, `ret`), and lint for single letters outside the table (`clippy::min_ident_chars`). Review catches compounds such as `slot_idx` and `row_ptr`.

### S-NAMES-HONEST: A name says what the thing is

`row_cursor`, not `row_ptr`, when it isn't a pointer. A binding doesn't reuse a primitive type's name, so no `char` or `str` as variable names.

*Why:* a name that points at the wrong concept is worse than a vague one, because the reader trusts it.

*Enforced:* review.

### S-NAMES-STD: Follow std's naming conventions

- Getters are `foo()`, `foo_mut()`, `set_foo()`, `take_foo()`, with no `get_` prefix. `get`/`get_mut` are kept for indexed lookups, as on `Frame`.
- Conversions are `as_` (cheap borrow), `to_` (costly or owned) and `into_` (consumes self).
- Predicates are `is_`/`has_`. If there's a `len`, there's an `is_empty`.
- Constructors are `new` and `from_foo`. A type with `new()` and no arguments also implements `Default`.

*Why:* readers already know these from std, so a name tells them the cost and ownership of a call before they read it.

*Enforced:* lint (`wrong_self_convention`, `new_without_default`, `len_without_is_empty`). Review catches `get_` prefixes.

### S-NAMES-LIFETIMES: `'a` is fine

Short lifetime names are fine. Prefer `'_` wherever elision allows it.

*Enforced:* lint (`elided_lifetimes_in_paths`, `needless_lifetimes`).

---

## Comments and docs

### S-COMMENTS: `//` comments are notes

Implementation comments can be loose fragments with no trailing period. They explain **why**, never what the next line plainly does.

*Why:* low friction means comments actually get written. Their job is to carry the reasoning the code can't show.

*Enforced:* review flags comments that only restate the code.

### S-DOCS: The code is the documentation; docs add what it can't say

Good names and types explain what code does. A `///` comment is for what they can't explain:

- what a type is for, and the invariants it keeps
- what a function promises, and anything a caller can't see from the signature
- **`# Panics`**: when a public function can panic, and on what input
- a short example, where showing beats telling

Don't document what the name and type already say. Not every public field, enum variant or obvious getter needs a comment. `/// The cursor position.` on a field named `cursor: CellPosition` is noise. When a doc comment is explaining *how confusing code works*, the fix is clearer code, not more docs.

When you do write one, it's full sentences ending with a period. Use `//!` at the top of a module file only when the module's purpose isn't obvious from its name. `///` text that belongs to an item never goes in a `//` comment, where rustdoc can't see it.

*Why:* documentation that's required everywhere gets written everywhere, and it then excuses unclear code ("it's documented") and rots as the code changes. Docs that exist only where they add insight get read, because readers learn that each one matters.

*Enforced:* lint (`clippy::missing_panics_doc`, `clippy::doc_paragraphs_missing_punctuation`, `clippy::doc_markdown`, and every rustdoc lint through `just doc`). Review catches docs that restate the code or prop up unclear code, missing docs where an invariant or promise isn't visible in the code, and `//` used where `///` belongs.

### S-DOC-EXAMPLES: Simple examples are welcome, not required

Add a short example where seeing a call explains a type or function faster than prose. Leave it out where it wouldn't.

*Why:* a good example is the fastest documentation there is, but a required one becomes boilerplate.

*Enforced:* none needed.

---

## Errors and panics

### S-ERRORS: Each failing module owns a `thiserror` enum in `error.rs`

A module whose functions can fail has an `error.rs` submodule with one enum, named plain `Error`, that is re-exported:

```rust
// buffer/error.rs
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file is not valid utf-8")]
    InvalidUtf8,
    #[error("file i/o failed")]
    Io(#[from] io::Error),
}

// buffer.rs
mod error;
pub use error::Error;
```

Callers refer to it through its module, `buffer::Error`, the same way std has `io::Error` and `fmt::Error`. A leaf module that gains errors becomes `foo.rs` plus `foo/error.rs` (see S-MOD-FILES).

`wend-core` returns these typed errors. The `wend` binary uses `anyhow`: it adds `.context("…")` and returns from `main` when it reaches a state it can't recover from.

*Why:* typed errors let the code that can do something about a failure `match` on exactly what went wrong. The binary is where errors stop being handled and start being shown, and that's what `anyhow` is good at. One file per module means you always know where a module's failure modes are listed.

*Enforced:* deps (cargo-deny allows `anyhow` only as a dependency of `wend`). Review checks that each error lives in `error.rs` and is named `Error`.

### S-ERROR-TEXT: Error messages are lowercase, with no trailing period

`"file is not valid utf-8"`, not `"File is not valid UTF-8."`.

*Why:* errors get chained, as in `failed to save: file is not valid utf-8`. Lowercase fragments compose into one line. Capitalization happens where a message is displayed.

*Enforced:* review.

### S-PANICS: A panic is a bug, and every possible panic says why it can't happen

- No `unwrap()` outside tests. Use `expect("why this holds")`, where the message states the invariant, not the failure.
- No `panic!` outside tests. A `let ... else { panic!(..) }` becomes `.expect(..)`.
- A function that returns `Result` doesn't panic on its inputs. It returns an error.
- Failure that comes from outside the program (files, the terminal, user input) is a `Result`, never a panic.
- An `assert!` explains why it must hold, not what it checks.

*Why:* in an editor, a panic loses the user's unsaved work. If one ever fires, the message should name the assumption that broke, using only the panic output.

*Enforced:* lint (`unwrap_used`, `panic`, `unwrap_in_result`, `get_unwrap`, with tests exempt through `clippy.toml`). Review checks that `expect` messages state the invariant and that external failures aren't treated as bugs.

---

## API design

### S-BOOLS: A `bool` parameter must read as "x is true"

`extend: bool` is fine: "extend is true" means something, and so does false. `right: bool` is not, because "right is false" doesn't clearly mean left. Use an enum (`Direction::Forward`/`Backward`). A function takes at most one `bool`. Several flags become a struct with named fields, or enums.

*Why:* at the call site you only see `true`. That's readable only if the parameter's name makes both values obvious. An enum can't be passed the wrong way round and leaves room for a third case.

*Enforced:* lint (`clippy::fn_params_excessive_bools` with `max-fn-params-bools = 1`). Review checks that a single remaining `bool` passes the "x is true" test.

### S-INPUTS: Borrow concrete views; no conversion generics

- For read-only input, take the borrowed view: `&str`, `&Path`, `&[T]`, `Option<&T>`. Don't take `&String`, `&Vec<T>`, `&Option<T>` or `&Box<T>`.
- For input the function changes, take `&mut` of the owned type: `&mut String`, `&mut Vec<T>`.
- Take ownership only when the function keeps the value.
- No `AsRef<..>` or `Into<..>` bounds: callers write `&x` or `.into()` themselves.

*Why:* the signature says exactly what the function does with its input. Every caller is in this repo, so call-site convenience isn't worth a generic, which costs a harder signature and a compiled copy per type.

*Enforced:* lint (`ptr_arg`, `ref_option`, `borrowed_box`, `needless_pass_by_value`, and `disallowed_types` banning `AsRef`/`Into` bounds).

### S-FIELDS: Fields are all public or all private, and logic means private

wend is written data-oriented, not object-oriented. A struct is one of two kinds:

- **Plain data.** For example `Area`, `Rect`, `CellPosition`, `Cell`. No invariant ties its fields together, so any combination of values is valid. Its fields may all be public (at the struct's own visibility), and it's usually `Copy`. You don't mutate it in place: you build a new value and overwrite the old one.
- **Stateful.** For example `Buffer`, `View`, `Layout`, `Editor`. It has logic that depends on its own fields staying consistent: the selection on grapheme starts, the layout matching the buffer, `dirty` matching the rope. **All of its fields are private**, and the only way to change them is through methods that keep the invariants.

Most structs are the stateful kind, so the default is private fields. A struct never mixes the two kinds: if one field needs to be private, they all are, and access goes through getters (S-NAMES-STD).

*Why:* the moment a stateful struct has a public field, any code anywhere can break its invariants, and the struct can no longer vouch for its own state. `View::check_invariants` can't hold if a caller can write `view.selection` directly. Plain data has no invariants to protect, so hiding its fields would only add getters with nothing behind them. Mixing the two tells the reader neither story.

*Enforced:* lint (`clippy::partial_pub_fields` catches `pub` mixed with private). Review catches `pub(crate)`/`pub(super)` mixed with private (which the lint doesn't see), and stateful structs whose fields are all public.

### S-VISIBILITY: The narrowest visibility that works

`pub` means part of `wend-core`'s published API. Everything else is private, `pub(super)` or `pub(crate)`.

*Why:* a small public surface is one you can change without breaking anyone, and the compiler then tells you when something became public by accident.

*Enforced:* lint (`unreachable_pub`, `unnameable_types`).

### S-MUST-USE: `#[must_use]` wherever it applies

Every function whose result is the point of calling it gets `#[must_use]`.

*Why:* pedantic finds every candidate, so the rule takes no thought and is never inconsistent.

*Enforced:* lint (`clippy::must_use_candidate`, `double_must_use`).

---

## Implementation

### S-ARITHMETIC: Plain arithmetic unless overflow is a real case

Write `+`, `-`, `*` where the values can't overflow. Use `saturating_*` or `checked_*` only where overflow or underflow is an expected input (a cursor at column 0 moving left, a size from the terminal), and say so in a comment if it isn't obvious.

*Why:* debug builds panic on overflow, so a broken assumption shows up in tests. Saturating hides it: the value sticks at `0` or `usize::MAX` and the bug appears somewhere else. And `saturating_add` on a counter that can't overflow suggests that it could.

*Enforced:* review.

### S-ITERATORS: Iterator chains are welcome

Use combinators freely when they read well.

*Enforced:* none needed.

### S-MACROS: Functions first, then generic functions, then macros

Reach for a macro only when a function, or a generic function, can't express it: real code generation. A `macro_rules!` comes with a comment saying why a function wouldn't do.

*Why:* macros break go-to-definition and refactors, and they create a dialect every reader has to learn.

*Enforced:* review.

### S-INLINE: No `#[inline]`

*Why:* the shipped `dist` build uses fat LTO, which inlines across crates anyway. Hand annotations are guesses that never get re-checked.

*Enforced:* review (`inline_always` is also linted).

### S-UNSAFE: Unsafe opts in per site, with a reason and a `SAFETY:` comment

Mark the item with `#[allow(unsafe_code, reason = "...")]`, and put a `// SAFETY:` comment on every `unsafe` block saying why it's sound.

*Why:* unsafe is rare in wend, and each use should be easy to find and to check.

*Enforced:* lint (`unsafe_code`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `unsafe_op_in_unsafe_fn`). Review checks that the `reason` is there.

### S-LINT-ESCAPES: Silence a lint with `#[allow(lint)]`, as locally as possible

Put it on the smallest item that needs it, never at the crate level. A workspace-wide allow goes in `[workspace.lints]` with a comment.

*Why:* simple, and it never breaks the build. Keeping it local keeps the exception visible where it applies.

*Enforced:* review.

---

## Tests

### S-TESTS: Inline unit tests plus end-to-end integration tests

- Unit tests live at the bottom of the file they test, in `#[cfg(test)] mod tests { use super::*; … }`. They cover exact edge cases, like layout and grapheme handling.
- Integration tests live in `crates/<crate>/tests/`. They drive `Editor` the way a user does: keys in, frame out.
- Test names describe the behavior, with no `test_` prefix: `wide_glyph_wraps_whole`.
- An assertion's message says what the case means: `assert_eq!(a, b, "snaps into the grapheme start")`.
- `unwrap` and `panic!` are fine in tests.

*Why:* unit tests pin down the details, and integration tests catch what a user actually hits.

*Enforced:* lint (`tests_outside_test_module`, `redundant_test_prefix`). Review checks that changes to behavior come with tests.

---

## Commits

### S-COMMITS: `area: lowercase imperative summary`

```
view: scroll by row instead of by line

Scrolling moved whole lines, so a line that wrapped past the bottom of
the screen could never be scrolled through. Scroll one row at a time.
```

- The first line is `area: summary`, where `area` is the module or topic (`view`, `layout`, `buffer`, `cli`, `ci`, `deps`, `docs`, `style`). The summary is lowercase except for code and proper nouns, starts with an imperative verb, has no trailing period, and is at most 72 characters (ideally 50).
- A blank line follows, then a body saying **why**: what was wrong or missing, and the idea of the fix. Use prose, not a list of files. Wrap at 72 characters. Leave the body out when the summary says it all.

*Why:* `git log --oneline` then scans by area, and the body records the reasoning the diff can't show.

*Enforced:* hook (`just hooks` installs a commit-msg check; the `commits` CI job checks every commit in a PR).
