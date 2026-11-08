---
name: wend-review
description: Review wend code against both STYLE.md and PERFORMANCE.md in one pass: runs just check, then style-review and perf-review, merges their findings into one numbered list and asks which to implement. Use when asked to review a change, a branch or files in wend, before committing, and from the commit skill. It never edits code.
---

# wend review

You run the whole review of a change: the mechanical gate, then `style-review`, then `perf-review`, as one report with one question.

**This skill never edits code.** It reads, runs checks, reports, and asks. Implementing the accepted fixes is separate work, done by whoever called you (the `commit` skill) or by the person, afterwards.

## 1. Scope

Resolve the argument as `scope.md` (in this folder) says: nothing is the working tree, `abc123` a commit, `a..b` a range, `..` or `all` the whole codebase, optionally limited by paths. The same file defines it for `style-review` and `perf-review`, and you pass them the scope you resolved.

- **Called from `commit`:** the working tree.
- **Touched files only (a re-review):** the files the caller names, nothing else.

State the resolved scope at the top of the report. If it is empty, say so and stop.

## 2. The mechanical gate

Run `just check` when `scope.md` says it describes the scope (see its "Running tools"), and otherwise skip it and say so. A failure is the first finding: summarize it in one line per failing step (fmt, clippy, doc, tests, deps) and don't repeat what the tools print. A change that doesn't build or pass its tests can't be committed, so say so plainly. If you can't run commands, say that and go on.

If a failure is not caused by the change (for instance `just deps` failing the same way on a clean checkout), say so and don't count it against the change.

## 3. Run both reviews

Invoke the `style-review` skill, then the `perf-review` skill, with the scope from step 1 and the instruction to **return findings and the rule checklist without asking** the question at their end. Use the Skill tool. If a skill is not available, read its `SKILL.md` and follow it yourself on the same scope.

## 4. Merge

Combine the findings into one list:

- Merge duplicates: when both reviews flag the same line for related reasons, make one finding that cites both rule ids.
- Order by how much they matter: build and test failures first, then correctness-adjacent ones (panics, errors, `S-PANICS`, `S-ERRORS`), then hot-path performance (`P-NO-ALLOC-HOT`, `P-BORROW`, `P-OUT-BUFFER`), then flatness, naming and the rest, and comments last.
- Number them `1.`, `2.`, … across both reviews so the person can pick by number.

## 5. Report

Start with a one-line verdict: how many findings, and whether the change is in line with both documents. Then the numbered findings, each as:

```
crates/wend-core/src/view/layout.rs:251  S-NAMES-HONEST
  `row_ptr` isn't a pointer, it's the row being written.
  → rename to `row_cursor`
```

Then: the combined rule checklist (every `S-…` and `P-…` id with its result), the hot paths the change touches, the tools' results (`just check` and what the reviews ran), and "Not covered" questions from both reviews.

## 6. Ask

End by asking what to implement: **all**, **some** by number, or **none**. Use the question tool if the harness has one, then stop and wait. Don't start on any fix in the same turn.
