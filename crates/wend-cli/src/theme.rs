use crossterm::style::Color;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    pub(crate) background: Color,
    pub(crate) foreground: Color,

    pub(crate) selection_bg: Color,
    pub(crate) selection_fg: Color,

    pub(crate) cursor_fg: Color,
    pub(crate) cursor_bg: Color,
}

/// Mellow by Rohit K Viswanath, the palette and choices of helix's `mellow.toml`.
pub(crate) const MELLOW: Theme = Theme {
    // bg
    background: Color::Rgb {
        r: 0x16,
        g: 0x16,
        b: 0x17,
    },
    // fg
    foreground: Color::Rgb {
        r: 0xc9,
        g: 0xc7,
        b: 0xcd,
    },
    // gray03, helix leaves the text as is so it keeps fg
    selection_bg: Color::Rgb {
        r: 0x3e,
        g: 0x3e,
        b: 0x43,
    },
    selection_fg: Color::Rgb {
        r: 0xc9,
        g: 0xc7,
        b: 0xcd,
    },
    // fg on gray04, helix only sets the cursor's bg
    cursor_fg: Color::Rgb {
        r: 0xc9,
        g: 0xc7,
        b: 0xcd,
    },
    cursor_bg: Color::Rgb {
        r: 0x57,
        g: 0x57,
        b: 0x5f,
    },
};
