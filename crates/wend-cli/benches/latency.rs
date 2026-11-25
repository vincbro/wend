//! How fast a key reaches the terminal, and how often a frame can be redrawn.
//!
//! Both run the whole path of the frontend with no terminal attached: `Editor::apply`
//! (parsing included), the frame it sends out, `draw::render_frame` building the escape
//! sequences, and one write of them to `/dev/null`. Only the terminal itself is missing,
//! so the numbers are what wend adds to a key press.
//!
//! - `input_delay` times one key press from the key to the written frame, per file size.
//! - `fps` redraws the same frame over and over. Its throughput line is frames per second.
//!
//! Each runs twice, `full` drawing every cell of every frame and `diff` drawing only what
//! differs from the frame before, as the frontend does.

use std::{
    fs::{File, OpenOptions},
    hint,
    io::Write,
    mem,
    path::Path,
    time::{Duration, Instant},
};

use criterion::{BenchmarkGroup, BenchmarkId, Criterion, Throughput, measurement::WallTime};
use wend_core::{
    config::Config,
    editor::Editor,
    frame::{Area, Frame},
    input::{
        Input,
        key::{Key, KeyCode},
    },
    output::Output,
    syntax::LanguageTag,
};

// The frontend's own modules, so the bench times the code that ships
// These files carry unit tests that the bench, with no test harness, never runs
#[allow(
    dead_code,
    reason = "test helpers of the file are compiled but not run here"
)]
#[path = "../src/draw.rs"]
mod draw;
#[allow(
    dead_code,
    reason = "test helpers of the file are compiled but not run here"
)]
#[path = "../src/theme.rs"]
mod theme;

/// File sizes in bytes.
const SIZES: [usize; 2] = [100_000, 1_000_000];

/// How a frame is drawn: every cell of it, or only what differs from the frame on screen.
const MODES: [(&str, bool); 2] = [("full", false), ("diff", true)];

/// A common terminal size.
const COLS: u16 = 120;
const ROWS: u16 = 40;

const RUST_WASM: &[u8] = include_bytes!("../../../tree-sitter-rust.wasm");
const RUST_HIGHLIGHTS: &[u8] = include_bytes!("../../../highlights.scm");

/// Lines that look like source code, cycled to fill a file. The same as the session bench.
const LINES: [&str; 9] = [
    "fn wrap(buffer: &Buffer, area: Area) -> Wrap {",
    "    let width = area.width().min(100); // keep lines readable",
    "    for (row, slot) in slots.iter().enumerate() {",
    "        // 漢字とかなが混ざったコメント",
    "        let name = \"café naïve résumé 🦀\";",
    "\tif slot.is_blank() {",
    "    }",
    "",
    "    let message = \"a line long enough to wrap on any terminal, so scrolling and \
     moving through it go through the wrapped row paths of the wrap, which a file of \
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

/// An editor and the frontend's half of the loop around it.
struct Session {
    editor: Editor,
    /// The frame `redraw` draws into.
    frame: Frame,
    /// The frame on screen, kept to draw only what differs from it. None in the `full` mode,
    /// and before the first frame.
    screen: Option<Frame>,
    diff: bool,
    /// Built once, like the frontend's, so a frame doesn't time reading a theme file.
    theme: theme::Theme,
    /// One frame's bytes, reused like the frontend's.
    bytes: Vec<u8>,
    sink: File,
    /// Outputs the editor sent that are not handled yet.
    outputs: Vec<Output>,
}

impl Session {
    /// Opens `text` as a Rust file, with its grammar loaded and the first frame drawn.
    fn open(text: &str, diff: bool) -> Self {
        let area = Area::new(COLS, ROWS);
        let mut editor = Editor::new(area, &Config::default())
            .expect("the wasm engine starts on every platform");
        let frame = editor.frame();
        let mut session = Self {
            editor,
            frame,
            screen: None,
            diff,
            theme: theme::Theme::built_in("default").expect("default is built in"),
            bytes: Vec::new(),
            sink: OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .expect("/dev/null exists on the platforms the benches run on"),
            outputs: Vec::new(),
        };
        session.deliver(Input::File {
            path: Path::new("bench.rs").into(),
            bytes: text.as_bytes().into(),
            language: Some(LanguageTag::new("rust")),
        });
        session
    }

    /// Hands `input` to the editor and does what the frontend does with everything it sends
    /// back: draw the frames and return them, load the grammar it asks for.
    fn deliver(&mut self, input: Input) {
        self.editor.apply(input);
        self.outputs.extend(self.editor.drain_outputs());
        while let Some(output) = self.outputs.pop() {
            match output {
                Output::Frame(frame) => self.show(frame),
                Output::Language(tag) => self.editor.apply(Input::Language {
                    tag,
                    wasm: Box::from(RUST_WASM),
                    highlights: Box::from(RUST_HIGHLIGHTS),
                }),
                Output::Write { .. } | Output::Read { .. } | Output::Quit => {}
            }
            self.outputs.extend(self.editor.drain_outputs());
        }
    }

    /// Draws `frame` and gives the editor back the frame that was on screen before it.
    fn show(&mut self, frame: Frame) {
        write(
            &frame,
            self.screen.as_ref(),
            &self.theme,
            &mut self.bytes,
            &mut self.sink,
        );
        let returned = if self.diff {
            self.screen.replace(frame)
        } else {
            Some(frame)
        };
        if let Some(returned) = returned {
            self.editor.apply(Input::Drawn(returned));
        }
    }

    fn press(&mut self, c: char) {
        self.deliver(Input::Key(Key::new(KeyCode::Char(c))));
    }

    /// Draws the frame again with nothing changed.
    fn redraw(&mut self) {
        self.editor.update_frame(&mut self.frame);
        write(
            &self.frame,
            self.screen.as_ref(),
            &self.theme,
            &mut self.bytes,
            &mut self.sink,
        );
        if !self.diff {
            return;
        }
        match &mut self.screen {
            Some(screen) => mem::swap(screen, &mut self.frame),
            None => self.screen = Some(self.frame.clone()),
        }
    }
}

/// Turns `frame` into escape sequences in `bytes` and writes them out in one go, as the
/// frontend does. `screen` is the frame the terminal shows, if the draw uses it.
fn write(
    frame: &Frame,
    screen: Option<&Frame>,
    theme: &theme::Theme,
    bytes: &mut Vec<u8>,
    sink: &mut File,
) {
    bytes.clear();
    draw::render_frame(frame, screen, theme, bytes).expect("a Vec takes any write");
    sink.write_all(bytes).expect("writing to /dev/null works");
}

/// Times `iterations` key presses, opening a fresh session every `run` presses, so a run
/// crosses lines, wrapped rows and scrolls and the average covers them. Setup is outside
/// the timer.
fn time_keys(
    source: &str,
    diff: bool,
    iterations: u64,
    run: u64,
    setup: &str,
    key: char,
) -> Duration {
    let mut elapsed = Duration::ZERO;
    let mut left = iterations;

    while left > 0 {
        let presses = left.min(run);
        let mut session = Session::open(source, diff);
        setup.chars().for_each(|c| session.press(c));

        let start = Instant::now();
        for _ in 0..presses {
            session.press(key);
        }
        elapsed += start.elapsed();

        hint::black_box(session);
        left -= presses;
    }

    elapsed
}

/// Times one benchmark of `keys`, which is the presses per session, the keys typed first and
/// the key pressed.
fn bench_keys(
    group: &mut BenchmarkGroup<'_, WallTime>,
    id: BenchmarkId,
    source: &String,
    diff: bool,
    (run, setup, key): (u64, &str, char),
) {
    group.bench_with_input(id, source, |bencher, source| {
        bencher.iter_custom(|iterations| time_keys(source, diff, iterations, run, setup, key));
    });
}

/// One key pressed over and over, after `setup` keys. `run` presses share a session.
fn input_delay(criterion: &mut Criterion) {
    // (name, keys typed first, key timed, presses per session)
    let phases = [
        ("type", "i", 'x', 25),
        ("down", "", 'j', 100),
        ("page_down", "", 'J', 20),
    ];

    for (name, setup, key, run) in phases {
        let mut group = criterion.benchmark_group(format!("input_delay/{name}"));
        for size in SIZES {
            let source = text(size);
            for (mode, diff) in MODES {
                let id = BenchmarkId::new(mode, size);
                bench_keys(&mut group, id, &source, diff, (run, setup, key));
            }
        }
        group.finish();
    }
}

/// Redraws a frame as fast as it can go. One iteration is one frame, so the throughput
/// criterion prints is frames per second. Nothing changes between frames, so the wrap and
/// the parse are not redone: this is the ceiling the other numbers are measured against.
fn fps(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("fps");
    group.throughput(Throughput::Elements(1));

    for size in SIZES {
        for (mode, diff) in MODES {
            let mut session = Session::open(&text(size), diff);
            group.bench_function(BenchmarkId::new(mode, size), |bencher| {
                bencher.iter(|| session.redraw());
            });
        }
    }

    group.finish();
}

criterion::criterion_group!(benches, input_delay, fps);
criterion::criterion_main!(benches);
