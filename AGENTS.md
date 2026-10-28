# Agent notes for wend

Before writing or reviewing code here, read:

- [STYLE.md](STYLE.md): how wend's Rust is written. Each rule has an `S-…` id and a reason.
- [CONTRIBUTING.md](CONTRIBUTING.md): setup, `just` recipes, lints, formatting and commits.

`just check` is the gate, and it must pass. The `style-review` skill in [.claude/skills/style-review/SKILL.md](.claude/skills/style-review/SKILL.md) validates code against every rule in STYLE.md, comments and docs included. Follow it when asked to style-check or review code, and run it on your own changes before you call any code change done. The skill never edits code: it reports its findings and asks which to implement, and only those get implemented.
