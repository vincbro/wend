# 🗻 wend

A terminal text editor written in Rust. It's very early, and most of what's described here doesn't exist yet.

I want two things from an editor. I want to write my config once and use it on any machine I SSH into without installing anything on that machine. And I want the editor to do less. I don't think an editor needs a built-in terminal, splits or tabs when my terminal already has them. The goal is an editor I don't have to think about.


## Namespaces

Actions are bound to namespaces, `waypoints` occupie the `leader + w` namespace, `buffers` occupie the `leader + b`, `edits` occupie the `e` key, with that we can chain moves like editing the waypoints with `leader + e + w` and buffers would be `leader + e + b`

The vision here is to be able to build a strong consistant mental model of what things will do by layering constant setups.

## Planned features

- One buffer. No splits, no tabs.
- The buffer is centered and as wide as the language's max line width (rustfmt's for Rust, and so on), so on a wide monitor the code sits where you're already looking.
- Waypoints instead of tabs. Mark a spot in a file and bind it to a key, like `leader w c` for `Cargo.toml`. Waypoints are saved per project instead of per path, so they work wherever the repo is checked out, including other worktrees. They should still land in the right place after the code around them changes. You can also make waypoints that work across projects.
- Remote editing over SSH with nothing installed on the remote.
- Strong defaults. Some choices can't be turned off. If they don't fit how you work, one of the editors at the bottom probably will.

## Remote editing

wend runs on your machine and keeps a local copy of the remote project, synced both ways in the background. You type into the local copy, so the network never slows down editing, and your own tools run against it. That's how you get rust-analyzer on a VM that doesn't have it installed. Ignored and large files stay on the remote unless you ask for them (`.env`, for example).

Yes, this means keeping a full copy of the project locally. I think that's worth it.

## Design

This part is experimental and will probably change.

The editor core never blocks. It takes in events (keys, file changes, sync updates, language server messages), updates its state and produces a snapshot of what the screen should look like. It doesn't know what a terminal is. The frontend sends input in and draws the latest snapshot. Anything slow, like disk, network or language servers, runs somewhere else and reports back as another event.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Alternatives

wend borrows a lot from [Helix](https://helix-editor.com), [Neovim](https://neovim.io) and [Zed](https://zed.dev). If wend isn't for you, try one of those.
