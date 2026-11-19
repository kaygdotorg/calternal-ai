//! Byte ranges of a Markdown document's frontmatter.

/// The borrowed spans of a complete frontmatter block, including its delimiters.
pub struct Frontmatter<'a> {
    /// Opening delimiter and its line ending, including a leading BOM if present.
    pub opening: &'a str,
    /// Property lines with their original line endings.
    pub properties: &'a str,
    /// Closing delimiter and its line ending, when present.
    pub closing: &'a str,
    /// Text following the closing delimiter.
    pub body: &'a str,
}

/// Split a complete leading frontmatter block without normalizing any bytes.
/// Incomplete blocks and non-delimiters return None. Scans through the closing
/// delimiter, allocating nothing.
pub fn markdown_frontmatter(markdown: &str) -> Option<Frontmatter<'_>> {
    let end = markdown.find('\n')? + 1;
    let (opening, rest) = markdown.split_at(end);
    let marker = opening.strip_prefix('\u{feff}').unwrap_or(opening);
    if !matches!(marker, "---\n" | "---\r\n") {
        return None;
    }
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if matches!(line, "---\n" | "---\r\n" | "---") {
            return Some(Frontmatter {
                opening,
                properties: &rest[..offset],
                closing: line,
                body: &rest[offset + line.len()..],
            });
        }
        offset += line.len();
    }
    None
}
