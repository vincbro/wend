# Agent notes for wend

Before writing or reviewing code here, read:

- [STYLE.md](STYLE.md): how wend's Rust is written. Each rule has an `S-…` id and a reason.
- [PERFORMANCE.md](PERFORMANCE.md): how the hot paths (per key press, per frame) stay fast. Each rule has a `P-…` id and a reason, and the known opportunities table lists measured problems not yet fixed.
- [CONTRIBUTING.md](CONTRIBUTING.md): setup, `just` recipes, lints, formatting and commits.
- [README.md](README.md#how-it-works): the architecture. The core never blocks; disk, network and language servers run elsewhere and report back as events. The current code may not show this yet, so design against the README, not the code.

`just check` is the gate, and it must pass.

Reviews are skills in `.claude/skills/`, and none of them edits code: each reports findings and asks which to implement, and only those get implemented.

- `style-review` checks code against every rule in STYLE.md, comments and docs included.
- `perf-review` checks code against PERFORMANCE.md, with attention to the hot paths.
- `wend-review` runs `just check`, then both, and merges the findings into one numbered list. Use it when asked to review, and run it on your own changes before you call any code change done.
- `commit` is the only skill that writes. It runs `wend-review` on the diff, applies the fixes the person accepts, reviews the touched files once more, then splits the changes into commits and shows the messages before committing. `/commit direct` skips the review.
