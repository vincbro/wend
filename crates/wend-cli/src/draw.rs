use std::io::Write;

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{self, Attribute, Color},
    terminal,
};
use wend_core::frame::{Cell, Frame};

use crate::theme::{MELLOW, Modifiers};

/// What the terminal is told to draw a cell with, after the theme has resolved its layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Style {
    pub(crate) foreground: Color,
    pub(crate) background: Color,
    pub(crate) modifiers: Modifiers,
}

impl Style {
    /// Sets the whole style, for when the terminal's current one isn't known.
    fn queue_all(self, writer: &mut impl Write) -> Result<()> {
        writer
            .queue(style::SetAttribute(Attribute::Reset))?
            .queue(style::SetBackgroundColor(self.background))?
            .queue(style::SetForegroundColor(self.foreground))?;
        for attribute in self.modifiers.attributes() {
            writer.queue(style::SetAttribute(attribute))?;
        }
        Ok(())
    }

    /// Changes the terminal from `previous` to this style, writing only what differs.
    fn queue(self, previous: Self, writer: &mut impl Write) -> Result<()> {
        // A reset is the one way to turn a modifier off that every terminal agrees on, and it
        // also clears the colors, so a change of modifiers sets the whole style again.
        if self.modifiers != previous.modifiers {
            return self.queue_all(writer);
        }
        if self.background != previous.background {
            writer.queue(style::SetBackgroundColor(self.background))?;
        }
        if self.foreground != previous.foreground {
            writer.queue(style::SetForegroundColor(self.foreground))?;
        }
        Ok(())
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            foreground: Color::Reset,
            background: Color::Reset,
            modifiers: Modifiers::NONE,
        }
    }
}

pub(crate) fn render_frame(frame: &Frame, writer: &mut impl Write) -> Result<()> {
    // Terminals that support it hold the frame back until it is whole, so a half-drawn
    // one never shows. The others ignore it.
    writer.queue(terminal::BeginSynchronizedUpdate)?;
    writer.queue(cursor::MoveTo(0, 0))?;
    // The terminal still has the last frame's style, so the first cell sets all of it
    Style::default().queue_all(writer)?;
    let mut last_style = Style::default();
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
    let style = MELLOW.resolve(cell.style);

    if style != last_style {
        style.queue(last_style, writer)?;
    }

    writer.queue(style::Print(cell.grapheme.as_str_sanitize()))?;
    Ok(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::Rgb { r: 255, g: 0, b: 0 };

    fn queued(style: Style, previous: Style) -> String {
        let mut bytes = Vec::new();
        style.queue(previous, &mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn a_color_change_writes_only_that_color() {
        let previous = Style::default();
        let style = Style {
            foreground: RED,
            ..previous
        };

        assert_eq!(queued(style, previous), "\x1b[38;2;255;0;0m");
    }

    #[test]
    fn a_modifier_change_resets_and_sets_the_whole_style() {
        let previous = Style {
            modifiers: Modifiers::BOLD,
            ..Style::default()
        };
        let style = Style {
            foreground: RED,
            modifiers: Modifiers::ITALIC,
            ..Style::default()
        };

        assert_eq!(
            queued(style, previous),
            "\x1b[0m\x1b[49m\x1b[38;2;255;0;0m\x1b[3m",
            "reset, both colors, then italic"
        );
    }
}
