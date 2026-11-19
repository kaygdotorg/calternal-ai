//! Parse untrusted HTML and emit only host-controlled markup and references.

use reqwest::Url;
use scraper::{ElementRef, Node, node::Element};

mod css;

const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_URL_BYTES: usize = 2048;
const MAX_REFERENCES: usize = 64;
const MAX_DEPTH: usize = 64;

/// Host-generated markup plus the upstream targets referenced by its indices.
/// Only `html` may be rendered inside the sealed frame.
pub(super) struct CleanContent {
    pub(super) html: String,
    pub(super) assets: Vec<Url>,
    pub(super) links: Vec<Url>,
}

/// Parse HTML once, then write out only the host's selected tags and attributes.
/// Time and memory are bounded by the input and output caps; no I/O.
pub(super) fn clean(html: &str, proxy: &impl Fn(usize) -> String) -> Option<CleanContent> {
    if html.len() > MAX_HTML_BYTES {
        return None;
    }
    let document = scraper::Html::parse_fragment(html);
    let mut output = CleanContent {
        html: String::new(),
        assets: Vec::new(),
        links: Vec::new(),
    };
    children(document.root_element(), 0, proxy, &mut output)?;
    Some(output)
}

fn children(
    element: ElementRef<'_>,
    depth: usize,
    proxy: &impl Fn(usize) -> String,
    output: &mut CleanContent,
) -> Option<()> {
    if depth == MAX_DEPTH {
        return None;
    }
    for child in element.children() {
        match child.value() {
            Node::Text(text) => escape(&mut output.html, text),
            Node::Element(_) => render(ElementRef::wrap(child)?, depth, proxy, output)?,
            _ => (),
        }
        if output.html.len() > MAX_HTML_BYTES {
            return None;
        }
    }
    Some(())
}

fn render(
    element: ElementRef<'_>,
    depth: usize,
    proxy: &impl Fn(usize) -> String,
    output: &mut CleanContent,
) -> Option<()> {
    let node = element.value();
    if &*node.name.ns != "http://www.w3.org/1999/xhtml" {
        return Some(());
    }
    let (tag, void) = match node.name() {
        "a" | "b" | "blockquote" | "caption" | "code" | "dd" | "del" | "div" | "dl" | "dt"
        | "em" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "i" | "li" | "ol" | "p" | "pre"
        | "s" | "small" | "span" | "strong" | "sub" | "sup" | "table" | "tbody" | "td"
        | "tfoot" | "th" | "thead" | "tr" | "u" | "ul" => (node.name(), false),
        "br" | "hr" | "img" => (node.name(), true),
        "style" => {
            let source = element.text().collect::<String>();
            let rewritten = css::stylesheet(&source, |url| asset(&mut output.assets, proxy, url));
            if !rewritten.is_empty() {
                output.html.push_str("<style>");
                output.html.push_str(&rewritten);
                output.html.push_str("</style>");
            }
            return Some(());
        }
        "iframe" | "noscript" | "object" | "script" | "template" => return Some(()),
        _ => return children(element, depth + 1, proxy, output),
    };
    output.html.push('<');
    output.html.push_str(tag);
    if let Some(title) = node.attr("title") {
        attribute(&mut output.html, "title", title);
    }
    if !tracking_image(node)
        && let Some(style) = node.attr("style")
    {
        let rewritten = css::inline(style, |url| asset(&mut output.assets, proxy, url));
        if !rewritten.is_empty() {
            attribute(&mut output.html, "style", &rewritten);
        }
    }
    match tag {
        "img" => image(node, proxy, output),
        "a" => {
            let link = node
                .attr("href")
                .and_then(|url| push_ref(&mut output.links, link_url(url)?));
            if let Some(index) = link {
                attribute(&mut output.html, "data-external-link", &index.to_string());
            }
        }
        _ => (),
    }
    output.html.push('>');
    if !void {
        children(element, depth + 1, proxy, output)?;
        output.html.push_str("</");
        output.html.push_str(tag);
        output.html.push('>');
    }
    Some(())
}

fn image(node: &Element, proxy: &impl Fn(usize) -> String, output: &mut CleanContent) {
    if let Some(alt) = node.attr("alt") {
        attribute(&mut output.html, "alt", alt);
    }
    if tracking_image(node) {
        return;
    }
    if let Some(target) = node
        .attr("src")
        .and_then(|url| asset(&mut output.assets, proxy, url))
    {
        attribute(&mut output.html, "src", &target);
    }
    if let Some(targets) = node
        .attr("srcset")
        .and_then(|text| srcset(text, proxy, &mut output.assets))
    {
        attribute(&mut output.html, "srcset", &targets);
    }
}

fn tracking_image(node: &Element) -> bool {
    node.name() == "img"
        && ["width", "height"].iter().all(|key| {
            node.attr(key)
                .and_then(|value| value.trim().parse::<u16>().ok())
                .is_some_and(|value| value <= 2)
        })
}

fn srcset(text: &str, proxy: &impl Fn(usize) -> String, assets: &mut Vec<Url>) -> Option<String> {
    if text.len() > MAX_SRCSET_BYTES {
        return None;
    }
    // A conservative srcset subset: comma-separated HTTPS URLs with either
    // no descriptor or one positive integer density/width. Ambiguous lists
    // (including comma-bearing URLs) leave no remote references.
    let mut candidates = Vec::new();
    for candidate in text.split(',') {
        if candidates.len() == 8 {
            return None;
        }
        let mut fields = candidate.split_ascii_whitespace();
        let url = asset_url(fields.next()?)?;
        let descriptor = fields.next().unwrap_or("");
        if fields.next().is_some()
            || !descriptor.is_empty()
                && !descriptor.strip_suffix(['x', 'w']).is_some_and(|value| {
                    !value.is_empty()
                        && value.bytes().all(|b| b.is_ascii_digit())
                        && value.parse::<u32>().is_ok_and(|number| number > 0)
                })
        {
            return None;
        }
        candidates.push((url, descriptor));
    }
    let mut output = Vec::with_capacity(candidates.len());
    for (url, descriptor) in candidates {
        let Some(index) = push_ref(assets, url) else {
            break;
        };
        let target = proxy(index);
        output.push(if descriptor.is_empty() {
            target
        } else {
            format!("{target} {descriptor}")
        });
    }
    (!output.is_empty()).then(|| output.join(", "))
}

const MAX_SRCSET_BYTES: usize = 16 * 1024;

fn asset(list: &mut Vec<Url>, proxy: &impl Fn(usize) -> String, text: &str) -> Option<String> {
    push_ref(list, asset_url(text)?).map(|index| proxy(index))
}

// Destination policy is independently enforced on every dial by the guard.
// These shape checks only decide which references the sanitizer retains.
fn asset_url(text: &str) -> Option<Url> {
    let mut url = link_url(text)?;
    if url.scheme() != "https" || url.port().is_some() || url.domain().is_none() {
        return None;
    }
    url.set_fragment(None);
    Some(url)
}

fn link_url(text: &str) -> Option<Url> {
    if text.len() > MAX_URL_BYTES || text.bytes().any(|byte| byte.is_ascii_control()) {
        return None;
    }
    let url = Url::parse(text).ok()?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.as_str().len() > MAX_URL_BYTES
    {
        return None;
    }
    Some(url)
}

fn push_ref(list: &mut Vec<Url>, url: Url) -> Option<usize> {
    if let Some(index) = list.iter().position(|value| *value == url) {
        return Some(index);
    }
    let index = list.len();
    if index == MAX_REFERENCES {
        return None;
    }
    list.push(url);
    Some(index)
}

fn attribute(html: &mut String, name: &str, value: &str) {
    html.push(' ');
    html.push_str(name);
    html.push_str("=\"");
    escape(html, value);
    html.push('"');
}

fn escape(html: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => html.push_str("&amp;"),
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '"' => html.push_str("&quot;"),
            c => html.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROXY: &str = "/api/v1/plugin-content/fixture/opaque";

    #[test]
    fn emits_only_opaque_media_references_and_text_from_stored_html() {
        let result = clean(
            r#"<base href="https://hidden.invalid"><meta http-equiv="refresh" content="0;url=https://hidden.invalid">
             <p onclick="evil()">A &lt; B<img src="https://example.com/a.png?x=1&amp;y=2" alt="A &amp; B"
               srcset="https://hidden.invalid/tracker 2x 2w"><a href="https://example.com/page">read</a></p>
             <svg><image href="https://hidden.invalid/inline.svg"></image></svg>
             <script>evil()</script><iframe src="https://hidden.invalid/frame"></iframe>"#,
            &|index| format!("{PROXY}/{index}"),
        )
        .unwrap();
        assert_eq!(
            result.html.trim(),
            r#"<p>A &lt; B<img alt="A &amp; B" src="/api/v1/plugin-content/fixture/opaque/0"><a data-external-link="0">read</a></p>"#
        );
        assert_eq!(
            result.assets[0].as_str(),
            "https://example.com/a.png?x=1&y=2"
        );
        assert_eq!(result.links[0].as_str(), "https://example.com/page");
    }

    #[test]
    fn rebuilds_supported_styles_using_only_local_proxy_urls() {
        let result = clean(
            r#"<style>
                @import url("https://example.com/tracker.css");
                @font-face { font-family: Letter; src: url(https://example.com/letter.woff2) format("woff2"); }
                a { background-image: url("https://example.com/dropped.png"); }
            </style><p style="font-family: Letter; cursor: url(https://example.com/cursor.png);
                           background-image: url(https://example.com/background.png)">Letter</p>"#,
            &|index| format!("{PROXY}/{index}"),
        ).unwrap();
        assert_eq!(result.assets.len(), 2);
        assert!(result.html.contains("@font-face{font-family:"));
        assert!(result.html.contains("background-image:url("));
        assert!(!result.html.contains("https://"));
        assert!(!result.html.contains("@import"));
        assert!(!result.html.contains("cursor"));
    }

    #[test]
    fn rewrites_a_bounded_srcset_and_drops_malformed_candidates() {
        let result = clean(
            r#"<img srcset="https://example.com/one 1x, https://example.com/two 2x">
               <img srcset="https://example.com/unexpected 2w 2x">"#,
            &|index| format!("{PROXY}/{index}"),
        )
        .unwrap();
        assert_eq!(result.assets.len(), 2);
        assert_eq!(result.html.matches("srcset=").count(), 1);
        assert!(result.html.contains("/0 1x,"));
        assert!(result.html.contains("/1 2x"));
    }

    #[test]
    fn reuses_reference_slots_and_bounds_reference_count() {
        let images = (0..MAX_REFERENCES + 2)
            .map(|i| {
                format!(r#"<img src="https://example.com/{i}"><img src="https://example.com/{i}">"#)
            })
            .collect::<String>();
        let result = clean(&images, &|index| format!("{PROXY}/{index}")).unwrap();
        assert_eq!(result.assets.len(), MAX_REFERENCES);
        assert_eq!(result.html.matches(" src=").count(), 2 * MAX_REFERENCES);
    }
}
