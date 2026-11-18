//! Remote media allow-list. Verify the declared type against the binary prefix
//! and serve a canonical type. The host never decodes the payload.

use reqwest::header::{CONTENT_TYPE, HeaderMap};

/// Return a canonical serve type when exactly one supported content type
/// matches the asset's bytes. Cost is constant and allocates no buffers.
pub(super) fn checked_type(headers: &HeaderMap, bytes: &[u8]) -> Option<&'static str> {
    let mut fields = headers.get_all(CONTENT_TYPE).iter();
    let declared = fields.next()?.to_str().ok()?.split(';').next()?.trim();
    if fields.next().is_some() {
        return None;
    }
    let type_and_signature: &[(&str, &[u8])] = &[
        ("image/png", b"\x89PNG\r\n\x1a\n"),
        ("image/jpeg", b"\xff\xd8\xff"),
        ("image/gif", b"GIF87a"),
        ("image/gif", b"GIF89a"),
        ("font/woff", b"wOFF"),
        ("font/woff2", b"wOF2"),
        ("font/ttf", b"\x00\x01\x00\x00"),
        ("font/otf", b"OTTO"),
    ];
    if let Some((canonical, _)) = type_and_signature.iter().find(|(kind, signature)| {
        declared.eq_ignore_ascii_case(kind) && bytes.starts_with(signature)
    }) {
        return Some(*canonical);
    }
    if declared.eq_ignore_ascii_case("image/webp")
        && bytes.starts_with(b"RIFF")
        && bytes.get(8..12) == Some(b"WEBP")
    {
        return Some("image/webp");
    }
    if declared.eq_ignore_ascii_case("image/avif")
        && matches!(bytes.get(4..12), Some(b"ftypavif" | b"ftypavis"))
    {
        return Some("image/avif");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn rejects_svg_mislabeled_markup_and_ambiguous_content_types() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
        assert_eq!(checked_type(&headers, b"<svg/>"), None);
        assert_eq!(
            checked_type(&headers, b"\x89PNG\r\n\x1a\n"),
            Some("image/png")
        );
        headers.append(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
        assert_eq!(checked_type(&headers, b"\x89PNG\r\n\x1a\n"), None);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
        assert_eq!(checked_type(&headers, b"<svg/>"), None);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("font/woff2"));
        assert_eq!(checked_type(&headers, b"wOF2"), Some("font/woff2"));
    }
}
