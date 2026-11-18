//! Validated face descriptors and original-format byte identification.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Format {
    Woff2,
    Woff,
    Ttf,
    Otf,
}

impl Format {
    pub(super) fn identify(bytes: &[u8]) -> Option<Self> {
        let format = match bytes.get(..4)? {
            b"wOF2" if bytes.len() >= 48 => Self::Woff2,
            b"wOFF" if bytes.len() >= 44 => Self::Woff,
            b"\x00\x01\x00\x00" | b"true" if bytes.len() >= 12 => return Some(Self::Ttf),
            b"OTTO" if bytes.len() >= 12 => return Some(Self::Otf),
            _ => return None,
        };
        (u32::from_be_bytes(bytes[8..12].try_into().ok()?) as usize == bytes.len())
            .then_some(format)
    }

    pub(super) fn mime(self) -> &'static str {
        match self {
            Self::Woff2 => "font/woff2",
            Self::Woff => "font/woff",
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
        }
    }

    fn css(self) -> &'static str {
        match self {
            Self::Woff2 => "woff2",
            Self::Woff => "woff",
            Self::Ttf => "truetype",
            Self::Otf => "opentype",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Face {
    pub digest: String,
    pub format: Format,
    pub first_weight: u16,
    pub last_weight: u16,
    pub italic: bool,
    pub unicode_range: Option<String>,
}

impl Face {
    pub(super) fn uploaded(filename: &str, format: Format, digest: String) -> Self {
        let lower = filename.to_ascii_lowercase();
        let name: String = lower.chars().filter(char::is_ascii_alphabetic).collect();
        let weights = [
            ("thin", 100),
            ("extralight", 200),
            ("ultralight", 200),
            ("semilight", 350),
            ("light", 300),
            ("medium", 500),
            ("semibold", 600),
            ("demibold", 600),
            ("extrabold", 800),
            ("ultrabold", 800),
            ("bold", 700),
            ("black", 900),
            ("heavy", 900),
        ];
        let weight = weights
            .into_iter()
            .find(|(key, _)| name.contains(key))
            .map_or(400, |(_, n)| n);
        Self {
            digest,
            format,
            first_weight: weight,
            last_weight: weight,
            italic: name.contains("italic") || name.contains("oblique"),
            unicode_range: None,
        }
    }

    pub(super) fn append_css(&self, css: &mut String, id: &str, alias: &str) {
        use std::fmt::Write as _;
        let style = if self.italic { "italic" } else { "normal" };
        let _ = write!(
            css,
            "@font-face{{font-family:\"{alias}\";font-display:swap;font-style:{style};font-weight:{}",
            self.first_weight
        );
        if self.last_weight != self.first_weight {
            let _ = write!(css, " {}", self.last_weight);
        }
        let _ = write!(
            css,
            ";src:url(\"/api/v1/fonts/face/{id}/{}\") format(\"{}\");",
            self.digest,
            self.format.css()
        );
        if let Some(range) = &self.unicode_range {
            let _ = write!(css, "unicode-range:{range};");
        }
        css.push_str("}\n");
    }
}

/// Upstream unicode-range permits hex intervals, singletons and trailing
/// wildcards. A strict grammar keeps arbitrary provider text out of our CSS.
pub(super) fn unicode_range(value: &str) -> Option<String> {
    fn part(s: &str) -> bool {
        if s.is_empty() || s.len() > 6 {
            return false;
        }
        let hex = s.trim_end_matches('?');
        hex.bytes().all(|b| b.is_ascii_hexdigit())
            && u32::from_str_radix(&s.replace('?', "F"), 16).is_ok_and(|v| v <= 0x10ffff)
    }
    if value.len() > 128 * 1024 {
        return None;
    }
    let valid = value.split(',').all(|r| {
        let Some(r) = r
            .trim()
            .strip_prefix("U+")
            .or_else(|| r.trim().strip_prefix("u+"))
        else {
            return false;
        };
        if let Some((a, b)) = r.split_once('-') {
            !r.contains('?')
                && part(a)
                && part(b)
                && u32::from_str_radix(a, 16).ok() <= u32::from_str_radix(b, 16).ok()
        } else {
            part(r)
        }
    });
    valid.then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_header_length_select_the_original_font_format() {
        for (tag, count, format, mime) in [
            (b"wOF2", 48, Format::Woff2, "font/woff2"),
            (b"wOFF", 44, Format::Woff, "font/woff"),
            (b"\x00\x01\x00\x00", 12, Format::Ttf, "font/ttf"),
            (b"OTTO", 12, Format::Otf, "font/otf"),
        ] {
            let mut bytes = vec![0; count];
            bytes[..4].copy_from_slice(tag);
            bytes[8..12].copy_from_slice(&(count as u32).to_be_bytes());
            assert_eq!(Format::identify(&bytes), Some(format));
            assert_eq!(format.mime(), mime);
            assert_eq!(Format::identify(&bytes[..11]), None);
        }
        assert_eq!(Format::identify(b"<html>not a font</html>"), None);
    }
}
