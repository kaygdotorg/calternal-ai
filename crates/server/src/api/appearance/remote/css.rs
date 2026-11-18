//! Parse the restricted font-face subset of the providers' CSS.
use super::{Asset, MAX_FACES, asset_url, specification, unavailable, weight};
use crate::api::appearance::face::unicode_range;
use calternal_types::Error;
use std::{collections::HashMap, sync::LazyLock};

pub(super) fn parse(css: &[u8], host: &str) -> Result<Vec<Asset>, Error> {
    static BLOCK: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"@font-face\s*\{([^{}]*)\}").unwrap());
    static URL: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"url\(\s*['"]?([^'")\s]+)['"]?\s*\)(\s*format\(\s*['"]?woff2['"]?\s*\))?"#,
        )
        .unwrap()
    });
    let mut faces = Vec::new();
    let text = std::str::from_utf8(css).map_err(|_| unavailable())?;
    for block in BLOCK.captures_iter(text) {
        let fields: HashMap<_, _> = block[1]
            .split(';')
            .filter_map(|field| field.split_once(':'))
            .map(|(key, value)| (key.trim(), value.trim()))
            .collect();
        let src = fields.get("src").ok_or_else(unavailable)?;
        let url = URL
            .captures_iter(src)
            .filter_map(|c| {
                let candidate = asset_url(&c[1], host)?;
                (c.get(2).is_some() || candidate.path().ends_with(".woff2")).then_some(candidate)
            })
            .next()
            .ok_or_else(unavailable)?;
        let mut weights = fields
            .get("font-weight")
            .copied()
            .unwrap_or("400")
            .split_ascii_whitespace();
        let first = weights
            .next()
            .and_then(|v| v.parse().ok())
            .filter(|w| weight(*w))
            .ok_or_else(unavailable)?;
        let last = weights
            .next()
            .map_or(Some(first), |v| v.parse::<u16>().ok())
            .filter(|w| weight(*w) && *w >= first)
            .ok_or_else(unavailable)?;
        if weights.next().is_some() {
            return Err(unavailable());
        }
        let italic = match fields.get("font-style").copied().unwrap_or("normal") {
            "normal" => false,
            "italic" | "oblique" => true,
            _ => return Err(unavailable()),
        };
        let range = fields
            .get("unicode-range")
            .map(|v| unicode_range(v).ok_or_else(unavailable))
            .transpose()?;
        faces.push(Asset {
            url,
            face: specification(first, last, italic, range),
        });
    }
    if faces.is_empty() || faces.len() > MAX_FACES {
        return Err(unavailable());
    }
    Ok(faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::appearance::remote::GOOGLE_BYTES;

    #[test]
    fn css_keeps_devanagari_and_latin_with_variable_weights() {
        let css = br"
            @font-face { font-weight:100 900;src:url(https://fonts.gstatic.com/dev.woff2) format('woff2');unicode-range:U+0900-097F; }
            @font-face { font-weight:100 900;src:url(https://fonts.gstatic.com/lat.woff2) format('woff2');unicode-range:U+0000-00FF; }
        ";
        let faces = parse(css, GOOGLE_BYTES).unwrap();
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].face.unicode_range.as_deref(), Some("U+0900-097F"));
        assert_eq!(
            (faces[0].face.first_weight, faces[0].face.last_weight),
            (100, 900)
        );
    }
}
