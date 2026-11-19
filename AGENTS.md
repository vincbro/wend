# Agent notes for wend

Before writing or reviewing code here, read:

- [CONTRIBUTING.md](CONTRIBUTING.md): setup, `just` recipes, lints, formatting and commits.
- [README.md](README.md#how-it-works): the architecture. The core never blocks; disk, network and language servers run elsewhere and report back as events. The current code may not show this yet, so design against the README, not the code.
- [STANDARDS.local.md](STANDARDS.local.md): wend's gate, hot paths, known performance problems and the overrides of the shared rules.

`just check` is the gate, and it must pass.

## Code standards

Rules live in `code-standards/` (a submodule). Read `code-standards/<language>/` before writing or reviewing code in that language, and `STANDARDS.local.md` for what is specific to this project.
Use the `review` skill to review a change and the `commit` skill to commit. Run `review` on your own changes before you call any code change done.
