//! A headless editing session, timed over files of growing size.
//!
//! It drives `Editor` the way a terminal client does: a key and then a frame.
//! Each phase is its own group with one entry per file size.

use std::hint;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use wend_core::{
    area::Area,
    buffer::Buffer,
    editor::Editor,
    frame::Frame,
    view::{Direction, Motion},
};

/// File sizes in bytes.
const SIZES: [usize; 4] = [10_000, 100_000, 1_000_000, 10_000_000];

/// A common terminal size. The editor caps its view at 100 columns.
const COLS: u16 = 120;
const ROWS: u16 = 40;

/// What the type phase types, and the delete phase deletes again.
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

/// Times `play` on a freshly opened editor.
///
/// Setup and destruction are excluded from the timed routine.
fn phase(
    criterion: &mut Criterion,
    name: &str,
    setup: fn(&mut Editor, &mut Frame),
    play: fn(&mut Editor, &mut Frame),
) {
    let mut group = criterion.benchmark_group(name);

    for size in SIZES {
        let source = text(size);

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &source,
            |bencher, source| {
                bencher.iter_batched(
                    || {
                        let (mut editor, mut frame) = opened(source);
                        setup(&mut editor, &mut frame);
                        (editor, frame)
                    },
                    |(mut editor, mut frame)| {
                        play(&mut editor, &mut frame);
                        hint::black_box((editor, frame))
                    },
                    BatchSize::LargeInput,
                );
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

fn page_down(criterion: &mut Criterion) {
    phase(criterion, "page_down", nothing, |editor, frame| {
        press(editor, frame, Motion::Page(Direction::Forward), 20);
    });
}

fn word(criterion: &mut Criterion) {
    phase(criterion, "word", nothing, |editor, frame| {
        press(editor, frame, Motion::Word(Direction::Forward), 100);
    });
}

fn jump_end(criterion: &mut Criterion) {
    phase(criterion, "jump_end", nothing, |editor, frame| {
        press(editor, frame, Motion::File(Direction::Forward), 1);
    });
}

fn type_and_delete(criterion: &mut Criterion) {
    phase(criterion, "type", nothing, type_text);

    phase(criterion, "delete", type_text, |editor, frame| {
        for _ in TYPED.chars() {
            editor.view_mut().delete();
            draw(editor, frame);
        }
    });
}

fn revert(criterion: &mut Criterion) {
    phase(
        criterion,
        "revert",
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
    page_down,
    word,
    jump_end,
    type_and_delete,
    revert
);

criterion::criterion_main!(benches);
