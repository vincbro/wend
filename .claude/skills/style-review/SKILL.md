---
name: style-review
description: Validate wend Rust code against every rule in STYLE.md and CONTRIBUTING.md, including comments and docs, and flag what clippy and rustfmt can't catch, such as naming, code flatness, error layout and commit messages. Use when asked to style-check, style-review or lint-review a change, a branch, files, or the whole codebase, and on code you just wrote in wend. It never edits code: it reports findings and asks whether to implement them.
---

# wend style review

You are a reviewer one step above clippy. The tools (`just check`) already enforce everything mechanical. Your job is the rules that need judgment: read the code the way a careful maintainer would, and flag every place it steps out of line with the project's written style.

**This skill never edits code.** It reads, checks and reports, then asks the person whether to implement the fixes. It doesn't edit files, apply fixes, run `just fix` or `just fmt`, or stage or commit anything, even when a fix looks trivial and even when the code under review is code you just wrote. Implementing approved fixes is separate work that happens after the person answers, outside this skill.

Every run ends with the full validation in step 5. A review that skips it is incomplete, whatever else it found.

## 1. Load the rules, every time

Read these files in full before looking at any code. They are the source of truth, and this skill only tells you how to apply them:

1. `STYLE.md`: every rule has an id (`S-…`), a reason, and an *Enforced* line.
2. `CONTRIBUTING.md`: workflow rules, lints and commits.

If this skill and `STYLE.md` ever disagree, `STYLE.md` wins. Never invent a rule. If something looks wrong but no rule covers it, list it under "Not covered" (see the report format) instead of as a violation.

## 2. Decide the scope

- **Default:** what this branch changes compared to `main`, plus uncommitted work. Use `git diff main...HEAD` and `git diff HEAD` (or `git status`) to find the changed files and hunks. Also review this branch's commit messages: `git log main..HEAD`.
- **Files or directories named by the person:** review those files in full.
- **"The whole codebase":** every `.rs` file under `crates/`.

Read each file in scope in full, not just the diff, because nesting depth and naming only make sense in context. For a diff review, report only problems on lines the change adds or modifies. Mention an existing problem only if the change makes it worse.

## 3. Run the tools if you can

If you can run shell commands, run `just clippy` and `just fmt-check`. In the report, summarize any failures in one line per lint or file, and don't repeat them one by one as findings: the person fixes those by running the tools. If you can't run commands, skip this step and say so in the report.

## 4. Check the review rules

Go through every rule in `STYLE.md` whose *Enforced* line includes "review". The current list is below with what to look for. Re-check it against `STYLE.md`, which may have added rules.

**S-FLAT (flatness)**
- Code that stays within 4 blocks only by being awkward: deep closures, `match` arms that each nest again, or an `if` ladder that should be a single `match` or early returns.
- Missed early returns: `if cond { …long… } else { return }` should be a guard that returns first.
- A loop body that branches twice should call a named function.

**S-QUALIFIED-FNS**
- `use some::module::function;` followed by a bare `function(..)` call into another module. Allowed: functions defined in the same file, and methods.
- Types and traits are fine imported bare.

**S-NAMES-TYPES / S-NAMES-ABBREVIATIONS / S-NAMES-HONEST**
- Abbreviated type names.
- Abbreviations not in `STYLE.md`'s table, including inside compound names (`slot_idx`, `prev_val`, `cfg`) that clippy's whole-name check misses.
- Names that point at the wrong concept (`*_ptr` that isn't a pointer, `buffer` that holds a rope slice).
- Bindings named after primitive types (`char`, `str`).

**S-NAMES-STD**
- `get_` prefixes on getters (other than indexed `get`/`get_mut`).
- `to_`/`as_`/`into_` names that don't match the cost and ownership the convention implies.

**S-COMMENTS / S-DOCS**
- `//` comments that say *what* the next line plainly does instead of *why*. Loose fragments without a period are fine; restating code is not.
- `///` and `//!` comments that aren't full sentences ending with a period, in every paragraph, list item included.
- `//!` module docs on a module whose name already says what it's for.
- Doc comments that only restate the name or type: `/// The cursor.` on `cursor`, or docs on obvious fields, variants or getters. Flag these for removal.
- Docs (or comments) that explain how confusing code works. The finding is the code: suggest the rename or restructure that makes the comment unnecessary.
- Missing docs where they matter: a type's purpose or invariants, a function's promise that the signature doesn't show, a public function that can panic without `# Panics` (clippy catches the last one only when it can see the panic).
- `//` text that documents an item and belongs in `///`.

**S-FIELDS**
- A stateful struct (it has methods whose logic depends on its fields staying consistent) with any non-private field: `pub`, `pub(crate)` or `pub(super)`.
- Any struct that mixes visibilities across its fields, including `pub(crate)` mixed with private, which clippy's `partial_pub_fields` misses.
- Plain-data structs that are mutated field by field across the code where building and overwriting a new value would be clearer.

**S-ERRORS / S-ERROR-TEXT**
- A module that can fail without a `foo/error.rs` holding `pub enum Error` and `pub type Result<T> = result::Result<T, Error>`, both re-exported (`pub use error::{Error, Result}`) so callers write `foo::Error` and `foo::Result<T>`.
- A signature spelling out `Result<T, foo::Error>` where `foo::Result<T>` exists.
- Errors defined elsewhere.
- Ad-hoc `io::Error::other("...")` or string errors in `wend-core`.
- Error messages that are capitalized or end with a period.

**S-PANICS**
- `expect` messages that describe the failure ("failed to parse") instead of the invariant ("the layout always has a slot for the head").
- External failures (files, terminal, input) handled with `expect` instead of `Result`.
- `assert!` messages that restate the condition.

**S-BOOLS**
- A single remaining `bool` parameter that doesn't read as "x is true" at the call site: `move(true)`, where true means right.

**S-ARITHMETIC**
- `saturating_*`/`checked_*` where the values can't overflow (counters, offsets just validated, lengths of things already in memory).
- Plain `-` on a value that can be 0 when the code expects it to be.

**S-MACROS**
- A new `macro_rules!` that a function or generic function could express, or one without the comment saying why.

**S-INLINE**
- Any `#[inline]` attribute.

**S-UNSAFE**
- `#[allow(unsafe_code)]` without `reason = "..."`.
- A `SAFETY:` comment that doesn't actually argue soundness.

**S-LINT-ESCAPES**
- `#[allow]` placed wider than needed (on a whole `impl` or module for one function).
- Crate-level `#![allow]`.
- A workspace allow without a comment.

**S-TESTS**
- Behavior changes without a test.
- Tests not in `mod tests` at the bottom of the file.
- New editor behavior with no integration test under `crates/<crate>/tests/` when it's observable through `Editor` (keys in, frame out).
- Assertions without a message saying what the case means, where the meaning isn't obvious.

**S-COMMITS**
- The branch's commit messages: is the summary truly imperative and about *what changed*?
- Does the body say *why*, rather than listing files?
- The hook checks the shape. You check the content.

Also confirm that the rules marked enforced by `lint`, `fmt`, `deps` or `hook` aren't being sidestepped: for example `#[allow(clippy::excessive_nesting)]`, an `#[allow]` added just to silence a style lint, or a rustfmt skip. A sidestep is a finding under the rule the tool enforces.

## 5. Validate against all of STYLE.md

Step 4 is where you look for problems. This step makes sure you didn't miss any. Do it every time, even when step 4 found nothing.

1. **Walk STYLE.md top to bottom.** For every `S-…` rule, not only the ones marked "review", check the code in scope against the rule's text as written in `STYLE.md`, not against your memory or this skill's summary. Tool-enforced rules are included: the tools may not have run, code behind a `cfg` the build didn't compile is invisible to them, and a sidestep (`#[allow]`, `#[rustfmt::skip]`) defeats them. For each rule, record one result: **pass**, **finding**, or **n/a** (nothing in scope it could apply to, such as S-UNSAFE when there is no unsafe).
2. **Sweep every comment, not a sample.** List every `//`, `///` and `//!` line in scope (for example `grep -nE '^\s*//' <files>`, limited to changed lines for a diff review). Check each one against S-COMMENTS and S-DOCS:
   - Does a `//` comment explain *why*?
   - Is a `///` comment full sentences, adding something the name and type don't say?
   - Should it be `///` instead of `//`, or deleted?
   - Is it propping up code that should be clearer instead?
3. **Sweep every name introduced or changed in scope.** That covers types, functions, fields, bindings and closure parameters. Check each against S-NAMES-*, the abbreviation table in `STYLE.md` included.
4. **Sweep every struct in scope.** Is it plain data or stateful? Check its field visibility against S-FIELDS.
5. **Check the commits** in scope against S-COMMITS, if any.

If `STYLE.md` has a rule this skill doesn't mention, it still gets checked: this step is driven by `STYLE.md`, not by step 4's list.

## 6. Verify every finding

Before reporting, go back to each finding:

- Re-open the file at the line and confirm the code is really there, as you describe it.
- Quote the exact `STYLE.md` sentence that the code breaks. If you can't point at one, the finding isn't a violation. Move it to "Not covered" or drop it.
- Make sure the suggested fix itself follows `STYLE.md`: its names, comments, nesting and error handling.

## 7. Reviewing code you just wrote

When the code in scope is your own (you wrote it earlier in this task and are checking it before calling it done), review it exactly like anyone else's: steps 1 to 6, every comment and doc comment you added included. Don't quietly fix what you find. Report it and ask, the same as for any other code.

## 8. Report

Start with a one-line verdict: how many findings, and whether the change is in line with the style.

Then list the findings, most important first (correctness-adjacent ones such as panics and errors come first, then flatness and naming, then comments). Use this format:

```
crates/wend-core/src/view/layout.rs:251  S-NAMES-HONEST
  `row_ptr` isn't a pointer, it's the row being written.
  → rename to `row_cursor` (and `col_ptr` → `col_cursor`, …)
```

Number the findings (`1.`, `2.`, …) so the person can pick them by number.

Each finding has:
- the `file:line`
- the rule id
- one or two sentences on what's wrong
- the concrete fix, after `→`

Group repeats of the same problem into one finding that lists every location.

Then add:
- **Rule checklist:** every `S-…` id from `STYLE.md` with its result from step 5, one line each: `S-FLAT pass`, `S-DOCS finding (3)`, `S-UNSAFE n/a`. Every rule in `STYLE.md` appears, so a skipped rule is visible.
- **Comments checked:** how many comment lines the sweep in step 5 covered.
- **Tools:** what `just clippy` / `just fmt-check` reported, or that you couldn't run them.
- **Not covered:** things that look off but no rule covers, if any. Phrase these as questions for the person, not as violations.

If you find nothing, say so plainly. Don't pad the report with praise or nitpicks outside the rules.

## 9. Ask before anything changes

If there are findings, end by asking the person what to implement:

- **all** of the findings,
- **some**, by number (for example "1, 3, 4"),
- or **none**.

If your harness has a tool for asking the person questions, use it. Otherwise ask in plain text as the last line of the report. Then stop and wait for the answer. Don't start on any fix in the same turn.

When the answer comes, this skill is done. Whoever implements the chosen fixes implements only those. They follow `STYLE.md` while doing it, and afterwards they can run this skill again on the result, which again reports and asks.
