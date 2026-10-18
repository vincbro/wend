# wend

A terminal text editor built for speed of movement, not for managing windows.

> **Status:** early days. Nothing usable yet, this crate currently reserves the name while the editor takes shape.

## One buffer, no splits

You always work in a single view. Instead of arranging panes, you jump between files so fast that a second pane is never needed.

## Waypoints instead of tabs and splits

You open a file with the finder. When you're working across several files, you drop a waypoint in each one and bind it to a key: `Cargo.toml` might live on `leader w c`, for cargo. One keystroke teleports you back to exactly that spot.

- **They survive shutdown.** Come back a week later, press your waypoint keys, and you're right where you left off.
- **They belong to the project, not the machine or the path.** A project is identified by its repo, so the same waypoints work wherever that repo is checked out.
- **They work across git worktrees.** A waypoint resolves inside whichever worktree you're in, and can jump to the same spot in another one.
- **Cross-project waypoints** for the files you keep coming back to, whatever you're working on.
- **Anchors that hold up.** A waypoint remembers more than a line number, so it still lands in the right place after the code around it has changed.

## Any machine, as if it were local

Over SSH, wend runs on your machine and only the files live on the remote. Typing never waits on the network, and the remote doesn't need anything installed: you can SSH into a box that has never heard of rust-analyzer and still have full tooling, because it runs locally against a synced copy of the project. Your waypoints stay on your machine too, so you can jump into any computer, work, and jump out.

## Built for performance, all the way down

- Fast file reads (mmap where it fits)
- A rope-based buffer
- Rendering that only redraws what changed
- Full keyboard input through the kitty keyboard protocol, so single-key bindings can be dense and mnemonic

## The point

Work fast, move between files without thinking, and never be locked into a window layout or a single machine.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Inspirations & alternatives

wend stands on the shoulders of these editors. If wend isn't for you, they're well worth a look:

- [Helix](https://helix-editor.com)
- [Neovim](https://neovim.io)
- [Zed](https://zed.dev)
