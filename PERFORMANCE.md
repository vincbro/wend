# wend performance

How wend keeps its hot paths fast. It sits next to [STYLE.md](STYLE.md) and works the same way: every rule has an ID (`P-…`), a reason and an *Enforced* line, and `review` means nothing automatic checks it.

The hot paths are the ones that run per key press and per frame: `Editor::apply`, `Keymap::press`, `Layout::rebuild`, `View::render` and the frontend's draw. Everything else (startup, loading a grammar, saving) can allocate and copy freely.

The ideas come from the [Rust Performance Book](https://nnethercote.github.io/perf-book/). What is here is the part that fits an editor, and the book stays the reference for the rest.

---

## Measure

### P-MEASURE: A speedup cites a benchmark

A change made for speed says what it measured. Run `just bench` before and after (see CONTRIBUTING, Benchmarks) and put the numbers in the commit body. Code that is harder to read because it is faster gets a comment with the measurement.

The same applies to anything the book suggests but wend has no evidence for: a different hasher, `SmallVec`, a different allocator, `target-cpu=native`. The default stays until a benchmark says otherwise.

*Why:* an unmeasured optimization is complexity with no proof. The book's own examples swing both ways: a faster hasher gave up to 6% in one place and 1 to 4% slowdowns in another.

*Enforced:* review.

---

## Allocation

### P-BOXED: Data that doesn't grow is a boxed slice

The default for owned data is `Box<[T]>`, `Box<str>`, `Box<Path>` or `Arc<str>`. It becomes a `Vec<T>`, `String` or `PathBuf` only when it has to grow.

*Why:* a `Vec` is three words and a boxed slice is two, so 24 bytes become 16 in every struct and enum that holds one. A boxed slice has no spare capacity that is never used, and it locks the API down: there is no `push` to misuse and the length can't change by accident.

Not covered by the rule: a collection that is reused on purpose to keep its capacity (the layout's slots, a frame's cells, `open_highlights`, a `FramePool`). Those are `Vec`s because growing and clearing them is their job.

Build a boxed slice directly, with `collect::<Box<[_]>>()` or `Box::from`. `Vec::into_boxed_slice` may reallocate to drop the extra capacity.

*Enforced:* review.

### P-NO-ALLOC-HOT: Hot paths allocate nothing after startup

A key press and a frame allocate no memory once the editor has started. `FramePool` and the reused scratch space in `View` exist for this.

*Why:* an allocation is a lock, a search for a free block and later a free, and it scatters the data the next frame reads. The book measured about 1% from removing ten allocations per million instructions in a compiler, and wend does the same work on every key.

*Enforced:* review and the benchmarks.

### P-BORROW: A hot function that returns existing data returns a borrow

If the data already lives somewhere, return `&[T]` or `&str`, and not a copy. A slice gives the caller more control than an iterator: it can index it, check its length, split it, and call `iter()` itself. Return an iterator only when the data is not contiguous or is computed as it goes.

When the result mixes stored and freshly made values, so no single slice holds it, return a small iterator enum over both, not a `Vec`. Keep the borrow short: if the caller needs `&mut self` while holding it, split what the callee touches into fields it can borrow separately (destructure `self`), and don't clone the data to get out of the conflict.

*Why:* nothing is allocated and nothing is copied, and the type says the caller doesn't own the data.

*Enforced:* review.

### P-OUT-BUFFER: A hot function that builds a collection fills a buffer it is given

When the result is new data and not something that already exists (otherwise see P-BORROW), instead of returning a new `Vec` or `String`, take `&mut Vec<T>` (or `&mut String`) from the caller. The function clears the buffer first and then writes its result into it. The caller owns the allocation and reuses it.

```rust
/// Replaces the contents of `rows` with the rows the view shows.
fn visible_rows(&self, rows: &mut Vec<Row>) {
    rows.clear();
    // ...
}
```

The function always clears, so no caller has to remember to and a stale result can never be mixed in. Clearing sets the length to 0 and keeps the capacity, which for the `Copy` types wend uses costs nothing. The doc comment says "Replaces the contents of" so the caller knows what they lose. A function that accumulates instead is a different function and says so in its name (`extend_…`).

*Why:* the caller can keep one buffer for the life of the program, and responsibility is the same everywhere. This differs from std's `read_line`, which appends, so the doc comment matters.

*Enforced:* review.

### P-REUSE: Reuse a collection instead of building a new one

Keep scratch space in the struct that calls repeatedly and `clear()` it, as `View::open_highlights` does. Use `a.clone_from(&b)` instead of `a = b.clone()`, which keeps `a`'s allocation. Pass `&mut Vec` into a callee instead of returning a `Vec` that is then appended.

*Enforced:* review.

### P-CAPACITY: Give a collection its size when it is known

Use `with_capacity` or `reserve` when the length is known. A `Vec` grows 0, 4, 8, 16 and so on, copying every element at each step.

*Enforced:* review.

---

## Types and sizes

### P-SIZES: Keep hot types small

Types that are copied on every key or frame stay under 128 bytes; above that the compiler calls `memcpy`. Box a large enum variant that is rarely used. Prefer a smaller integer for an index when the range allows it, and convert to `usize` where it is used.

*Enforced:* lint, partly (`enum-variant-size-threshold = 64`, `pass-by-value-size-limit = 64` in `clippy.toml`). Review checks the rest. Measure with `RUSTFLAGS=-Zprint-type-sizes cargo +nightly build --release`.

---

## Iterators and loops

### P-ITERATORS: Iterate, don't collect and don't index

- Return `impl Iterator` instead of a `Vec` when the caller just loops.
- Don't `collect` to iterate again. `extend` an existing collection instead of collecting and appending.
- Implement `size_hint` (and `ExactSizeIterator`) on a custom iterator, so `collect` and `extend` can size their allocation.
- Loop over an iterator or a slice, not over indices: the compiler can drop the per-element bounds check.
- Use `filter_map` instead of `filter` then `map`, and `chunks_exact` instead of `chunks` when the size divides evenly.

*Enforced:* review.

---

## Output

### P-OUTPUT: The terminal gets one write per frame

The frontend builds a whole frame in a `Vec<u8>` it keeps between frames, clears it before each frame, and hands it to a locked `stdout` with a single `write_all` and one `flush`, checking the errors. The lock is only held for that write, so the panic hook can still restore the terminal from another thread. `Stdout` on its own is line-buffered with a small buffer, so a frame of many escape sequences becomes many system calls. The `Vec` grows to the largest frame seen and then stays that size.

Inside the frame, a color is written only when it differs from the cell before, and the cursor is moved only at the start of a row and after a cell that isn't plain single-width ASCII.

*Enforced:* review.

---

## Build

### P-BUILD: The shipped build is the release profile plus LTO

`just dist` builds with fat LTO, one codegen unit and stripped symbols. `#[inline]` and its relatives stay banned (S-INLINE), because LTO makes the call across crates.

To profile, build with `debug = "line-tables-only"` so the profiler shows source lines, and see the book's profiling chapter for the tools (on macOS: Instruments or `samply`).

*Enforced:* none needed.

---

## Known opportunities

Measured on 2026-11-11, not yet changed. Each one is a candidate for a change that follows P-MEASURE. `just bench`, `just bench-latency` and `just bench-e2e` give the numbers.

| Where | What | Numbers |
|---|---|---|
| `syntax.rs`, `Syntax::highlights` | Every frame runs the highlight query over the visible bytes, and its cost grows with the file, not the screen. With highlighting off, a redraw is about 0.17 ms at 100 KB and at 1 MB. | With it on: 0.52 ms at 100 KB and 2.76 ms at 1 MB. Cache the visible spans while the tree and the range are unchanged. |
| `wend-cli/src/draw.rs` | Every cell is redrawn on every frame. | Frame diffing is in TODO.md. |
| `Editor::apply`, `reparse` | Typing in a 1 MB file takes about 14 ms, nearly all of it the incremental parse and the highlight query. | Not yet split further. |
| `view/layout.rs`, `Layout::find_byte` | Scans the rows and slots, and one key press calls it several times (`ensure_cursor_visible`, `cursor_position`, `grapheme_start`). | Not measured on its own. Cache the head's slot after a rebuild. |
| `Cargo.toml` | `panic = "abort"` and a profiling profile aren't set. | Undecided. |
