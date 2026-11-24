//! The `wend` binary: the terminal frontend for `wend-core`.

use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    panic,
    path::{Path, PathBuf},
    str::FromStr,
    sync::mpsc,
    thread,
};

use anyhow::{Context, Result, anyhow};
use crossterm::{event, terminal};
use wend_core::{
    editor::Editor,
    frame::{Area, Frame},
    input::Input,
    output,
    syntax::LanguageTag,
};

use crate::screen::TerminalGuard;

mod args;
mod config;
mod draw;
mod input;
mod screen;
mod theme;

fn main() -> Result<()> {
    let file = match args::parse_args()? {
        args::Command::Edit(file) => file,
        args::Command::DefaultConfig => return print(config::DEFAULT),
        args::Command::Theme(name) => return print(config::theme::built_in(&name)?),
    };
    // Before the terminal is taken over, so a mistake in the file is shown where it can be read
    let config = config::load()?;
    let theme = config::theme::load(&config.theme)?;

    // Restore before the default hook prints, or the message is lost on the
    // alternate screen and the shell is left in raw mode
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        screen::restore_terminal();
        default_hook(panic_info);
    }));

    let (output_tx, output_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();

    let _guard = TerminalGuard::enter().context("failed to set up the terminal")?;

    let window = terminal::window_size().context("failed to read the terminal size")?;
    let area = Area::new(window.columns, window.rows);
    let mut editor = Editor::new(area, &config).context("failed to start the editor")?;

    let path = PathBuf::from_str(&file)
        .context("not a valid path")?
        .into_boxed_path();
    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .into_boxed_slice();
    input_tx
        .send(Input::File {
            path,
            bytes,
            language: Some(LanguageTag::new("rust")),
        })
        .context("the editor stopped before the file was sent")?;

    let editor_thread = thread::spawn(move || -> Result<()> {
        for input in input_rx {
            editor.apply(input).context("the editor failed")?;
            for output in editor.drain_outputs() {
                output_tx.send(output).context("the screen stopped")?;
            }
        }

        Ok(())
    });

    let terminal_tx = input_tx.clone();
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            let Some(input) = input::to_input(&event) else {
                continue;
            };
            // The editor is gone, nobody is left to read input
            if terminal_tx.send(input).is_err() {
                break;
            }
        }
    });

    // One frame's bytes, kept between frames so it only grows when a frame is bigger than
    // any before
    let mut buf = Vec::new();

    // The frame the terminal shows, so the next one only draws what differs from it
    let mut on_screen: Option<Frame> = None;

    for output in output_rx {
        match output {
            output::Output::Frame(frame) => {
                buf.clear();
                draw::render_frame(&frame, on_screen.as_ref(), &theme, &mut buf)
                    .context("failed to draw the screen")?;
                let mut out = io::stdout().lock();
                out.write_all(&buf)
                    .and_then(|()| out.flush())
                    .context("failed to draw the screen")?;

                // The editor gets the frame before this one back, which is no longer on screen
                if let Some(replaced) = on_screen.replace(frame) {
                    input_tx
                        .send(Input::Drawn(replaced))
                        .context("the editor stopped")?;
                }
            }
            output::Output::Quit => break,
            output::Output::Write { path, rope } => write_file(&path, rope.chunks())
                .with_context(|| format!("failed to save {}", path.display()))?,
            output::Output::Read { path } => {
                let bytes = fs::read(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?
                    .into_boxed_slice();
                input_tx
                    .send(Input::File {
                        path,
                        bytes,
                        language: Some(LanguageTag::new("rust")),
                    })
                    .context("the editor stopped")?;
            }
            output::Output::Language(tag) => input_tx
                .send(Input::Language {
                    tag,
                    wasm: Box::from(include_bytes!("../../../tree-sitter-rust.wasm").as_slice()),
                    highlights: Box::from(include_bytes!("../../../highlights.scm").as_slice()),
                })
                .context("the editor stopped")?,
        }
    }

    // The loop above also ends when the editor thread fails and drops its sender, and then
    // its error is the reason. After a quit the thread is still waiting for input.
    if editor_thread.is_finished() {
        editor_thread
            .join()
            .map_err(|_| anyhow!("the editor thread panicked"))??;
    }

    Ok(())
}

/// Writes `text` to stdout, for a flag that prints a file to copy.
fn print(text: &str) -> Result<()> {
    io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .context("failed to write to stdout")
}

/// Writes the text chunk by chunk, so it is never copied into one string first.
fn write_file<'a>(path: &Path, chunks: impl Iterator<Item = &'a str>) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for chunk in chunks {
        file.write_all(chunk.as_bytes())?;
    }
    file.flush()
}
