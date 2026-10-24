use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, Default)]
pub struct Grapheme {
    len: u8,
    bytes: [u8; 31],
}

impl Grapheme {
    #[must_use]
    pub fn from_char(c: char) -> Self {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).bytes();
        Self::from_bytes_iter(bytes)
    }

    pub fn from_bytes_iter(iter: impl Iterator<Item = u8>) -> Self {
        let mut ptr = 0;
        let mut bytes = [0; 31];

        for byte in iter {
            if ptr >= 15 {
                break;
            }
            bytes[usize::from(ptr)] = byte;
            ptr += 1;
        }

        Self { len: ptr, bytes }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.as_str_sanitize().width()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        unsafe {
            // SAFETY:
            // We know that a grapheme has to be a safe
            str::from_utf8_unchecked(&self.bytes[0..usize::from(self.len)])
        }
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
