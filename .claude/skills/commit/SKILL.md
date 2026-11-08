---
name: commit
description: Read the working-tree diff of wend, split it into logical commits and write each commit message in the STYLE.md S-COMMITS format (`area: lowercase imperative summary`, then a body saying why). Use when asked to commit, write commit messages, split a diff into commits, or /commit. It proposes the split and messages first and only stages or commits after the person agrees.
---

# wend commit

You turn the current changes into well-formed commits. The format is STYLE.md **S-COMMITS**. Read that rule and `.githooks/commit-msg` (which calls `just commit-msg`) before writing anything, and follow them over this skill if they disagree.

**Nothing is staged, committed, amended or pushed until the person has agreed to the exact plan.** Reading (`git status`, `git diff`, `git log`) is always fine.

## 1. Read the changes

Run `git status --short`, `git diff HEAD` (staged and unstaged) and `git log -5 --format='%h %s'`. Read untracked files that look like part of the work. If there is nothing to commit, say so and stop.

If the current branch is `main`, mention it in the plan and ask whether to commit there or branch first.

## 2. Split into commits

Group the changes by *reason*, not by directory. One commit is one idea that could be reverted on its own.

- A rename or file move with no behavior change is its own commit, apart from the change that uses it.
- Formatting-only changes (`just fmt`) are their own commit, apart from the logic.
- `Cargo.lock` and `Cargo.toml` changes go with the commit that needs them.
- Tests go with the behavior they cover.
- Prefer whole files per commit. Only split a file across commits when its hunks really belong to different ideas, and then stage them with `git apply --cached` from a patch you write to the scratchpad, since `git add -p` is interactive and not available.
- If the changes are one idea, make one commit. Don't split for the sake of it.

Order the commits so each one leaves the tree building, with the foundation first.

## 3. Write the messages

Per S-COMMITS:

- First line is `area: summary`. The area is the module or topic (`view`, `layout`, `gutter`, `buffer`, `syntax`, `cli`, `ci`, `deps`, `docs`, `style`). The summary is lowercase except for code and proper nouns, starts with an imperative verb, has no trailing period and is at most 72 characters (ideally 50).
- A blank line, then a body in prose that says **why**: what was wrong or missing and the idea of the fix. No list of files. Wrap at 72 characters. Leave the body out when the summary says it all.
- End the message with the attribution trailer the harness gives for commits in this session, when it gives one.

Write each message to a file in the scratchpad directory and check it with `just commit-msg <file>`. Fix it until the check passes.

## 4. Ask before doing anything

Show the plan: for each commit, in order, the message and the files (or hunks) it contains. Note anything you left out of every commit and why, such as a stray build artifact or a file that looks secret. Never include secrets or credentials.

Then ask the person what to do, with the question tool if the harness has one:

- **commit all** as planned,
- **change the plan**, which they describe, and you show the new plan before asking again,
- **messages only**: print the messages and stop without touching git.

Do not stage or commit anything before the answer.

## 5. Commit

After approval, for each commit in order: stage exactly its files, run `git commit` with the approved message, and check it succeeded. If the commit-msg hook rejects it, show the hook's output, fix the message, and ask again only if the fix changes the meaning.

Never use `--no-verify`, `--amend`, `git reset`, `git push` or `git add -A`. If something fails or the tree isn't what you expected, stop and report with `git status`. Finish by showing `git log --oneline` for the new commits.
