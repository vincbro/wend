# 🗻 wend

A terminal text editor that does less, on purpose.

- Write your config once, use it on every machine you SSH into. Install nothing on the remote.
- No built-in terminal, no splits, no tabs. Your terminal already has those.
- An editor you don't have to think about.

## The rules

- **One buffer.** No splits. No tabs.
- **Centered, sized to your code.** The buffer is as wide as the language's max line width (rustfmt's for Rust, and so on). On a wide monitor your code sits where you're already looking, not glued to the left edge.
- **Waypoints, not tabs.** Mark a spot, bind it to a key: `leader w c` takes you to `Cargo.toml`. Waypoints belong to the project, not the path, so they work wherever the repo is checked out, worktrees included. They follow the code when it moves around them. Cross-project waypoints work too.
- **Remote editing over SSH.** Nothing to install on the other end.
- **Live files.** When something else changes a file (a formatter, git, an agent), you see it change in the buffer. See [Live files](#live-files).
- **Strong defaults.** Some choices can't be turned off. If they don't fit how you work, see [Alternatives](#alternatives). No hard feelings.

## Namespaces

Every action lives in a namespace, and namespaces compose:

| Keys             | Does                 |
| ---------------- | -------------------- |
| `leader w`       | waypoints            |
| `leader b`       | buffers              |
| `leader e`       | edit                 |
| `leader e w`     | edit waypoints       |
| `leader e b`     | edit buffers         |

Learn the pieces once, and the combinations are free. Same layers, same meaning, everywhere.

## Remote editing

wend runs on your machine and keeps a local copy of the remote project, synced both ways in the background.

- You type into the local copy. The network never slows you down.
- Your local tools run against it. rust-analyzer on a VM that's never heard of rust-analyzer? Sure.
- Ignored and large files stay on the remote unless you ask for them (`.env`, for example).

Yes, that's a full copy of the project on your disk. Worth it for zero setup.

## Live files

Your editor isn't the only thing that writes to your files. A formatter rewrites one on save. `git checkout` swaps it out. A code generator regenerates it. The remote sync pulls in a teammate's change. An agent edits it while you work on something else.

wend is built around that. A change on disk is just another event: the buffer follows it, and you watch it happen live, with no "file changed, reload?" prompt and no stale buffer. It doesn't matter who made the change. A formatter and an agent go through the same path.

## LLMs and AI

wend is built with LLMs, within limits. They're used for testing, bug hunting, code quality checks and refactoring. The code itself is mostly written by hand: it's quicker to write the function or struct you want than to explain it, and then let an agent fill in the code around it.

Where agents shine is parallel work. You work on one part while an agent works on another, then you switch. Live files make that workflow pleasant instead of painful: the agent edits, and you see it land in the buffer as it happens.

Planned: something like [99](https://github.com/ThePrimeagen/99). Select some code, hit a key, write a message, and it goes to the chat you already have open in Claude Code, opencode or whatever harness you use. Review what the agent wrote, highlight the part you want changed, say what to change, and keep going.

## How it works

The core never blocks. It takes in events (keys, file changes, sync updates, language server messages), updates its state and emits a snapshot of what the screen should look like. The frontend feeds it input and draws the latest snapshot. Anything slow (disk, network, language servers) runs elsewhere and reports back as just another event.

## Keys

What works today. Normal and select mode share a keymap, select only makes motions extend the selection.

| Key       | Does                                    |
| --------- | --------------------------------------- |
| `h` `l`   | one grapheme left, right                |
| `j` `k`   | one row down, up                        |
| `w` `b`   | select the next, previous word          |
| `H` `L`   | start, end of the line                  |
| `J` `K`   | to the last, first row on screen        |
| `gs` `ge` | start, end of the file                  |
| `i`       | insert mode, `Esc` to leave             |
| `o`       | open a line below and insert            |
| `d`       | delete the selection                    |
| `u`       | revert to the last save                 |
| `s`       | save                                    |
| `v`       | toggle select mode                      |
| `q`       | quit                                    |

## Alternatives

wend borrows a lot from [Helix](https://helix-editor.com), [Neovim](https://neovim.io) and [Zed](https://zed.dev). If wend isn't for you, one of those probably is.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

> **Status:** wend is in early development. Most of the above isn't built yet. Expect breakage.
