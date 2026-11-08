# Review scope

How `style-review`, `perf-review` and `wend-review` decide what to review. They all read this file, so the syntax cannot drift between them. The argument is one word after the skill name, optionally followed by paths.

## The argument

| You type | What is reviewed | How to find it |
|---|---|---|
| *(nothing)* | The working tree: staged, unstaged and untracked changes, against `HEAD`. | `git diff HEAD`, and `git status --short` for untracked files. |
| `abc123` | The changes of that one commit. | `git diff abc123^ abc123` (for a root commit, `git show abc123`). |
| `abc123..def456` | What `def456` adds on top of `abc123`. This is commit onto commit. | `git diff $(git merge-base abc123 def456) def456`. |
| `main..feature` | What the branch adds on top of `main`, which is what a pull request shows. | The same merge-base diff. |
| `main..` | `main..HEAD`: the open right end means `HEAD`. Committed work only, never the working tree. | The same merge-base diff, against `HEAD`. |
| `..` or `all` | The whole codebase. | Every `.rs` file under `crates/`. |

Either side of `..` can be a short id, a branch, a tag or any ref git resolves. Never guess: resolve each side with `git rev-parse --verify <ref>^{commit}`. If a side doesn't resolve, or a short id is ambiguous, stop and say which one, with the matches. Don't fall back to another scope.

Review the working tree with no argument, and committed work with a range. They are separate on purpose: run both when you want both.

## Paths

Anything after the scope that is a path limits the scope to it: `main.. crates/wend-core/src/view` reviews only that directory's part of the branch, and `.. crates/wend-cli` reviews the whole of that crate. With paths and no scope, the working-tree changes under those paths are reviewed. A path that doesn't exist is an error, not an empty scope.

A named file with no scope and no changes is reviewed in full.

## What counts as in scope

- **Diff scopes** (the working tree, a commit, a range): read each changed file in full, because nesting depth and names only make sense in context, but **report only problems on the lines the change adds or modifies**. Mention an existing problem only if the change makes it worse.
- **The whole codebase:** every line counts. The list can be long, so group repeated problems into one finding that lists every location, and order the findings by rule and then by how many places they hit.
- **Deleted files and deleted lines** are never findings, but check that nothing still references what was removed.

## Commit messages

For a commit or a range, also review the messages against S-COMMITS: `git log abc123..def456 --format=%B` (for a single commit, `git log -1 --format=%B abc123`). The working tree and the whole codebase have no commits to review.

## Say what you resolved

Start the report by stating the scope you resolved, as one line: the refs and their full ids, or "working tree", or "whole codebase", plus how many files and lines it covers. If the scope is empty, say so and stop.

## Running tools

`just check`, `just clippy` and `just fmt-check` test the code that is checked out. They describe the scope only for the working tree, the whole codebase, and a commit or range that ends at `HEAD` with a clean working tree. For any other commit or range they would judge different code, so skip them and say so in the report. If the person wants them anyway, check the range's end out in a scratch git worktree in the scratchpad directory (`git worktree add --detach <path> <ref>`), run them there with `CARGO_TARGET_DIR` pointed at the main `target/` to reuse dependencies, and remove the worktree afterwards.
