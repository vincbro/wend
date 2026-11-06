use crossterm::style::Color;
use wend_core::syntax::Highlight;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    pub(crate) background: Color,
    pub(crate) foreground: Color,

    pub(crate) selection_background: Color,
    pub(crate) selection_foreground: Color,

    pub(crate) cursor_foreground: Color,
    pub(crate) cursor_background: Color,

    pub(crate) attribute: Color,
    pub(crate) comment: Color,
    pub(crate) constant: Color,
    pub(crate) constructor: Color,
    pub(crate) escape: Color,
    pub(crate) function: Color,
    pub(crate) keyword: Color,
    pub(crate) label: Color,
    pub(crate) operator: Color,
    pub(crate) property: Color,
    pub(crate) punctuation: Color,
    pub(crate) string: Color,
    pub(crate) type_name: Color,
    pub(crate) variable: Color,
}

impl Theme {
    /// The color text is drawn in, which is the plain foreground for text with no highlight.
    pub(crate) fn foreground_of(&self, highlight: Option<Highlight>) -> Color {
        let Some(highlight) = highlight else {
            return self.foreground;
        };
        match highlight {
            Highlight::Attribute => self.attribute,
            Highlight::Comment => self.comment,
            Highlight::Constant => self.constant,
            Highlight::Constructor => self.constructor,
            Highlight::Escape => self.escape,
            Highlight::Function => self.function,
            Highlight::Keyword => self.keyword,
            Highlight::Label => self.label,
            Highlight::Operator => self.operator,
            Highlight::Property => self.property,
            Highlight::Punctuation => self.punctuation,
            Highlight::String => self.string,
            Highlight::Type => self.type_name,
            Highlight::Variable => self.variable,
        }
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb {
        r: red,
        g: green,
        b: blue,
    }
}

const FOREGROUND: Color = Color::Rgb {
    r: 0xc9,
    g: 0xc7,
    b: 0xcd,
};

/// Mellow by Rohit K Viswanath, the palette and choices of helix's `mellow.toml`.
pub(crate) const MELLOW: Theme = Theme {
    background: Color::Rgb {
        r: 0x16,
        g: 0x16,
        b: 0x17,
    },
    foreground: FOREGROUND,
    // gray03, helix leaves the text as is so it keeps fg
    selection_background: Color::Rgb {
        r: 0x3e,
        g: 0x3e,
        b: 0x43,
    },
    selection_foreground: FOREGROUND,
    // fg on gray04, helix only sets the cursor's bg
    cursor_foreground: FOREGROUND,
    cursor_background: Color::Rgb {
        r: 0x57,
        g: 0x57,
        b: 0x5f,
    },

    // Picked from mellow's palette by eye, not copied from helix's `mellow.toml`
    attribute: rgb(0xac, 0xa1, 0xcf),
    comment: rgb(0x65, 0x65, 0x6b),
    constant: rgb(0xf5, 0xa1, 0x91),
    constructor: rgb(0xe2, 0x9e, 0xca),
    escape: rgb(0xea, 0x83, 0xa5),
    function: rgb(0xe6, 0xb9, 0x9d),
    keyword: rgb(0xac, 0xa1, 0xcf),
    label: rgb(0xe6, 0xb9, 0x9d),
    operator: rgb(0xb4, 0xb4, 0xb9),
    property: rgb(0xb5, 0xb4, 0xc9),
    punctuation: rgb(0x92, 0x91, 0x99),
    string: rgb(0x90, 0xb9, 0x9f),
    type_name: rgb(0xe2, 0x9e, 0xca),
    variable: FOREGROUND,
};
