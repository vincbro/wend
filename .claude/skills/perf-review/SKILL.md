---
name: perf-review
description: Validate wend Rust code against every rule in PERFORMANCE.md, mostly the hot paths that run per key press and per frame, and flag allocation, copying, oversized types and unbuffered output that clippy can't catch. Use when asked to performance-review a change, a branch or files, and as part of wend-review. It never edits code: it reports findings and asks which to implement, or returns them to wend-review.
---

# wend performance review

You check code against the rules in `PERFORMANCE.md`, the way a careful maintainer who cares about the hot paths would. `style-review` covers `STYLE.md`; you cover only `PERFORMANCE.md`.

**This skill never edits code.** It reads, checks and reports. Implementing fixes is separate work that happens after the person (or the `commit` skill) has accepted them.

**When another skill invokes you** (`wend-review`), return the findings and the rule checklist and **do not ask** the question in step 6. That skill asks once for both reviews.

## 1. Load the rules, every time

Read `PERFORMANCE.md` in full, including the *Known opportunities* table. It is the source of truth: if this skill and `PERFORMANCE.md` disagree, `PERFORMANCE.md` wins. Never invent a rule. Something that looks slow but no rule covers goes under "Not covered".

## 2. Decide the scope

Read `../wend-review/scope.md` and resolve the argument as it says: nothing is the working tree, `abc123` a commit, `a..b` a range, `..` or `all` the whole codebase, optionally limited by paths. State the scope you resolved at the top of the report.

When `wend-review` or `commit` invokes you, use the scope they pass.

## 3. Find the hot paths

The hot paths are the code that runs per key press and per frame, as `PERFORMANCE.md` defines them: `Editor::apply`, `Keymap::press`, `Layout::rebuild`, `View::render` and the frontend's draw. Trace what the changed lines call and what calls them, with the graph and rust-analyzer tools when they are available and with `grep` otherwise. Say in the report which changed code is on a hot path and which is not.

## 4. Check the rules

Go through every rule in `PERFORMANCE.md` whose *Enforced* line includes "review". Re-check against the file, which may have more rules than listed here:

- **P-MEASURE:** a change described as faster has numbers in the commit body or a benchmark. A comment on non-obvious optimized code cites the measurement. Nothing from the "only with a benchmark" list (hashers, `SmallVec`, allocators, `target-cpu`) is added without one.
- **P-BOXED:** a new field, enum payload or message that never grows is `Box<[T]>`, `Box<str>`, `Box<Path>` or `Arc<str>`, not `Vec`, `String` or `PathBuf`. A reused scratch buffer is allowed to be a `Vec`.
- **P-NO-ALLOC-HOT:** `Vec::new`, `vec![]`, `String::new`, `format!`, `to_string`, `to_owned`, `clone` of an owning type, `collect` and `Box::new` inside a hot path.
- **P-BORROW:** a hot function that returns data that already exists but returns an owned copy or a `Vec`.
- **P-OUT-BUFFER:** a hot function that builds a collection but returns it, instead of clearing and filling a buffer it is given. Also a function that takes a buffer and appends without clearing, with no `extend_` name.
- **P-REUSE and P-CAPACITY:** a collection rebuilt per call that could live in the calling struct, `a = b.clone()` where `clone_from` fits, and growing a `Vec` whose size is known.
- **P-SIZES:** a new hot type that is large, or a large enum variant that is not boxed. Say how big it is when you can tell.
- **P-ITERATORS:** `collect` only to iterate again, indexing in a loop that an iterator expresses, `filter` then `map`, `chunks` where `chunks_exact` fits, a custom iterator without `size_hint`.
- **P-OUTPUT:** terminal output that is not one locked, buffered writer flushed once per frame.
- **P-BUILD:** a change to a profile in `Cargo.toml` that undoes the shipped settings, or an inline attribute (also S-INLINE).

Do not repeat a problem already in the *Known opportunities* table. Report it only if the change makes it worse, and say if the change fixes a row, so the row can be removed.

## 5. Verify every finding

Go back to each finding before reporting:

- Re-open the file at the line and confirm the code is there as you describe it.
- Quote the exact `PERFORMANCE.md` sentence the code breaks. If you can't, it is not a violation: move it to "Not covered" or drop it.
- Confirm the code really is on a hot path, if the rule is about hot paths.
- Make sure the suggested fix follows `PERFORMANCE.md` and `STYLE.md` (names, comments, nesting, error handling).

## 6. Report

Start with a one-line verdict: how many findings, and whether the change is in line with `PERFORMANCE.md`.

List the findings most important first, numbered so they can be picked, in this format:

```
crates/wend-core/src/editor/keymap.rs:38  P-NO-ALLOC-HOT  (hot: Editor::apply)
  `actions.clone()` allocates a Vec on every key press.
  → return `&[Action]` from the leaf (P-BORROW) and iterate it
```

Group repeats of the same problem into one finding with every location. Then add:

- **Rule checklist:** every `P-…` id with `pass`, `finding (n)` or `n/a`.
- **Hot paths touched:** the changed functions that are on one, or "none".
- **Tools:** which tools you used to trace callers, or that you used `grep`.
- **Not covered:** things that look slow but no rule covers, as questions for the person.

If you find nothing, say so plainly.

## 7. Ask before anything changes

Unless another skill invoked you, end by asking what to implement: **all**, **some** by number, or **none**. Use the question tool if the harness has one. Then stop and wait. Don't start on any fix in the same turn.
