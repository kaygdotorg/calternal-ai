//! Native Markdown projection for the official Notes kind.
mod links;
mod retention;
mod tags;

use serde_json::{Value, json};

/// Derive the list and picker metadata in one pass over the submitted Markdown.
pub(crate) fn metadata(title: &str, markdown: &str) -> Result<Value, calternal_types::Error> {
    Ok(json!({
        "title": title,
        "title_key": title_key(title),
        "labels": tags::collect(markdown)?,
        "links": links::targets(markdown),
    }))
}

/// Fold each title scalar independently so lookup prefixes are literal.
pub(crate) fn title_key(title: &str) -> String {
    title.chars().flat_map(char::to_lowercase).collect()
}

/// Look up a Notes address's lifecycle state after its owning-space check.
pub(crate) fn state(
    store: &crate::store::Store,
    resolved: &calternal_core::ResolveResult,
) -> Result<calternal_core::ItemState, calternal_types::Error> {
    use calternal_core::ItemState;
    let space = calternal_types::SpaceId::parse(&resolved.space_id)?;
    let id = crate::store::item_id(&resolved.id)?;
    Ok(match store.content().item_state(space, id)? {
        "in_trash" => ItemState::InTrash,
        "deleted" => ItemState::Deleted,
        _ => ItemState::Active,
    })
}

/// Keep the properties intact and find the first H1 outside code fences.
/// Returns the heading title byte range and the offset after frontmatter.
fn heading(markdown: &str) -> (Option<std::ops::Range<usize>>, usize) {
    let body = split_frontmatter(markdown).1;
    let start = markdown.len() - body.len();
    let mut at = start;
    let mut fence = None;
    for line in body.split_inclusive('\n') {
        let clean = line.trim_start_matches(' ');
        let marker = clean
            .as_bytes()
            .first()
            .copied()
            .filter(|c| matches!(c, b'`' | b'~'));
        let run = marker.map_or(0, |c| clean.bytes().take_while(|b| *b == c).count());
        if run >= 3 && line.len() - clean.len() <= 3 {
            match fence {
                None => fence = Some((marker, run)),
                Some((c, len)) if marker == c && run >= len && clean[run..].trim().is_empty() => {
                    fence = None;
                }
                _ => {}
            }
        } else if fence.is_none()
            && let Some(after) = line.strip_prefix("# ")
        {
            return (
                Some(at + 2..at + 2 + after.trim_end_matches(['\r', '\n']).len()),
                start,
            );
        }
        at += line.len();
    }
    (None, start)
}

/// Replace or insert the heading used by the reference app to name the file.
pub(crate) fn rename_heading(markdown: &str, title: &str) -> String {
    let (span, start) = heading(markdown);
    let mut next = markdown.to_owned();
    if let Some(span) = span {
        next.replace_range(span, title);
    } else {
        next.insert_str(start, &format!("# {title}\n"));
    }
    next
}

/// Read the first Heading 1 title as the source of a note's readable filename.
pub(crate) fn heading_title(markdown: &str) -> Option<&str> {
    let title = markdown[heading(markdown).0?].trim();
    (!title.is_empty()).then_some(title)
}

fn split_frontmatter(markdown: &str) -> (&str, &str) {
    markdown
        .strip_prefix("---\n")
        .and_then(|text| text.split_once("\n---\n"))
        .unwrap_or(("", markdown))
}

/// Repoint one locked snapshot and project the final Markdown once. The root
/// always needs its projection, while an unchanged linking note can be skipped.
pub(crate) fn retarget(
    item: &calternal_content::Item,
    old_path: &str,
    new_path: &str,
    old_title: &str,
    title: &str,
    root: uuid::Uuid,
) -> Result<Option<(Value, Vec<u8>)>, calternal_types::Error> {
    let text = std::str::from_utf8(&item.bytes)
        .map_err(|_| calternal_types::Error::internal("note is not UTF-8"))?;
    let next = links::retarget(text, old_path, new_path, old_title, title);
    if next.is_none() && root != item.id {
        return Ok(None);
    }
    let next = next.as_deref().unwrap_or(text);
    calternal_core::validate_markdown(next)?;
    let metadata = metadata(item.metadata["title"].as_str().unwrap_or(""), next)?;
    Ok(Some((metadata, next.as_bytes().to_vec())))
}

/// Build the relative destination a note picker inserts inside angle brackets.
pub(crate) fn link_target(path: &str) -> String {
    links::encode_destination(path.strip_prefix("Notes/").unwrap_or(path))
}

/// Actor-attributed journal event emitted by the scheduled Trash sweep.
pub(crate) const TRASH_EXPIRY_EVENT: &str = "trash.expire";

/// Identify item-bearing Notes journal entries, including operations in Trash.
pub(crate) fn journal_kind(event: &str) -> Option<&'static str> {
    (event == TRASH_EXPIRY_EVENT || calternal_core::note_operation(event).is_some_and(|op| op.item))
        .then_some("notes")
}
