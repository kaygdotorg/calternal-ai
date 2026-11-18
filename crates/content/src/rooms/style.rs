//! Preserve each file's BOM and dominant line ending while sharing LF text.
use std::borrow::Cow;

const BOM: char = '\u{feff}';

#[derive(Clone, Copy)]
pub(crate) struct Style {
    bom: bool,
    crlf: bool,
}

impl Style {
    pub(crate) fn split(text: &str) -> (Self, Cow<'_, str>) {
        let bom = text.starts_with(BOM);
        let text = text.strip_prefix(BOM).unwrap_or(text);
        let crlf = text.matches("\r\n").count();
        let style = Self {
            bom,
            crlf: crlf > text.matches('\n').count() / 2,
        };
        let body = if crlf == 0 {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(text.replace("\r\n", "\n"))
        };
        (style, body)
    }

    pub(crate) fn encode(self, text: &str) -> Vec<u8> {
        let mut out = String::with_capacity(text.len() + 3);
        if self.bom {
            out.push(BOM);
        }
        if self.crlf {
            out.push_str(&text.replace('\n', "\r\n"));
        } else {
            out.push_str(text);
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_bom_round_trip_after_sharing_lf_text() {
        let original = "\u{feff}one\r\ntwo\r\n";
        let (style, text) = Style::split(original);
        assert_eq!(text, "one\ntwo\n");
        assert_eq!(style.encode(&text), original.as_bytes());
        assert_eq!(
            style.encode("one!\ntwo\n"),
            "\u{feff}one!\r\ntwo\r\n".as_bytes()
        );
    }
}
