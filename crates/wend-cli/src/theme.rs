use std::{fmt, str::FromStr};

use crossterm::style::{Attribute, Color};
use serde::{
    Deserialize, Deserializer,
    de::{Error as _, SeqAccess, Visitor},
};
use toml_edit::de;
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

    /// The names a theme file writes them as.
    const NAMES: [(&'static str, Self); 5] = [
        ("bold", Self::BOLD),
        ("dim", Self::DIM),
        ("italic", Self::ITALIC),
        ("underline", Self::UNDERLINE),
        ("strikethrough", Self::STRIKETHROUGH),
    ];

    fn named(name: &str) -> Option<Self> {
        Self::NAMES
            .into_iter()
            .find(|(known, _)| *known == name)
            .map(|(_, modifier)| modifier)
    }

    /// The terminal attributes that switch these modifiers on.
    pub(crate) fn attributes(self) -> impl Iterator<Item = Attribute> {
        Self::ATTRIBUTES
            .into_iter()
            .filter(move |(modifier, _)| self.contains(*modifier))
            .map(|(_, attribute)| attribute)
    }
}

/// Reads the list of names a theme file gives modifiers.
struct ModifierNames;

impl<'de> Visitor<'de> for ModifierNames {
    type Value = Modifiers;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of bold, dim, italic, underline and strikethrough")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut names: A) -> Result<Modifiers, A::Error> {
        let mut modifiers = Modifiers::NONE;
        while let Some(name) = names.next_element::<String>()? {
            let modifier = Modifiers::named(&name)
                .ok_or_else(|| A::Error::custom(format!("unknown modifier `{name}`")))?;
            modifiers = modifiers.union(modifier);
        }
        Ok(modifiers)
    }
}

impl<'de> Deserialize<'de> for Modifiers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(ModifierNames)
    }
}

/// A color as a theme file writes it: `#rrggbb`.
mod hex {
    use super::*;

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Color>, D::Error> {
        let text = String::deserialize(deserializer)?;
        let digits = text
            .strip_prefix('#')
            .filter(|digits| digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| D::Error::custom(format!("`{text}` is not a color like #c9c7cd")))?;
        let channel = |range| u8::from_str_radix(&digits[range], 16).map_err(D::Error::custom);
        Ok(Some(Color::Rgb {
            r: channel(0..2)?,
            g: channel(2..4)?,
            b: channel(4..6)?,
        }))
    }
}

/// What one layer changes about a cell's look. A color it leaves out shows through from the
/// layers below, so a rule can say only "bold" or only "a background".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Rule {
    #[serde(deserialize_with = "hex::deserialize")]
    pub(crate) foreground: Option<Color>,
    #[serde(deserialize_with = "hex::deserialize")]
    pub(crate) background: Option<Color>,
    pub(crate) modifiers: Modifiers,
}

impl Default for Rule {
    fn default() -> Self {
        Self::NONE
    }
}

impl Rule {
    /// A layer that changes nothing.
    pub(crate) const NONE: Self = Self {
        foreground: None,
        background: None,
        modifiers: Modifiers::NONE,
    };

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

/// Decides how every layer of a cell is drawn. A theme file says every rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
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

impl FromStr for Theme {
    type Err = de::Error;

    /// Reads a theme from the text of a theme file.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        de::from_str(text)
    }
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

/// The themes that ship with wend: every file in the repo's `themes` folder, listed by
/// `build.rs` and sorted by name.
const BUILT_IN: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/themes.rs"));

pub(crate) fn built_in_names() -> impl Iterator<Item = &'static str> {
    BUILT_IN.iter().map(|(name, _)| *name)
}

pub(crate) fn built_in_text(name: &str) -> Option<&'static str> {
    let (_, text) = BUILT_IN.iter().find(|(known, _)| *known == name)?;
    Some(text)
}

impl Theme {
    /// The built-in theme called `name`.
    ///
    /// # Panics
    /// When a built-in theme file doesn't parse, which the tests rule out.
    #[must_use]
    pub(crate) fn built_in(name: &str) -> Option<Self> {
        let text = built_in_text(name)?;
        Some(
            text.parse()
                .expect("every built-in theme file is a valid theme"),
        )
    }
}

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

    impl Rule {
        const fn foreground(color: Color) -> Self {
            Self {
                foreground: Some(color),
                ..Self::NONE
            }
        }

        const fn background(color: Color) -> Self {
            Self {
                background: Some(color),
                ..Self::NONE
            }
        }

        const fn with(self, modifiers: Modifiers) -> Self {
            Self {
                modifiers: self.modifiers.union(modifiers),
                ..self
            }
        }
    }

    const RED: Color = Color::Red;
    const BLUE: Color = Color::Blue;

    fn mellow() -> Theme {
        Theme::built_in("mellow").unwrap()
    }

    fn theme() -> Theme {
        Theme {
            keyword: Rule::foreground(RED).with(Modifiers::BOLD),
            selection: Rule::background(BLUE),
            cursor: Rule::NONE.with(Modifiers::UNDERLINE),
            ..mellow()
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

        assert_eq!(style.foreground, mellow().plain.foreground.unwrap());
        assert_eq!(style.background, mellow().plain.background.unwrap());
        assert_eq!(style.modifiers, Modifiers::NONE);
    }

    #[test]
    fn a_highlight_adds_its_color_and_modifiers_to_the_plain_text() {
        let style = theme().resolve(keyword());

        assert_eq!(style.foreground, RED);
        assert_eq!(
            style.background,
            mellow().plain.background.unwrap(),
            "the rule left it out"
        );
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

    #[test]
    fn every_built_in_theme_parses_and_is_named_once() {
        for &(name, text) in BUILT_IN {
            assert!(text.parse::<Theme>().is_ok(), "{name} parses");
            let count = BUILT_IN.iter().filter(|(known, _)| *known == name).count();
            assert_eq!(count, 1, "{name} is listed once");
        }
    }

    /// The comment lines from the explanation every theme file starts with, down to the
    /// last one.
    fn explanation(text: &str) -> Vec<&str> {
        text.lines()
            .skip_while(|line| *line != "# A wend theme. Save it as `<name>.toml` in the `themes` folder next to config.toml, and pick")
            .filter(|line| line.starts_with('#'))
            .collect()
    }

    #[test]
    fn every_built_in_theme_explains_itself_the_same_way() {
        let first = explanation(built_in_text("default").unwrap());

        assert!(first.len() > 20, "the first theme has the explanation");
        for &(name, text) in BUILT_IN {
            assert_eq!(explanation(text), first, "{name} explains differently");
        }
    }

    #[test]
    fn every_rule_of_a_built_in_theme_has_a_comment() {
        for &(name, text) in BUILT_IN {
            let lines: Vec<_> = text.lines().collect();
            let rules: Vec<_> = (0..lines.len())
                .filter(|&row| lines[row].starts_with('['))
                .collect();
            assert_eq!(rules.len(), 20, "{name} has a section for every rule");
            for row in rules {
                assert!(
                    lines[row - 1].starts_with("# "),
                    "{name}: {} has no comment above it",
                    lines[row]
                );
            }
        }
    }

    #[test]
    fn every_built_in_theme_lists_every_modifier() {
        for &(name, text) in BUILT_IN {
            for (modifier, _) in Modifiers::NAMES {
                assert!(
                    text.contains(&format!("#   \"{modifier}\"")),
                    "{name} doesn't list {modifier}"
                );
            }
        }
    }

    #[test]
    fn a_theme_file_must_say_every_rule() {
        assert!(
            "[keyword]\nforeground = \"#ff0000\"\n"
                .parse::<Theme>()
                .is_err(),
            "the other rules are missing"
        );
    }

    /// The first built-in theme with its keyword section swapped for `rule`.
    fn with_keyword(rule: &str) -> String {
        let text = built_in_text("default").unwrap();
        let (before, rest) = text.split_once("[keyword]\n").unwrap();
        let (_, after) = rest.split_once("\n\n").unwrap();
        format!("{before}[keyword]\n{rule}\n\n{after}")
    }

    #[test]
    fn bad_theme_files_are_errors() {
        assert!(
            with_keyword("foreground = \"#ff0000\"\nmodifiers = [\"bold\"]")
                .parse::<Theme>()
                .is_ok(),
            "a color and a modifier are what a rule is made of"
        );
        assert!(
            with_keyword("foreground = \"red\"")
                .parse::<Theme>()
                .is_err(),
            "a color is hex, not a name"
        );
        assert!(
            with_keyword("foreground = \"#ff00\"")
                .parse::<Theme>()
                .is_err(),
            "a color is six digits"
        );
        assert!(
            with_keyword("foreground = \"#+1+2+3\"")
                .parse::<Theme>()
                .is_err(),
            "a color is hex digits, not anything from_str_radix takes"
        );
        assert!(
            with_keyword("modifiers = [\"loud\"]")
                .parse::<Theme>()
                .is_err(),
            "no such modifier"
        );
        assert!(
            format!("{}\n[keywrd]\n", built_in_text("default").unwrap())
                .parse::<Theme>()
                .is_err(),
            "a typo is not ignored"
        );
    }
}
