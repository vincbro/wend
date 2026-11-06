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

use anyhow::{Context, Result, bail};
use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event, KeyEvent, KeyEventKind, KeyModifiers},
    style::{self, Attribute},
    terminal::{self},
};
use wend_core::{
    editor::Editor,
    frame::{Area, Cell, CellPosition, Frame},
    input::{
        Input,
        key::{Key, KeyCode, Modifiers},
    },
    output,
    syntax::LanguageTag,
};

use crate::theme::MELLOW;

mod theme;

const HELP: &str = "usage: wend [file/to/open]";

struct Args {
    file: String,
}

fn parse_args() -> Result<Args> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(argument) = parser.next()? {
        match argument {
            Value(value) if file.is_none() => {
                file = Some(value.string()?);
            }
            Long("help") => {
                bail!(HELP);
            }
            _ => return Err(argument.unexpected().into()),
        }
    }

    let Some(file) = file else { bail!(HELP) };

    Ok(Args { file })
}

/// Puts the terminal in raw mode on the alternate screen, and restores it when dropped,
/// so an early `?` return can't leave the user's shell broken.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        // Created before anything else can fail, so a failure below still restores
        let guard = Self;
        io::stdout()
            .queue(terminal::EnterAlternateScreen)?
            .queue(cursor::Hide)?
            .flush()?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Best effort, it runs while we are already failing so errors are ignored. Safe to
/// call more than once.
fn restore_terminal() {
    let mut stdout = io::stdout();
    drop(
        stdout
            .queue(cursor::Show)
            .and_then(|stdout| stdout.queue(terminal::LeaveAlternateScreen))
            .and_then(Write::flush),
    );
    drop(terminal::disable_raw_mode());
}

fn main() -> Result<()> {
    let args = parse_args()?;

    // Restore before the default hook prints, or the message is lost on the
    // alternate screen and the shell is left in raw mode
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        default_hook(panic_info);
    }));

    let (output_tx, output_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();

    let _guard = TerminalGuard::enter().context("failed to set up the terminal")?;
    let mut stdout = io::stdout();

    let window = terminal::window_size().context("failed to read the terminal size")?;
    let area = Area::new(window.columns, window.rows);
    let editor = Editor::new(area).context("failed to start the editor")?;

    let path = PathBuf::from_str(&args.file).context("not a valid path")?;
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

    thread::spawn(move || editor.run(&input_rx, &output_tx));

    let terminal_tx = input_tx.clone();
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            let Some(input) = to_input(&event) else {
                continue;
            };
            // The editor is gone, nobody is left to read input
            if terminal_tx.send(input).is_err() {
                break;
            }
        }
    });

    for output in output_rx {
        match output {
            output::Output::Frame(frame) => {
                render_frame(&frame, &mut stdout).context("failed to draw the screen")?;
                input_tx
                    .send(Input::Drawn(frame))
                    .context("the editor stopped")?;
            }
            output::Output::Quit => break,
            output::Output::Write { path, rope } => write_file(&path, rope.chunks())
                .with_context(|| format!("failed to save {}", path.display()))?,
            output::Output::Language(tag) => input_tx
                .send(Input::Language {
                    tag,
                    wasm: Box::from(include_bytes!("../../../tree-sitter-rust.wasm").as_slice()),
                    highlights: Box::from(include_bytes!("../../../highlights.scm").as_slice()),
                })
                .context("the editor stopped")?,
        }
    }

    Ok(())
}

/// Turns a terminal event into an input for the editor, none for events it has no use for.
fn to_input(event: &Event) -> Option<Input> {
    match *event {
        Event::Resize(cols, rows) => Some(Input::Resize(Area::new(cols, rows))),
        Event::Key(key_event) if key_event.kind != KeyEventKind::Release => {
            to_key(key_event).map(Input::Key)
        }
        _ => None,
    }
}

fn to_key(key_event: KeyEvent) -> Option<Key> {
    let code = match key_event.code {
        event::KeyCode::Backspace => KeyCode::Backspace,
        event::KeyCode::Enter => KeyCode::Enter,
        event::KeyCode::Esc => KeyCode::Esc,
        event::KeyCode::Tab | event::KeyCode::BackTab => KeyCode::Tab,
        event::KeyCode::Char(c) => KeyCode::Char(c),
        _ => return None,
    };
    let is_shifted = key_event.modifiers.contains(KeyModifiers::SHIFT)
        || key_event.code == event::KeyCode::BackTab;
    let modifiers = Modifiers {
        alt: key_event.modifiers.contains(KeyModifiers::ALT),
        control: key_event.modifiers.contains(KeyModifiers::CONTROL),
        // A character is already the shifted one, and the keymap binds it that way
        shift: is_shifted && !matches!(code, KeyCode::Char(_)),
    };
    Some(Key { code, modifiers })
}

/// Writes the text chunk by chunk, so it is never copied into one string first.
fn write_file<'a>(path: &Path, chunks: impl Iterator<Item = &'a str>) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for chunk in chunks {
        file.write_all(chunk.as_bytes())?;
    }
    file.flush()
}

fn render_frame(frame: &Frame, writer: &mut impl Write) -> Result<()> {
    // Terminals that support it hold the frame back until it is whole, so a half-drawn
    // one never shows. The others ignore it.
    writer.queue(terminal::BeginSynchronizedUpdate)?;
    writer.queue(cursor::MoveTo(0, 0))?;
    for (pos, cell) in frame.iter() {
        if cell.covered {
            continue;
        }
        queue_cell(cell, pos, writer)?;
    }
    writer.queue(terminal::EndSynchronizedUpdate)?;
    writer.flush()?;
    Ok(())
}

fn queue_cell(cell: &Cell, pos: CellPosition, writer: &mut impl Write) -> Result<()> {
    let (background, foreground) = if cell.cursor {
        (MELLOW.cursor_background, MELLOW.cursor_foreground)
    } else if cell.selected {
        (MELLOW.selection_background, MELLOW.selection_foreground)
    } else {
        (MELLOW.background, MELLOW.foreground_of(cell.highlight))
    };
    writer
        .queue(style::SetBackgroundColor(background))?
        .queue(style::SetForegroundColor(foreground))?
        .queue(cursor::MoveTo(pos.col, pos.row))?
        .queue(style::Print(cell.grapheme.as_str_sanitize()))?
        .queue(style::SetAttribute(Attribute::Reset))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(code: event::KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn shifted_characters_drop_shift() {
        let key = to_key(key_event(event::KeyCode::Char('H'), KeyModifiers::SHIFT)).unwrap();
        assert_eq!(
            key,
            Key::new(KeyCode::Char('H')),
            "the keymap binds H, not shift and H"
        );
    }

    #[test]
    fn back_tab_is_shift_tab() {
        let key = to_key(key_event(event::KeyCode::BackTab, KeyModifiers::SHIFT)).unwrap();
        assert_eq!(key.code, KeyCode::Tab);
        assert!(
            key.modifiers.shift,
            "shift stays on keys that aren't characters"
        );
    }

    #[test]
    fn control_stays_on_characters() {
        let key = to_key(key_event(event::KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();
        assert!(key.modifiers.control);
    }

    #[test]
    fn resize_becomes_an_input() {
        assert!(matches!(
            to_input(&Event::Resize(80, 24)),
            Some(Input::Resize(Area { cols: 80, rows: 24 }))
        ));
    }

    #[test]
    fn key_releases_are_dropped() {
        let mut release = key_event(event::KeyCode::Char('j'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(
            to_input(&Event::Key(release)).is_none(),
            "or j would move twice"
        );
    }
}
