# 🗻 wend

A terminal text editor that does less, on purpose.

- Write your config once, use it on every machine you SSH into. Install nothing on the remote.
- No built-in terminal, no splits, no tabs. Your terminal already has those.
- An editor you don't have to think about.

## The rules

- **One buffer.** No splits and no tabs.
- **Centered, sized to your code.** The buffer is as wide as the language's max line width (rustfmt's for Rust, and so on). On a wide monitor your code sits where you're already looking, not glued to the left edge.
- **Waypoints, not tabs.** Mark a spot, bind it to a key: `leader w c` takes you to `Cargo.toml`. Waypoints belong to the project, not the path, so they work wherever the repo is checked out, worktrees included. They follow the code when it moves around them. Cross-project waypoints work too.
- **Remote editing over SSH.** Nothing to install on the other end.
- **Live files.** When something else changes a file (a formatter, git, an agent), you see it change in the buffer. See [Live files](#live-files).
- **Strong defaults.** Some choices can't be changed. If they don't fit how you work, [Alternatives](#alternatives) has editors that might. No hard feelings.

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
| `Esc`     | drop the selection, back to normal mode |
| `o`       | open a line below and insert            |
| `d`       | delete the selection                    |
| `x`       | select the screen row                   |
| `u`       | revert to the last save                 |
| `v`       | toggle select mode                      |
| `:`       | type a command, `Enter` runs it         |

Saving and quitting are commands and have no key by default: `:save` or `:w` (`:write`, and `:w path` to write to another file), `:quit` or `:q`.

`/` opens a prompt along the bottom that searches the text as you type: the match is selected, so `d` deletes it. `Enter` keeps it and `Esc` goes back to where you were. `n` and `p` then select the next and the previous match, wrapping around, and in select mode they extend the selection. A query with no capital ignores case.

`<leader>f` opens the search: type a query, and the files that match are listed under it. `Esc` moves from the query to the list, `j` and `k` move in it, `i` goes back to the query and `Enter` opens the file the cursor is on. `<leader>/` is the same search over the text in the files, and `Enter` opens the file on the line of the result. What finds them are the `files` and `text` commands of `[search]`, which need `rg` installed by default and match the query as plain text. Swap the second `rg` for `fzf --filter "$WEND_QUERY"` to match fuzzily.

## Config

The config file is `$XDG_CONFIG_HOME/wend/config.toml`, or `~/.config/wend/config.toml`. It holds every setting, with a comment on each, so the file is the documentation.

With no config file wend uses the defaults. `wend --default-config` prints them as a file, so `wend --default-config > ~/.config/wend/config.toml` gives you one to edit. The same file is in the repo as [`config/default.toml`](config/default.toml). A setting the file leaves out keeps its default.

Bindings in the file go over the defaults. Bind a combo to `[]` to remove it, so `h = []` removes the left key. Nothing is bound to save or quit by default, so add `s = "save"` and `q = "quit"` to `[keys.scratch.normal]` if you want them.

Keys come in tables per mode. `[keys.normal]`, `[keys.select]` and `[keys.insert]` hold the motions and edits every text box answers to, the editor and the command line alike. What is about the document and the editor (`:`, and whatever you bind to `save` or `quit`) is in `[keys.scratch.<mode>]`, and the command line has `[keys.command.<mode>]` for what only it does, such as `Enter`. A target's table goes over the shared one.

A command is the name of an action, such as `:save`, `:left` or `:open src/main.rs`.

## Themes

The built-in themes are the files in the repo's [`themes`](themes) folder, compiled into the binary: Default, Mellow, Catppuccin Mocha, Tokyo Night, Gruvbox Dark, Nord, Dracula, Rosé Pine, One Dark, Solarized Dark and Solarized Light. Pick one with `theme = "name"` in the config file; it is `default` until you do.

To make your own, print a built-in theme and save it as `<name>.toml` in the `themes` folder next to the config file: `wend --theme default > ~/.config/wend/themes/dusk.toml`. Every theme file explains itself, with a comment on every rule that says what it colors and an example. A theme file in that folder is loaded when it is the selected theme, and goes before a built-in theme of the same name.

To add a theme that ships with wend, run `just theme <name>` in a checkout. It copies `themes/default.toml` to `themes/<name>.toml`, and the next build lists it as a built-in theme, with no code to change.

A theme file says every rule. A rule sets a `foreground` and a `background` as `#rrggbb`, and `modifiers` from `bold`, `dim`, `italic`, `underline` and `strikethrough`. Layers stack (plain text, then the highlight, then the selection, then the cursor), and a layer only changes what it sets.

## Alternatives

wend borrows a lot from [Helix](https://helix-editor.com), [Neovim](https://neovim.io) and [Zed](https://zed.dev). If wend isn't for you, one of those probably is.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

> **Status:** wend is in early development. Most of the above isn't built yet, so expect rough edges.
