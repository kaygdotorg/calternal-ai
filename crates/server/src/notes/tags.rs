//! Tag grammar ported from the reference app's notes engine.
mod front;
mod scan;

use std::collections::BTreeSet;

pub(super) fn collect(markdown: &str) -> Result<BTreeSet<&str>, calternal_types::Error> {
    let (head, body) = super::split_frontmatter(markdown);
    let tags = front::spans(head)
        .into_iter()
        .map(|span| &head[span])
        .chain(scan::spans(body).into_iter().map(|span| &body[span]));
    // Index the parent tags too, once per note at each depth.
    let mut names = BTreeSet::new();
    let mut bytes = 0;
    for tag in tags {
        for name in tag
            .match_indices('/')
            .map(|(end, _)| &tag[..end])
            .chain(std::iter::once(tag))
            .filter(|name| !name.is_empty())
        {
            if names.insert(name) {
                bytes += name.len() + 3;
                if bytes > calternal_content::MAX_METADATA_BYTES {
                    return Err(calternal_types::Error::new(
                        calternal_types::ErrorCode::TooLarge,
                        "note tag index is too large",
                    ));
                }
            }
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_tag_and_parent_once() {
        let text = "---\ntags: [work/q4, home]\n---\n# Plan\n#work/q4 #work #work/q3 `#hidden`";
        assert_eq!(
            collect(text).unwrap(),
            BTreeSet::from(["home", "work", "work/q3", "work/q4"])
        );
    }

    #[test]
    fn long_nested_tags_are_rejected_before_projection_can_grow_quadratically() {
        let text = format!("#a{}", "/a".repeat(10_000));
        assert!(collect(&text).is_err());
    }
}
