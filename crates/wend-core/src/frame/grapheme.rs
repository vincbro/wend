use unicode_width::UnicodeWidthStr;

/// Sets how many bytes a grapheme can hold inline, longer ones become U+FFFD.
const CAPACITY: u8 = 31;

const REPLACEMENT: &str = "\u{FFFD}";

/// Stores a grapheme inline, so a cell never allocates.
///
/// The first `len` bytes are always valid UTF-8, which `as_str` relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Grapheme {
    len: u8,
    bytes: [u8; CAPACITY as usize],
}

impl Grapheme {
    #[must_use]
    pub fn from_char(c: char) -> Self {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).bytes();
        Self::from_bytes_iter(bytes)
    }

    /// Creates the single space that blank cells show.
    #[must_use]
    pub(crate) fn blank() -> Self {
        Self::from_char(' ')
    }

    /// Copies a grapheme in, anything too long or not valid UTF-8 becomes U+FFFD.
    pub fn from_bytes_iter(source: impl Iterator<Item = u8>) -> Self {
        let mut len = 0;
        let mut bytes = [0; CAPACITY as usize];

        for byte in source {
            if len >= CAPACITY {
                return Self::from_char(char::REPLACEMENT_CHARACTER);
            }
            bytes[usize::from(len)] = byte;
            len += 1;
        }

        if str::from_utf8(&bytes[..usize::from(len)]).is_err() {
            return Self::from_char(char::REPLACEMENT_CHARACTER);
        }

        Self { len, bytes }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.as_str_sanitize().width()
    }

    #[must_use]
    #[allow(unsafe_code, reason = "called on every cell of every frame")]
    pub fn as_str(&self) -> &str {
        // SAFETY: `from_bytes_iter` is the only constructor that sets the bytes and it
        // only keeps them if they are valid UTF-8, `Default` is the empty string.
        unsafe { str::from_utf8_unchecked(&self.bytes[0..usize::from(self.len)]) }
    }

    /// Returns the grapheme in a form that is safe to print to a terminal.
    ///
    /// Controls and zero width graphemes are swapped for a single column stand-in,
    /// everything else is printed as is, its width matches `UnicodeWidthStr::width`.
    #[must_use]
    pub fn as_str_sanitize(&self) -> &str {
        let text = self.as_str();
        let mut chars = text.chars();
        let Some(c) = chars.next() else {
            // An empty print would leave the old terminal cell untouched
            return " ";
        };

        // Controls always form their own grapheme (UAX #29), the only exception is CRLF
        if c.is_control() && (chars.as_str().is_empty() || text == "\r\n") {
            return match c {
                // The layout already gives these their width
                '\t' | '\n' | '\r' => " ",
                // C0, ESC (0x1B) being the dangerous one
                '\0'..='\u{1F}' => CONTROL_PICTURES[c as usize],
                '\u{7F}' => "␡",
                // C1, some terminals treat 0x9B as CSI
                _ => REPLACEMENT,
            };
        }

        // Bidi controls, zero width chars, BOM, lone combining marks, ...
        if text.width() == 0 {
            return REPLACEMENT;
        }

        text
    }
}

/// Lists the control pictures (U+2400..U+241F), a visible single column stand-in for C0.
const CONTROL_PICTURES: [&str; 32] = [
    "␀", "␁", "␂", "␃", "␄", "␅", "␆", "␇", "␈", "␉", "␊", "␋", "␌", "␍", "␎", "␏", "␐", "␑", "␒",
    "␓", "␔", "␕", "␖", "␗", "␘", "␙", "␚", "␛", "␜", "␝", "␞", "␟",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn grapheme(text: &str) -> Grapheme {
        Grapheme::from_bytes_iter(text.bytes())
    }

    fn sanitized(text: &str) -> String {
        grapheme(text).as_str_sanitize().to_owned()
    }

    #[test]
    fn printable_text_is_kept() {
        assert_eq!(sanitized("a"), "a", "ascii");
        assert_eq!(sanitized("漢"), "漢", "a wide glyph");
        assert_eq!(
            sanitized("e\u{301}"),
            "e\u{301}",
            "a letter with its accent"
        );
    }

    #[test]
    fn the_default_prints_as_a_space() {
        assert_eq!(
            Grapheme::default().as_str_sanitize(),
            " ",
            "an empty print would leave the old terminal cell untouched"
        );
    }

    #[test]
    fn layout_controls_print_as_a_space() {
        for text in ["\t", "\n", "\r", "\r\n"] {
            assert_eq!(
                sanitized(text),
                " ",
                "the layout already gives {text:?} its width"
            );
        }
    }

    #[test]
    fn c0_controls_print_as_control_pictures() {
        assert_eq!(sanitized("\0"), "␀", "the first C0");
        assert_eq!(
            sanitized("\u{1b}"),
            "␛",
            "escape, which must never reach the terminal"
        );
        assert_eq!(sanitized("\u{1f}"), "␟", "the last C0");
        assert_eq!(sanitized("\u{7f}"), "␡", "delete");
    }

    #[test]
    fn c1_controls_print_as_the_replacement() {
        assert_eq!(
            sanitized("\u{9b}"),
            REPLACEMENT,
            "some terminals treat 0x9B as CSI"
        );
    }

    #[test]
    fn zero_width_text_prints_as_the_replacement() {
        assert_eq!(sanitized("\u{200b}"), REPLACEMENT, "a zero width space");
        assert_eq!(sanitized("\u{feff}"), REPLACEMENT, "a byte order mark");
        assert_eq!(sanitized("\u{301}"), REPLACEMENT, "a lone combining mark");
    }

    #[test]
    fn width_follows_what_is_printed() {
        assert_eq!(grapheme("a").width(), 1, "ascii");
        assert_eq!(grapheme("漢").width(), 2, "a wide glyph");
        assert_eq!(grapheme("\u{200b}").width(), 1, "the replacement stands in");
    }

    #[test]
    fn a_grapheme_that_just_fits_is_kept() {
        let text = "a".repeat(usize::from(CAPACITY));
        assert_eq!(grapheme(&text).as_str(), text);
    }

    #[test]
    fn a_grapheme_that_is_too_long_becomes_the_replacement() {
        let text = "a".repeat(usize::from(CAPACITY) + 1);
        assert_eq!(grapheme(&text).as_str(), REPLACEMENT, "one byte over");
    }

    #[test]
    fn invalid_utf8_becomes_the_replacement() {
        let invalid = Grapheme::from_bytes_iter([0xff].into_iter());
        assert_eq!(invalid.as_str(), REPLACEMENT, "0xff is never valid utf-8");
    }
}
