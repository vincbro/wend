use unicode_width::UnicodeWidthStr;

/// How many bytes a grapheme can hold inline, longer ones become U+FFFD.
const CAPACITY: u8 = 31;

#[derive(Debug, Clone, Copy, Default)]
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

    /// Copies a grapheme in, anything too long or not valid UTF-8 becomes U+FFFD.
    pub fn from_bytes_iter(iter: impl Iterator<Item = u8>) -> Self {
        let mut len = 0;
        let mut bytes = [0; CAPACITY as usize];

        for byte in iter {
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
    #[expect(unsafe_code, reason = "called on every cell of every frame")]
    pub fn as_str(&self) -> &str {
        // SAFETY: `from_bytes_iter` is the only constructor that sets the bytes and it
        // only keeps them if they are valid UTF-8, `Default` is the empty string.
        unsafe { str::from_utf8_unchecked(&self.bytes[0..usize::from(self.len)]) }
    }

    /// Returns the grapheme in a form that is safe to print to a terminal.
    ///
    /// Controls and zero width graphemes are swapped for a single column stand in,
    /// everything else is printed as is, its width matches `UnicodeWidthStr::width`.
    #[must_use]
    pub fn as_str_sanitize(&self) -> &str {
        let str = self.as_str();
        let mut chars = str.chars();
        let Some(c) = chars.next() else {
            // An empty print would leave the old terminal cell untouched
            return " ";
        };

        // Controls always form their own grapheme (UAX #29), the only exception is CRLF
        if c.is_control() && (chars.as_str().is_empty() || str == "\r\n") {
            return match c {
                // The layout already gives these their width
                '\t' | '\n' | '\r' => " ",
                // C0, ESC (0x1B) being the dangerous one
                '\0'..='\u{1F}' => CONTROL_PICTURES[c as usize],
                '\u{7F}' => "␡",
                // C1, some terminals treat 0x9B as CSI
                _ => "\u{FFFD}",
            };
        }

        // Bidi controls, zero width chars, BOM, lone combining marks, ...
        if str.width() == 0 {
            return "\u{FFFD}";
        }

        str
    }
}

/// Unicode control pictures (U+2400..U+241F), a visible single column stand in for C0.
const CONTROL_PICTURES: [&str; 32] = [
    "␀", "␁", "␂", "␃", "␄", "␅", "␆", "␇", "␈", "␉", "␊", "␋", "␌", "␍", "␎", "␏", "␐", "␑", "␒",
    "␓", "␔", "␕", "␖", "␗", "␘", "␙", "␚", "␛", "␜", "␝", "␞", "␟",
];
