//! The `wend` binary: the terminal frontend for `wend-core`.

use std::{
    io::{self, Write},
    mem,
    ops::ControlFlow,
    panic,
    path::Path,
};

use anyhow::{Context, Result, bail};
use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event, KeyCode},
    style::{self, Attribute},
    terminal::{self},
};
use wend_core::{
    editor::{Editor, Mode},
    frame::{Area, Cell, CellPosition, Frame},
    view::{Direction, Motion},
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

    let _guard = TerminalGuard::enter().context("failed to set up the terminal")?;
    let mut stdout = io::stdout();

    let window = terminal::window_size().context("failed to read the terminal size")?;
    let area = Area::new(window.columns, window.rows);
    let mut editor = Editor::new(area);
    editor
        .open_file(Path::new(&args.file))
        .with_context(|| format!("failed to open {}", args.file))?;
    let mut frame = editor.frame();
    draw(&mut editor, &mut frame, &mut stdout)?;

    // Set by the first key of a two-key binding, like the g in gs
    let mut pending_g = false;

    loop {
        let event = event::read().context("failed to read a terminal event")?;
        if handle_event(&mut editor, &event, &mut pending_g).is_break() {
            break;
        }
        draw(&mut editor, &mut frame, &mut stdout)?;
    }
    Ok(())
}

fn draw(editor: &mut Editor, frame: &mut Frame, writer: &mut impl Write) -> Result<()> {
    editor.update_frame(frame);
    render_frame(frame, writer).context("failed to draw the frame")
}

fn handle_event(editor: &mut Editor, event: &Event, pending_g: &mut bool) -> ControlFlow<()> {
    match *event {
        Event::Resize(cols, rows) => editor.resize(Area::new(cols, rows)),
        Event::Key(key) if editor.is_insert() => insert_key(editor, key.code),
        Event::Key(key) => return normal_key(editor, key.code, pending_g),
        _ => {}
    }
    ControlFlow::Continue(())
}

fn insert_key(editor: &mut Editor, code: KeyCode) {
    match code {
        KeyCode::Esc => editor.set_mode(Mode::Normal),
        KeyCode::Backspace => editor.view_mut().delete(),
        KeyCode::Tab => editor.view_mut().insert('\t'),
        KeyCode::Enter => editor.view_mut().insert('\n'),
        KeyCode::Char(c) => editor.view_mut().insert(c),
        _ => {}
    }
}

/// Normal and Select share one keymap, Select only makes motions extend.
fn normal_key(editor: &mut Editor, code: KeyCode, pending_g: &mut bool) -> ControlFlow<()> {
    let extend = editor.is_select();

    if mem::take(pending_g) {
        if let Some(motion) = g_motion(code) {
            editor.view_mut().move_selection(motion, extend);
        }
        return ControlFlow::Continue(());
    }
    if code.is_esc() {
        editor.set_mode(Mode::Normal);
        return ControlFlow::Continue(());
    }
    let Some(c) = code.as_char() else {
        return ControlFlow::Continue(());
    };
    if let Some(motion) = motion(c) {
        editor.view_mut().move_selection(motion, extend);
        return ControlFlow::Continue(());
    }

    match c {
        'g' => *pending_g = true,
        'd' => editor.view_mut().delete_selection(),
        'u' => editor.view_mut().revert(),
        // TODO: show the error once there is a status line
        's' => drop(editor.view_mut().save()),
        'i' => editor.set_mode(Mode::Insert),
        'o' => {
            editor.view_mut().new_line();
            editor.set_mode(Mode::Insert);
        }
        'v' => editor.set_mode(if extend { Mode::Normal } else { Mode::Select }),
        'q' => return ControlFlow::Break(()),
        _ => {}
    }
    ControlFlow::Continue(())
}

fn motion(c: char) -> Option<Motion> {
    let motion = match c {
        'h' => Motion::Horizontal(Direction::Backward),
        'l' => Motion::Horizontal(Direction::Forward),
        'j' => Motion::Vertical(Direction::Forward),
        'k' => Motion::Vertical(Direction::Backward),
        'w' => Motion::Word(Direction::Forward),
        'b' => Motion::Word(Direction::Backward),
        'H' => Motion::Line(Direction::Backward),
        'L' => Motion::Line(Direction::Forward),
        'J' => Motion::Page(Direction::Forward),
        'K' => Motion::Page(Direction::Backward),
        _ => return None,
    };
    Some(motion)
}

/// The motion for the key after a g.
fn g_motion(code: KeyCode) -> Option<Motion> {
    match code.as_char()? {
        's' => Some(Motion::File(Direction::Backward)),
        'e' => Some(Motion::File(Direction::Forward)),
        _ => None,
    }
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
        (MELLOW.background, MELLOW.foreground)
    };
    writer
        .queue(style::SetBackgroundColor(background))?
        .queue(style::SetForegroundColor(foreground))?
        .queue(cursor::MoveTo(pos.col, pos.row))?
        .queue(style::Print(cell.grapheme.as_str_sanitize()))?
        .queue(style::SetAttribute(Attribute::Reset))?;
    Ok(())
}
