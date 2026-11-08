use std::io::{self, Write};

use anyhow::Result;
use crossterm::{QueueableCommand, cursor, terminal};

/// Puts the terminal in raw mode on the alternate screen, and restores it when dropped,
/// so an early `?` return can't leave the user's shell broken.
pub(crate) struct TerminalGuard;

impl TerminalGuard {
    pub(crate) fn enter() -> Result<Self> {
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
pub(crate) fn restore_terminal() {
    let mut stdout = io::stdout();
    drop(
        stdout
            .queue(cursor::Show)
            .and_then(|stdout| stdout.queue(terminal::LeaveAlternateScreen))
            .and_then(Write::flush),
    );
    drop(terminal::disable_raw_mode());
}
