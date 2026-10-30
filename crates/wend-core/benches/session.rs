//! A headless editing session, timed over files of growing size.
//!
//! It drives `Editor` the way a terminal client does: a key and then a frame.
//! Each phase is its own group with one entry per file size, and reports the time of a
//! single action, the frame after it included.

use std::{
    hint,
    time::{Duration, Instant},
};

use criterion::{BenchmarkId, Criterion, Throughput};
use wend_core::{
    buffer::Buffer,
    editor::Editor,
    frame::{Area, Frame},
    view::{Direction, Motion},
};

/// File sizes in bytes.
const SIZES: [usize; 3] = [100_000, 1_000_000, 10_000_000];

/// A common terminal size. The editor caps its view at 100 columns.
const COLS: u16 = 120;
const ROWS: u16 = 40;

/// What the delete phase types first, and then deletes again.
const TYPED: &str = "let typed = \"some text\";";

/// Lines that look like source code, cycled to fill a file.
const LINES: [&str; 9] = [
    "fn layout(buffer: &Buffer, area: Area) -> Layout {",
    "    let width = area.width().min(100); // keep lines readable",
    "    for (row, slot) in slots.iter().enumerate() {",
    "        // 漢字とかなが混ざったコメント",
    "        let name = \"café naïve résumé 🦀\";",
    "\tif slot.is_blank() {",
    "    }",
    "",
    "    let message = \"a line long enough to wrap on any terminal, so scrolling and \
     moving through it go through the wrapped row paths of the layout, which a file of \
     short lines never touches, and it keeps going a little longer to wrap twice\";",
];

/// The same text for the same `len` on every run, cut on a char boundary.
fn text(len: usize) -> String {
    let mut text = String::with_capacity(len + 256);

    for line in LINES.iter().cycle() {
        if text.len() >= len {
            break;
        }

        text.push_str(line);
        text.push('\n');
    }

    let mut end = len.min(text.len());

    while !text.is_char_boundary(end) {
        end -= 1;
    }

    text.truncate(end);
    text
}

/// Opens the editor and returns its initial frame as well.
///
/// Returning the frame avoids allocating a second frame in `phase()`.
fn opened(text: &str) -> (Editor, Frame) {
    let mut editor = Editor::new(Area::new(COLS, ROWS));
    editor.view_mut().open_buffer(Buffer::from(text));

    let mut frame = editor.frame();
    draw(&mut editor, &mut frame);

    (editor, frame)
}

/// Updates the existing frame, reusing it between draws.
fn draw(editor: &mut Editor, frame: &mut Frame) {
    editor.update_frame(frame);
    hint::black_box(());
}

fn press(editor: &mut Editor, frame: &mut Frame, motion: Motion, times: usize) {
    for _ in 0..times {
        editor.view_mut().move_selection(motion, false);
        draw(editor, frame);
    }
}

fn type_text(editor: &mut Editor, frame: &mut Frame) {
    for c in TYPED.chars() {
        editor.view_mut().insert(c);
        draw(editor, frame);
    }
}

/// Times `iterations` actions, opening a fresh editor every `run` actions.
///
/// One action from a fresh editor would time the same press over and over. A run also
/// crosses lines, wrapped rows and scrolls, and the average covers all of them. Setup and
/// destruction are outside the timer.
fn time_actions(
    source: &str,
    iterations: u64,
    run: u64,
    setup: fn(&mut Editor, &mut Frame),
    action: &impl Fn(&mut Editor, &mut Frame),
) -> Duration {
    let mut elapsed = Duration::ZERO;
    let mut left = iterations;

    while left > 0 {
        let actions = left.min(run);
        let (mut editor, mut frame) = opened(source);
        setup(&mut editor, &mut frame);

        let start = Instant::now();
        for _ in 0..actions {
            action(&mut editor, &mut frame);
        }
        elapsed += start.elapsed();

        hint::black_box((editor, frame));
        left -= actions;
    }

    elapsed
}

fn phase(
    criterion: &mut Criterion,
    name: &str,
    run: u64,
    setup: fn(&mut Editor, &mut Frame),
    action: impl Fn(&mut Editor, &mut Frame),
) {
    let mut group = criterion.benchmark_group(name);

    for size in SIZES {
        let source = text(size);

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &source,
            |bencher, source| {
                bencher.iter_custom(|iterations| {
                    time_actions(source, iterations, run, setup, &action)
                });
            },
        );
    }

    group.finish();
}

/// Builds the buffer and draws the first frame. File reading is excluded.
fn open(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("open");

    for size in SIZES {
        let source = text(size);

        group.throughput(Throughput::Bytes(source.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &source,
            |bencher, source| {
                bencher.iter_with_large_drop(|| opened(source));
            },
        );
    }

    group.finish();
}

/// Redraws without moving, the part of every motion phase that isn't the motion.
fn draw_only(criterion: &mut Criterion) {
    phase(criterion, "draw", 100, nothing, draw);
}

fn motions(criterion: &mut Criterion) {
    both_directions(criterion, "horizontal", Motion::Horizontal, 100);
    both_directions(criterion, "vertical", Motion::Vertical, 100);
    both_directions(criterion, "word", Motion::Word, 100);
    both_directions(criterion, "line", Motion::Line, 100);
    both_directions(criterion, "page", Motion::Page, 20);
}

/// Times a motion each way. Forward starts at the top of the file and backward near the end,
/// so both have `run` presses of text to move through.
fn both_directions(
    criterion: &mut Criterion,
    name: &str,
    motion: fn(Direction) -> Motion,
    run: u64,
) {
    phase(
        criterion,
        &format!("{name}_forward"),
        run,
        nothing,
        |editor, frame| press(editor, frame, motion(Direction::Forward), 1),
    );
    phase(
        criterion,
        &format!("{name}_backward"),
        run,
        near_end,
        |editor, frame| press(editor, frame, motion(Direction::Backward), 1),
    );
}

/// A second press wouldn't move, so every run is a single press.
fn file_edges(criterion: &mut Criterion) {
    phase(criterion, "file_forward", 1, nothing, |editor, frame| {
        press(editor, frame, Motion::File(Direction::Forward), 1);
    });
    phase(criterion, "file_backward", 1, near_end, |editor, frame| {
        press(editor, frame, Motion::File(Direction::Backward), 1);
    });
}

/// Ends up a few pages before the end of the file with a full screen of text. Right after
/// the jump most rows are empty, which would make every backward draw look cheap.
fn near_end(editor: &mut Editor, frame: &mut Frame) {
    press(editor, frame, Motion::File(Direction::Forward), 1);
    press(editor, frame, Motion::Page(Direction::Backward), 3);
}

fn type_and_delete(criterion: &mut Criterion) {
    let run = TYPED.len() as u64;

    phase(criterion, "type", run, nothing, |editor, frame| {
        editor.view_mut().insert('x');
        draw(editor, frame);
    });

    phase(criterion, "delete", run, type_text, |editor, frame| {
        editor.view_mut().delete();
        draw(editor, frame);
    });
}

/// The edit that makes a revert do something is setup, so every run is a single revert.
fn revert(criterion: &mut Criterion) {
    phase(
        criterion,
        "revert",
        1,
        |editor, _frame| editor.view_mut().insert('x'),
        |editor, frame| {
            editor.view_mut().revert();
            draw(editor, frame);
        },
    );
}

fn nothing(_: &mut Editor, _: &mut Frame) {}

criterion::criterion_group!(
    benches,
    open,
    draw_only,
    motions,
    file_edges,
    type_and_delete,
    revert
);

criterion::criterion_main!(benches);
