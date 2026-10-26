//! The `wend` binary: the terminal frontend for `wend-core`.

use std::{
    io::{self, Write},
    panic,
};

use anyhow::{Result, bail};
use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event},
    style::{self, Attribute},
    terminal::{self},
};
use wend_core::{
    area::Area,
    editor::{Editor, Mode},
    frame::Frame,
};

const HELP: &str = "Usage: wend [file/to/open]";

struct Args {
    file: String,
}

fn parse_args() -> Result<Args> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Value(val) if file.is_none() => {
                file = Some(val.string()?);
            }
            Long("help") => {
                bail!(HELP);
            }
            _ => return Err(arg.unexpected().into()),
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
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let _guard = TerminalGuard::enter()?;
    let mut stdout = io::stdout();

    let window = terminal::window_size()?;
    let area = Area::new(window.columns, window.rows);
    let mut editor = Editor::new(area);
    editor.open_file(args.file.as_str())?;
    let frame = editor.frame();
    render_frame(&frame, &mut stdout)?;

    loop {
        let event = event::read()?;
        if let Event::Resize(cols, rows) = event {
            editor.resize(Area::new(cols, rows));
        } else if let Event::Key(key) = event {
            if editor.is_normal() {
                if key.code.is_char('h') {
                    editor.view.left();
                } else if key.code.is_char('j') {
                    editor.view.down();
                } else if key.code.is_char('k') {
                    editor.view.up();
                } else if key.code.is_char('l') {
                    editor.view.right();
                } else if key.code.is_char('i') {
                    editor.switch_mode(Mode::Insert);
                } else if key.code.is_char('o') {
                    editor.view.new_line();
                    editor.switch_mode(Mode::Insert);
                } else if key.code.is_char('q') {
                    break;
                }
            } else if editor.is_insert() {
                if key.code.is_esc() {
                    editor.switch_mode(Mode::Normal);
                } else if key.code.is_backspace() {
                    editor.view.delete();
                } else if key.code.is_tab() {
                    editor.view.insert('\t');
                } else if let Some(char) = key.code.as_char() {
                    editor.view.insert(char);
                }
            }
        }

        let frame = editor.frame();
        render_frame(&frame, &mut stdout)?;
    }
    Ok(())
}

fn render_frame(frame: &Frame, writer: &mut impl Write) -> Result<()> {
    writer.queue(cursor::MoveTo(0, 0))?;
    for ((col, row), cell) in frame.iter() {
        if cell.cursor {
            writer.queue(style::SetAttribute(Attribute::Reverse))?;
        }
        writer
            .queue(cursor::MoveTo(col, row))?
            .queue(style::Print(cell.grapheme.as_str_sanitize()))?
            .queue(style::SetAttribute(Attribute::Reset))?;
    }
    writer.flush()?;
    Ok(())
}
