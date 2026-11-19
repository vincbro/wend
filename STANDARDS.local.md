# Project standards

What is specific to wend. The shared rules are in `code-standards/`. Where this file disagrees with them, this file wins.

## Commands

- **Gate** (must pass before any change is done): `just check`, which runs fmt, clippy, docs, tests and dependencies.
- **Full gate**: `just check-full`, the same with every property test run for 40000 cases. It takes a few minutes, so run it after touching a stateful type. The editor's fuzz test runs a quarter of the cases, as each one lays out whole terminals.
- **Commit-message check**: `just commit-msg <file>`. The commit-msg hook (`just hooks`) and the `commits` CI job run it.
- **Benchmarks** (for P-MEASURE): `just bench` times an editing session over growing files, `just bench-latency` a key press to the written frame, and `just bench-e2e` key presses on the release binary.

## Hot paths

The code that runs per key press and per frame: `Editor::apply`, `Keymap::press`, `Wrap::rebuild`, `View::render` and the frontend's draw. `View::move_selection` and `View::delete` run per key press too.

## Known performance opportunities

Measured on 2026-11-11, not yet changed. Each one is a candidate for a change that follows P-MEASURE.

| Where | What | Numbers |
|---|---|---|
| `syntax.rs`, `Syntax::highlights` | Every frame runs the highlight query over the visible bytes, and its cost grows with the file, not the screen. With highlighting off, a redraw is about 0.17 ms at 100 KB and at 1 MB. | With it on: 0.52 ms at 100 KB and 2.76 ms at 1 MB. Cache the visible spans while the tree and the range are unchanged. |
| `Editor::apply`, `reparse` | Typing in a 1 MB file takes about 14 ms, nearly all of it the incremental parse and the highlight query. | Not yet split further. |
| `view/wrap.rs`, `Wrap::find_byte` | Scans the rows and slots, and one key press calls it several times (`ensure_cursor_visible`, `cursor_position`, `grapheme_start`, which backspace calls too). | Not measured on its own. Cache the head's slot after a rebuild. |
| `view/motion/word.rs`, `word_backward` | Walks the line before the head forward into a scratch buffer, so it costs the column and not one word. | Not measured. On a minified file it could be 100k graphemes per key press. |
| `Cargo.toml` | `panic = "abort"` and a profiling profile aren't set. | Undecided. |
