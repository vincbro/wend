//! The `wend` binary: the terminal frontend for `wend-core`.

use std::io::{self, Write};

use anyhow::{Result, bail};
use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event},
    style::{self, Attribute},
    terminal::{self},
};
use wend_core::{area::Area, editor::Editor, frame::Frame};

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

fn main() -> Result<()> {
    let args = parse_args()?;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    stdout
        .queue(terminal::EnterAlternateScreen)?
        .queue(cursor::Hide)?
        .flush()?;

    let window = terminal::window_size()?;
    let area = Area::new(window.columns, window.rows);
    let mut editor = Editor::new(area);
    editor.open_file(args.file.as_str())?;
    let frame = editor.frame();
    render_frame(&frame, &mut stdout)?;

    loop {
        let event = event::read()?;
        if let Event::Key(key) = event {
            if key.code.is_char('h') {
                editor.view.left();
            } else if key.code.is_char('j') {
                editor.view.down();
            } else if key.code.is_char('k') {
                editor.view.up();
            } else if key.code.is_char('l') {
                editor.view.right();
            } else if key.code.is_enter() || key.code.is_char('q') {
                break;
            }
        }

        let frame = editor.frame();
        render_frame(&frame, &mut stdout)?;
    }
    stdout
        .queue(cursor::Show)?
        .queue(terminal::LeaveAlternateScreen)?
        .flush()?;
    terminal::disable_raw_mode()?;
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
