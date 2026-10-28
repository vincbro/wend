//! The `wend` binary: the terminal frontend for `wend-core`.

use std::{
    io::{self, Write},
    mem,
    ops::ControlFlow,
    panic,
    path::Path,
};

use anyhow::{Result, bail};
use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event, KeyCode},
    style::{self, Attribute},
    terminal::{self},
};
use wend_core::{
    area::Area,
    editor::{Editor, Mode},
    frame::Frame,
    view::{Direction, Motion},
};

use crate::theme::MELLOW;

mod theme;

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
            Value(value) if file.is_none() => {
                file = Some(value.string()?);
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
    editor.open_file(Path::new(&args.file))?;
    let frame = editor.frame();
    render_frame(&frame, &mut stdout)?;

    // Set by the first key of a two key binding, like the g in gs
    let mut pending_g = false;

    loop {
        if handle_event(&mut editor, &event::read()?, &mut pending_g).is_break() {
            break;
        }
        let frame = editor.frame();
        render_frame(&frame, &mut stdout)?;
    }
    Ok(())
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
        KeyCode::Esc => editor.switch_mode(Mode::Normal),
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
        editor.switch_mode(Mode::Normal);
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
        'i' => editor.switch_mode(Mode::Insert),
        'o' => {
            editor.view_mut().new_line();
            editor.switch_mode(Mode::Insert);
        }
        'v' => editor.switch_mode(if extend { Mode::Normal } else { Mode::Select }),
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
    // Terminals that support it hold the frame back until it is whole, so a half drawn
    // one never shows. The others ignore it.
    writer.queue(terminal::BeginSynchronizedUpdate)?;
    writer.queue(cursor::MoveTo(0, 0))?;
    for (pos, cell) in frame.iter() {
        if cell.covered {
            continue;
        }
        if cell.cursor {
            writer.queue(style::SetBackgroundColor(MELLOW.cursor_background))?;
            writer.queue(style::SetForegroundColor(MELLOW.cursor_foreground))?;
        } else if cell.selected {
            writer.queue(style::SetBackgroundColor(MELLOW.selection_background))?;
            writer.queue(style::SetForegroundColor(MELLOW.selection_foreground))?;
        } else {
            writer.queue(style::SetBackgroundColor(MELLOW.background))?;
            writer.queue(style::SetForegroundColor(MELLOW.foreground))?;
        }
        writer
            .queue(cursor::MoveTo(pos.col, pos.row))?
            .queue(style::Print(cell.grapheme.as_str_sanitize()))?
            .queue(style::SetAttribute(Attribute::Reset))?;
    }
    writer.queue(terminal::EndSynchronizedUpdate)?;
    writer.flush()?;
    Ok(())
}
