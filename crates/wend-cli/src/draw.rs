use std::io::Write;

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{self, Attribute, Color},
    terminal,
};
use wend_core::frame::{Cell, Frame};

use crate::theme::{Modifiers, Theme};

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

/// Draws `frame`, leaving out the cells that are the same in `previous`, which is the frame the
/// terminal shows now. Without one, or with one of another size, it draws every cell.
pub(crate) fn render_frame(
    frame: &Frame,
    previous: Option<&Frame>,
    theme: &Theme,
    writer: &mut impl Write,
) -> Result<()> {
    // The cells of a frame of another size don't line up with these
    let previous = previous.filter(|previous| previous.area() == frame.area());
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
        if previous.is_some_and(|previous| unchanged(cell, previous.get(pos))) {
            // The cursor stays where it is, so the next cell drawn has to move it
            needs_move = true;
            continue;
        }
        if needs_move || pos.col == 0 {
            writer.queue(cursor::MoveTo(pos.col, pos.row))?;
        }
        last_style = queue_cell(cell, theme, last_style, writer)?;
        needs_move = cell.grapheme.as_str_sanitize().len() != 1;
    }
    writer.queue(terminal::EndSynchronizedUpdate)?;
    Ok(())
}

/// Returns whether `cell` looks the same as the `shown` one. The layer is left out, because
/// it only decides which writes win and not what is drawn.
fn unchanged(cell: &Cell, shown: &Cell) -> bool {
    cell.grapheme == shown.grapheme && cell.style == shown.style && cell.covered == shown.covered
}

fn queue_cell(
    cell: &Cell,
    theme: &Theme,
    last_style: Style,
    writer: &mut impl Write,
) -> Result<Style> {
    let style = theme.resolve(cell.style);

    if style != last_style {
        style.queue(last_style, writer)?;
    }

    writer.queue(style::Print(cell.grapheme.as_str_sanitize()))?;
    Ok(style)
}

#[cfg(test)]
mod tests {
    use wend_core::frame::{Area, CellPosition, Grapheme};

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

    /// A frame of one row with `text` in it, one glyph to a cell.
    fn row(text: &str) -> Frame {
        let cols = u16::try_from(text.chars().count()).unwrap();
        let mut frame = Frame::new(Area::new(cols, 1), None);
        for (col, glyph) in (0..).zip(text.chars()) {
            let cell = Cell {
                grapheme: Grapheme::from_char(glyph),
                ..Cell::default()
            };
            frame.write(CellPosition::new(col, 0), cell);
        }
        frame
    }

    /// What drawing `frame` over `previous` writes.
    fn drawn(frame: &Frame, previous: Option<&Frame>) -> String {
        let theme = Theme::built_in("default").unwrap();
        let mut bytes = Vec::new();
        render_frame(frame, previous, &theme, &mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    /// The text `bytes` prints, without the escape sequences that move and color it.
    fn printed(bytes: &str) -> String {
        let mut text = String::new();
        let mut chars = bytes.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // An escape sequence ends with a letter, or a symbol like `~` or `@`
                chars
                    .by_ref()
                    .find(|&end| end.is_ascii_alphabetic() || end == '~');
            } else {
                text.push(c);
            }
        }
        text
    }

    #[test]
    fn without_a_previous_frame_every_cell_is_drawn() {
        assert_eq!(printed(&drawn(&row("abcd"), None)), "abcd");
    }

    #[test]
    fn a_frame_like_the_previous_one_draws_no_cell() {
        let frame = row("abcd");

        assert_eq!(
            printed(&drawn(&frame, Some(&frame))),
            "",
            "nothing changed, nothing is written"
        );
    }

    #[test]
    fn only_the_cell_that_changed_is_drawn_after_a_move_to_it() {
        let bytes = drawn(&row("abcd"), Some(&row("abXd")));

        assert_eq!(printed(&bytes), "c", "the rest is left alone");
        assert!(
            bytes.contains("\x1b[1;3H"),
            "a move to the third column: {bytes:?}"
        );
    }

    #[test]
    fn cells_that_changed_next_to_each_other_share_one_move() {
        let bytes = drawn(&row("abcd"), Some(&row("aXYd")));

        assert_eq!(printed(&bytes), "bc");
        assert!(bytes.contains("\x1b[1;2H"), "a move to the first of them");
        assert!(!bytes.contains("\x1b[1;3H"), "and none to the second");
    }

    #[test]
    fn a_cell_after_an_unchanged_one_moves_again() {
        let bytes = drawn(&row("abcd"), Some(&row("Xbcd")));

        assert_eq!(printed(&bytes), "a");
        let bytes = drawn(&row("abcd"), Some(&row("aXcY")));
        assert_eq!(printed(&bytes), "bd", "the cell between is left alone");
        assert!(
            bytes.contains("\x1b[1;4H"),
            "and the cursor jumps over it: {bytes:?}"
        );
    }

    #[test]
    fn a_frame_of_another_size_is_drawn_whole() {
        let bytes = drawn(&row("abcd"), Some(&row("ab")));

        assert_eq!(
            printed(&bytes),
            "abcd",
            "the old frame's cells don't line up"
        );
    }

    #[test]
    fn a_cell_that_stopped_being_covered_is_a_change() {
        let covered = Cell {
            covered: true,
            ..Cell::default()
        };

        assert!(
            !unchanged(&Cell::default(), &covered),
            "a blank and the covered half of a wide glyph look the same but aren't"
        );
    }
}
