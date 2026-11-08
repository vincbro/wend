use std::io::Write;

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{self, Attribute},
    terminal,
};
use wend_core::frame::{Cell, CellPosition, Frame};

use crate::theme::MELLOW;

pub(crate) fn render_frame(frame: &Frame, writer: &mut impl Write) -> Result<()> {
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
