//! Native Notes handlers called after the operation gate.
use super::*;
use calternal_core::{
    NoteMatch, NotePrefixInput, NotesListInput, NotesListResult, NotesLookupResult, SpaceInput,
    TagCount, TagsResult,
};

pub(super) async fn discover<D: Directory>(
    state: &Shared<D>,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
        "trash.list" => {
            let i: SpaceInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let notes = blocking(state, move |s| {
                s.content()
                    .list_trash(space, "notes", 100)
                    .map(|rows| rows.into_iter().map(crate::store::note_summary).collect())
            })
            .await?;
            to_value(NotesListResult { notes })
        }
        "notes.list" => {
            let i: NotesListInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let notes = blocking(state, move |s| {
                if let Some(tag) = i.tag {
                    s.content()
                        .discover(space, "notes", None, Some(&tag))
                        .map(|rows| rows.into_iter().map(crate::store::note_summary).collect())
                } else {
                    s.list(space, 100)
                }
            })
            .await?;
            to_value(NotesListResult { notes })
        }
        "notes.lookup" => {
            let i: NotePrefixInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let prefix = crate::notes::title_key(&i.prefix);
            let notes = blocking(state, move |s| {
                s.content()
                    .discover(space, "notes", Some(&prefix), None)
                    .map(|rows| {
                        rows.into_iter()
                            .map(|i| NoteMatch {
                                link_target: crate::notes::link_target(&i.path),
                                note: crate::store::note_summary(i),
                            })
                            .collect()
                    })
            })
            .await?;
            to_value(NotesLookupResult { notes })
        }
        _ => {
            let (space, prefix) = if op == "tags.list" {
                let i: SpaceInput = decode(input)?;
                (space_of(&i.space_id)?, String::new())
            } else {
                let i: NotePrefixInput = decode(input)?;
                (space_of(&i.space_id)?, i.prefix)
            };
            let tags = blocking(state, move |s| s.content().labels(space, "notes", &prefix))
                .await?
                .into_iter()
                .map(|(name, count)| TagCount { name, count })
                .collect();
            to_value(TagsResult { tags })
        }
    }
}

pub(super) async fn mutate<D: Directory>(
    state: &Shared<D>,
    actor: Identity,
    rid: &str,
    op: &str,
    mut input: JsonObject,
) -> Result<Value, Error> {
    use calternal_core::{NoteRenameInput, NoteTarget};
    let fp = fingerprint(op, &input);
    let space = space_of(
        input
            .remove("space_id")
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            .unwrap_or(""),
    )?;
    let rename = if op == "notes.rename" {
        Some(decode::<NoteRenameInput>(input.clone())?)
    } else {
        None
    };
    let id = if let Some(i) = &rename {
        crate::store::item_id(&i.id)?
    } else {
        crate::store::item_id(&decode::<NoteTarget>(input)?.id)?
    };
    let (rid, op) = (rid.to_owned(), op.to_owned());
    blocking(state, move |s| {
        let content = s.content();
        let request = calternal_content::Request { account: actor.id, id: &rid, operation: &op, fingerprint: &fp };
        if op == "trash.purge" {
            return content.purge(space, "notes", id, request);
        }
        if let Some(v) = replay_receipt(s, actor.id, space, &rid, &op, &fp)? {
            return Ok(v);
        }
        let item = if op == "notes.restore" { content.get(space, id)? } else { content.get_live(space, id)? };
        if item.kind != "notes" {
            return Err(Error::not_found("item not found"));
        }
        if let Some(i) = rename {
            let markdown = std::str::from_utf8(&item.bytes).map_err(|_| Error::internal("note is not UTF-8"))?;
            let markdown = crate::notes::rename_heading(markdown, &i.title);
            calternal_core::validate_markdown(&markdown)?;
            let result = |item: &calternal_content::Item| serde_json::json!({"note":crate::store::note(item)});
            let old_title = item.metadata["title"].as_str().unwrap_or("");
            let renamed = content.write_with_links(
                space, id, "notes", &i.base_revision,
                serde_json::json!({"title":i.title}), markdown.as_bytes(), request, result,
                |item, old, new| crate::notes::retarget(item, old, new, old_title, &i.title, id),
            )?;
            return Ok(result(&renamed));
        }
        let response = |item: &calternal_content::Item| serde_json::json!({"note":{
            "id":item.id,"space_id":item.space,"title":item.metadata["title"],
            "revision":item.revision,"updated_at":item.updated_at
        }});
        let changed = if op == "notes.restore" {
            content.restore(&item, request, response)?
        } else {
            content.trash(&item, item.metadata.clone(), &item.bytes, request, response, || Ok(()))?
        };
        Ok(response(&changed))
    }).await
}
