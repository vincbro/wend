use std::io::Write;

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{self, Color},
    terminal,
};
use wend_core::frame::{Cell, Frame};

use crate::theme::MELLOW;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Style {
    foreground: Color,
    background: Color,
}

impl Style {
    fn queue(self, writer: &mut impl Write) -> Result<()> {
        writer
            .queue(style::SetBackgroundColor(self.background))?
            .queue(style::SetForegroundColor(self.foreground))?;
        Ok(())
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            foreground: Color::Reset,
            background: Color::Reset,
        }
    }
}

pub(crate) fn render_frame(frame: &Frame, writer: &mut impl Write) -> Result<()> {
    // Terminals that support it hold the frame back until it is whole, so a half-drawn
    // one never shows. The others ignore it.
    writer.queue(terminal::BeginSynchronizedUpdate)?;
    writer.queue(cursor::MoveTo(0, 0))?;
    let mut last_style = Style::default();
    last_style.queue(writer)?;
    // The terminal advances the cursor by what it thinks a glyph is wide, so only plain
    // ASCII, which it can't disagree on, lets the next cell skip the move. Everything else
    // moves again, so a glyph the terminal measures differently can't shift the row.
    let mut needs_move = true;
    for (pos, cell) in frame.iter() {
        if cell.covered {
            continue;
        }
        if needs_move || pos.col == 0 {
            writer.queue(cursor::MoveTo(pos.col, pos.row))?;
        }
        last_style = queue_cell(cell, last_style, writer)?;
        needs_move = cell.grapheme.as_str_sanitize().len() != 1;
    }
    writer.queue(terminal::EndSynchronizedUpdate)?;
    Ok(())
}

fn queue_cell(cell: &Cell, last_style: Style, writer: &mut impl Write) -> Result<Style> {
    let (background, foreground) = if cell.cursor {
        (MELLOW.cursor_background, MELLOW.cursor_foreground)
    } else if cell.selected {
        (MELLOW.selection_background, MELLOW.selection_foreground)
    } else {
        (MELLOW.background, MELLOW.foreground_of(cell.highlight))
    };

    let style = Style {
        foreground,
        background,
    };

    if style != last_style {
        style.queue(writer)?;
    }

    writer.queue(style::Print(cell.grapheme.as_str_sanitize()))?;
    Ok(style)
}
