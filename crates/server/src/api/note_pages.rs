//! Native Notes, tag and Trash index pages after the operation gate.
use super::*;
use calternal_content::ItemOrder;
use calternal_core::{
    NoteMatch, NotePrefixInput, NotesHistoryInput, NotesHistoryResult, NotesListInput,
    NotesListResult, NotesLookupResult, SpaceDirectoryPageInput, TagCount, TagsResult,
};

const PAGE_SIZE: usize = 100;

pub(super) async fn discover<D: Directory>(
    state: &Shared<D>,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
        "trash.list" | "notes.list" => {
            let trashed = op == "trash.list";
            let (space, tag, cursor) = if trashed {
                let i: SpaceDirectoryPageInput = decode(input)?;
                (space_of(&i.space_id)?, None, i.cursor)
            } else {
                let i: NotesListInput = decode(input)?;
                (space_of(&i.space_id)?, i.tag, i.cursor)
            };
            let page = blocking(state, move |s| {
                let order = if trashed {
                    ItemOrder::Trash
                } else {
                    ItemOrder::Updated {
                        label: tag.as_deref(),
                    }
                };
                s.content()
                    .item_page(space, "notes", order, cursor.as_deref(), PAGE_SIZE)
            })
            .await?;
            to_value(NotesListResult {
                notes: page
                    .rows
                    .into_iter()
                    .map(crate::store::note_summary)
                    .collect(),
                next_cursor: page.next,
            })
        }
        "notes.lookup" => {
            let i: NotePrefixInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let prefix = crate::notes::title_key(&i.prefix);
            let page = blocking(state, move |s| {
                s.content().item_page(
                    space,
                    "notes",
                    ItemOrder::Title(&prefix),
                    i.cursor.as_deref(),
                    PAGE_SIZE,
                )
            })
            .await?;
            to_value(NotesLookupResult {
                notes: page
                    .rows
                    .into_iter()
                    .map(|i| NoteMatch {
                        link_target: crate::notes::link_target(&i.path),
                        note: crate::store::note_summary(i),
                    })
                    .collect(),
                next_cursor: page.next,
            })
        }
        "notes.history" => {
            let i: NotesHistoryInput = decode(input)?;
            let limit = i.limit.unwrap_or(20);
            let space = space_of(&i.space_id)?;
            let (versions, next_cursor) = blocking(&state, move |s| {
                s.history(space, &i.id, limit, i.cursor.as_deref())
            })
            .await?;
            to_value(NotesHistoryResult {
                versions,
                next_cursor,
            })
        }
        _ => {
            let (space, prefix, cursor) = if op == "tags.list" {
                let i: SpaceDirectoryPageInput = decode(input)?;
                (space_of(&i.space_id)?, String::new(), i.cursor)
            } else {
                let i: NotePrefixInput = decode(input)?;
                (space_of(&i.space_id)?, i.prefix, i.cursor)
            };
            let page = blocking(state, move |s| {
                s.content()
                    .label_page(space, "notes", &prefix, cursor.as_deref(), PAGE_SIZE)
            })
            .await?;
            to_value(TagsResult {
                tags: page
                    .rows
                    .into_iter()
                    .map(|(name, count)| TagCount { name, count })
                    .collect(),
                next_cursor: page.next,
            })
        }
    }
}
