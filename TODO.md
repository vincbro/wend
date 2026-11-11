# TODO

In order. Things that only touch the core come first, and anything that needs downloads or
outside processes (grammars, LSP) comes last. Each step says why it sits where it does.

- [ ] **0. Debts and file splits.** Cheap now, and they touch the same files as everything below.
  - [x] Break the `buffer`↔`syntax` cycle: `Loader::parse` takes `&Rope` and `&mut Syntax`, not `&mut Buffer`.
  - [x] Break the `input`↔`editor` cycle: `Mode` moves to `input`.
  - [x] Move `Editor::run` and its channels to the frontend, and report errors from the editor thread (they are dropped silently today).
  - [x] Give `view` its own `Error` wrapping `buffer::Error`, so `View::update` and `View::save` stop returning another module's error (S-ERRORS), and `Editor::show_file` returns `editor::Result`.
  - [x] Drop the unused `dirs` workspace dependency and fix the `anyhow` ban wrapper in `deny.toml`.
  - [x] Remove the never-produced `buffer::Error::Io`.
  - [x] Split the big files into submodules (`main.rs`, `editor.rs`, `view.rs`, `view/layout.rs`, `view/motion.rs`, `syntax.rs`, `editor/keymap.rs`).
- [x] **0b. Performance pass.** The known opportunities in `PERFORMANCE.md`, each as its own measured change (P-MEASURE): buffer the frame output, `Keymap::press` returning a borrowed slice, `Layout::last_row_of_line` without a new `Layout`, boxed slices for fixed data, and a smaller `Slot`.
- [ ] **0c. Number conversions.** Use the `az` crate (`saturating_cast`, `checked_cast`) in place of `frame::terminal_coordinate` and the scattered `try_from` calls, with a thin `exact` that panics in debug builds and saturates in release, since `az` has no such cast (`az::Cast` wraps in release). Add the dependency in the same change that uses it, or `cargo machete` fails.
- [ ] **1. Styles.** `Cell { grapheme, style }` with a flat `Style` enum (`Plain`, `Text(Highlight)`, `Selection`, `Cursor`, `LineNumber`, `CurrentLineNumber`, `Border`, `Marker`) and a pure `Theme::colors(style)`. `covered` stays a field. Everything after this draws with styles.
- [ ] **2. Config.** A core `Config` type. The frontend reads TOML and sends it in as an event, because the core never touches the disk. Includes the keymap, so keys are rebindable.
- [ ] **3. Themes.** Theme files loaded through config, Mellow built in, and a way to create and select themes. After config because themes load from it, and after styles because they color `Style`.
- [ ] **4. Layout.** `View` gets the whole rect and centers its block (gutter plus text) by the buffer's max line width, 100 until the language registry exists. `Rect` gets `split_top`, `split_bottom`, `center`, `inset` and `anchored`. `Placement` rules, a fixed paint order, and the status bar as the second element.
- [ ] **5. Frame diffing.** Only draw cells that changed (`Style` is `PartialEq`).
- [ ] **6. Search overlay.** The first floating element, with `overlay: Option<..>` painted last and the cursor going to the topmost focused element.
- [ ] **7. Multi-file waypoints.** Jump between open files. No splits or tabs.
- [ ] **8. Language registry.** File extension to language, replacing the hardcoded `"rust"`. Per-language settings (max width and tabs) read from `rustfmt.toml` by the frontend. Replaces layout's default width of 100.
- [ ] **9. Grammar pipeline.** Any tree-sitter wasm grammar from config paths, and building grammars from source. The build runs in the frontend and reports back as an event.
- [ ] **10. LSP.** A background process, JSON-RPC as events, diagnostics in the gutter, and a completion overlay. Last because it needs the layout, the registry and the background plumbing.
