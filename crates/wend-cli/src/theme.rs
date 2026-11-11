use crossterm::style::{Attribute, Color};
use wend_core::{
    frame::{Content, Style as CellStyle},
    syntax::Highlight,
};

use crate::draw::Style;

/// The text attributes a theme can turn on, kept as bits so a layer can add its own to the
/// ones below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Modifiers(u8);

impl Modifiers {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const BOLD: Self = Self(1);
    pub(crate) const DIM: Self = Self(1 << 1);
    pub(crate) const ITALIC: Self = Self(1 << 2);
    pub(crate) const UNDERLINE: Self = Self(1 << 3);
    pub(crate) const STRIKETHROUGH: Self = Self(1 << 4);

    const ATTRIBUTES: [(Self, Attribute); 5] = [
        (Self::BOLD, Attribute::Bold),
        (Self::DIM, Attribute::Dim),
        (Self::ITALIC, Attribute::Italic),
        (Self::UNDERLINE, Attribute::Underlined),
        (Self::STRIKETHROUGH, Attribute::CrossedOut),
    ];

    #[must_use]
    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The terminal attributes that switch these modifiers on.
    pub(crate) fn attributes(self) -> impl Iterator<Item = Attribute> {
        Self::ATTRIBUTES
            .into_iter()
            .filter(move |(modifier, _)| self.contains(*modifier))
            .map(|(_, attribute)| attribute)
    }
}

/// What one layer changes about a cell's look. A color it leaves out shows through from the
/// layers below, so a rule can say only "bold" or only "a background".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    pub(crate) foreground: Option<Color>,
    pub(crate) background: Option<Color>,
    pub(crate) modifiers: Modifiers,
}

impl Rule {
    /// A layer that changes nothing.
    pub(crate) const NONE: Self = Self {
        foreground: None,
        background: None,
        modifiers: Modifiers::NONE,
    };

    pub(crate) const fn foreground(color: Color) -> Self {
        Self {
            foreground: Some(color),
            ..Self::NONE
        }
    }

    pub(crate) const fn background(color: Color) -> Self {
        Self {
            background: Some(color),
            ..Self::NONE
        }
    }

    pub(crate) const fn with(self, modifiers: Modifiers) -> Self {
        Self {
            modifiers: self.modifiers.union(modifiers),
            ..self
        }
    }

    /// Lays `top` over `self`: a color `top` sets wins, one it leaves out shows through,
    /// and the modifiers of both stay on.
    #[must_use]
    fn over(self, top: Self) -> Self {
        Self {
            foreground: top.foreground.or(self.foreground),
            background: top.background.or(self.background),
            modifiers: self.modifiers.union(top.modifiers),
        }
    }
}

/// Decides how every layer of a cell is drawn.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    /// The bottom of every cell, so it sets both colors.
    pub(crate) plain: Rule,
    pub(crate) line_number: Rule,
    pub(crate) current_line_number: Rule,
    pub(crate) marker: Rule,
    pub(crate) selection: Rule,
    pub(crate) cursor: Rule,

    pub(crate) attribute: Rule,
    pub(crate) comment: Rule,
    pub(crate) constant: Rule,
    pub(crate) constructor: Rule,
    pub(crate) escape: Rule,
    pub(crate) function: Rule,
    pub(crate) keyword: Rule,
    pub(crate) label: Rule,
    pub(crate) operator: Rule,
    pub(crate) property: Rule,
    pub(crate) punctuation: Rule,
    pub(crate) string: Rule,
    pub(crate) type_name: Rule,
    pub(crate) variable: Rule,
}

impl Theme {
    /// Turns a cell's layers into what the terminal draws: the content over the plain
    /// text, then the selection, then the cursor.
    #[must_use]
    pub(crate) fn resolve(&self, style: CellStyle) -> Style {
        let mut rule = self.plain.over(self.content(style.content));
        if style.selected {
            rule = rule.over(self.selection);
        }
        if style.cursor {
            rule = rule.over(self.cursor);
        }
        Style {
            foreground: rule.foreground.unwrap_or(Color::Reset),
            background: rule.background.unwrap_or(Color::Reset),
            modifiers: rule.modifiers,
        }
    }

    fn content(&self, content: Content) -> Rule {
        match content {
            Content::Plain => Rule::NONE,
            Content::Text(highlight) => self.highlight(highlight),
            Content::LineNumber => self.line_number,
            Content::CurrentLineNumber => self.current_line_number,
            Content::Marker => self.marker,
        }
    }

    fn highlight(&self, highlight: Highlight) -> Rule {
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

const BACKGROUND: Color = rgb(0x16, 0x16, 0x17);
const FOREGROUND: Color = rgb(0xc9, 0xc7, 0xcd);
const COMMENT: Color = rgb(0x65, 0x65, 0x6b);
const KEYWORD: Color = rgb(0xac, 0xa1, 0xcf);

/// Mellow by Rohit K Viswanath, the palette and choices of helix's `mellow.toml`.
pub(crate) static MELLOW: Theme = Theme {
    plain: Rule {
        foreground: Some(FOREGROUND),
        background: Some(BACKGROUND),
        modifiers: Modifiers::NONE,
    },
    line_number: Rule::foreground(COMMENT),
    current_line_number: Rule::foreground(KEYWORD),
    marker: Rule::NONE,
    // gray03 and gray04, helix leaves the text's color as it is on both
    selection: Rule::background(rgb(0x3e, 0x3e, 0x43)),
    cursor: Rule::background(rgb(0x57, 0x57, 0x5f)),

    // Picked from mellow's palette by eye, not copied from helix's `mellow.toml`
    attribute: Rule::foreground(rgb(0xac, 0xa1, 0xcf)),
    comment: Rule::foreground(COMMENT).with(Modifiers::ITALIC),
    constant: Rule::foreground(rgb(0xf5, 0xa1, 0x91)),
    constructor: Rule::foreground(rgb(0xe2, 0x9e, 0xca)),
    escape: Rule::foreground(rgb(0xea, 0x83, 0xa5)),
    function: Rule::foreground(rgb(0xe6, 0xb9, 0x9d)),
    keyword: Rule::foreground(KEYWORD),
    label: Rule::foreground(rgb(0xe6, 0xb9, 0x9d)),
    operator: Rule::foreground(rgb(0xb4, 0xb4, 0xb9)),
    property: Rule::foreground(rgb(0xb5, 0xb4, 0xc9)),
    punctuation: Rule::foreground(rgb(0x92, 0x91, 0x99)),
    string: Rule::foreground(rgb(0x90, 0xb9, 0x9f)),
    type_name: Rule::foreground(rgb(0xe2, 0x9e, 0xca)),
    variable: Rule::foreground(FOREGROUND),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_union_keeps_both_sets() {
        let both = Modifiers::BOLD.union(Modifiers::ITALIC);

        assert!(both.contains(Modifiers::BOLD), "from the left");
        assert!(both.contains(Modifiers::ITALIC), "from the right");
        assert!(!both.contains(Modifiers::UNDERLINE), "never added");
    }

    #[test]
    fn attributes_lists_what_is_on() {
        let both = Modifiers::BOLD.union(Modifiers::ITALIC);

        assert_eq!(
            both.attributes().collect::<Vec<_>>(),
            [Attribute::Bold, Attribute::Italic]
        );
        assert_eq!(Modifiers::NONE.attributes().count(), 0, "nothing is on");
    }

    const RED: Color = Color::Red;
    const BLUE: Color = Color::Blue;

    fn theme() -> Theme {
        Theme {
            keyword: Rule::foreground(RED).with(Modifiers::BOLD),
            selection: Rule::background(BLUE),
            cursor: Rule::NONE.with(Modifiers::UNDERLINE),
            ..MELLOW
        }
    }

    fn keyword() -> CellStyle {
        CellStyle {
            content: Content::Text(Highlight::Keyword),
            ..CellStyle::default()
        }
    }

    #[test]
    fn plain_text_is_drawn_in_the_plain_colors() {
        let style = theme().resolve(CellStyle::default());

        assert_eq!(style.foreground, FOREGROUND);
        assert_eq!(style.background, BACKGROUND);
        assert_eq!(style.modifiers, Modifiers::NONE);
    }

    #[test]
    fn a_highlight_adds_its_color_and_modifiers_to_the_plain_text() {
        let style = theme().resolve(keyword());

        assert_eq!(style.foreground, RED);
        assert_eq!(style.background, BACKGROUND, "the rule left it out");
        assert_eq!(style.modifiers, Modifiers::BOLD);
    }

    #[test]
    fn a_selected_keyword_keeps_its_color_and_gets_the_selection_background() {
        let style = theme().resolve(CellStyle {
            selected: true,
            ..keyword()
        });

        assert_eq!(style.foreground, RED, "the selection leaves it alone");
        assert_eq!(style.background, BLUE);
        assert_eq!(style.modifiers, Modifiers::BOLD);
    }

    #[test]
    fn the_cursor_layers_over_the_selection() {
        let style = theme().resolve(CellStyle {
            selected: true,
            cursor: true,
            ..keyword()
        });

        assert_eq!(style.background, BLUE, "the cursor rule has no background");
        assert_eq!(
            style.modifiers,
            Modifiers::BOLD.union(Modifiers::UNDERLINE),
            "each layer adds its own"
        );
    }

    #[test]
    fn a_later_layer_wins_a_color_both_set() {
        let top = Rule::foreground(BLUE);

        assert_eq!(Rule::foreground(RED).over(top).foreground, Some(BLUE));
    }
}
