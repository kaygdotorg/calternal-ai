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
        let style = Self::detect(text, bom, crlf);
        let body = if crlf == 0 {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(text.replace("\r\n", "\n"))
        };
        (style, body)
    }

    fn detect(text: &str, bom: bool, crlf: usize) -> Self {
        Self {
            bom,
            crlf: crlf > text.matches('\n').count() / 2,
        }
    }

    // Non-live editors can send LF text without the original BOM. Explicit
    // CRLF sequences are already part of the submitted Markdown.
    pub(crate) fn restore(original: &str, replacement: &str) -> Option<Vec<u8>> {
        let original_bom = original.starts_with(BOM);
        let replacement_bom = replacement.starts_with(BOM);
        let original_crlf = original.matches("\r\n").count();
        let style = Self::detect(original, original_bom, original_crlf);
        let text = replacement.strip_prefix(BOM).unwrap_or(replacement);
        let restore_crlf = style.crlf && text.contains('\n') && !text.contains("\r\n");
        let restore_bom = style.bom && !replacement_bom;
        (restore_crlf || restore_bom).then(|| {
            Self {
                bom: style.bom || replacement_bom,
                crlf: restore_crlf,
            }
            .encode(text)
        })
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

#[cfg(test)]
#[test]
fn mixed_endings_follow_the_reference_majority_policy_after_edit() {
    for (before, expected) in [
        ("a\r\nb\r\nc\nd", "a!\r\nb\r\nc\r\nd"),
        ("a\nb\r\nc\nd", "a!\nb\nc\nd"),
        ("a\r\nb\nc", "a!\nb\nc"),
    ] {
        let (style, text) = Style::split(before);
        assert_eq!(
            style.encode(&text.replacen('a', "a!", 1)),
            expected.as_bytes()
        );
    }
}
