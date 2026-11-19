//! Native Notes mutations called after the operation gate.
use super::*;

pub(super) async fn mutate<D: Directory>(
    state: &Shared<D>,
    actor: calternal_types::Actor,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    use calternal_core::{NoteRenameInput, NoteTarget};
    let (fp, mut input) = super::request::fingerprint(op, input).await?;
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
        let request = calternal_content::Request { account: actor.account, via: actor.via.as_ref(), id: &rid, operation: &op, fingerprint: &fp };
        if op == "trash.purge" {
            return content.purge(space, "notes", id, request);
        }
        if let Some(v) = replay_receipt(s, actor.account, space, &rid, &op, &fp)? {
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
            let job = uuid::Uuid::now_v7();
            let result = |item: &calternal_content::Item| serde_json::json!({"note":crate::store::note(item),"job_id":job});
            let old_title = item.metadata["title"].as_str().unwrap_or("");
            let renamed = content.write_with_links(
                job, space, id, "notes", &i.base_revision,
                serde_json::json!({"title":i.title}), markdown.as_bytes(), request, result,
                |item, old, new| crate::notes::retarget(item, old, new, old_title, &i.title, id),
            )?;
            s.advance_note_rename_job(space, job);
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

/// Preview, confirm or inspect irreversible work after the native Notes gate.
pub(super) async fn empty_trash<D: Directory>(
    state: &Shared<D>,
    actor: calternal_types::Actor,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    use calternal_core::{NoteJobInput, TrashEmptyInput, TrashScope};
    match op {
        "trash.empty.preview" => {
            let i = decode::<TrashScope>(input)?;
            let space = space_of(&i.space_id)?;
            blocking(state, move |s| s.content().empty_trash_preview(space)).await
        }
        "trash.empty.status" => {
            let i = decode::<NoteJobInput>(input)?;
            let space = space_of(&i.space_id)?;
            let job = crate::store::item_id(&i.job_id)?;
            blocking(state, move |s| s.content().empty_trash_status(space, job)).await
        }
        _ => {
            let (fp, input) = super::request::fingerprint(op, input).await?;
            let i = decode::<TrashEmptyInput>(input)?;
            let space = space_of(&i.space_id)?;
            let (rid, op) = (rid.to_owned(), op.to_owned());
            blocking(state, move |s| {
                let request = calternal_content::Request {
                    account: actor.account,
                    via: actor.via.as_ref(),
                    id: &rid,
                    operation: &op,
                    fingerprint: &fp,
                };
                let result = s.content().empty_trash(space, &i.confirmation, request)?;
                s.notes_maintenance_wake.notify_one();
                Ok(result)
            })
            .await
        }
    }
}

/// Inspect or reverse a previously admitted rename after the space policy gate.
pub(super) async fn rename_job<D: Directory>(
    state: &Shared<D>,
    actor: calternal_types::Actor,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    if op == "notes.rename.status" {
        let i = decode::<calternal_core::NoteJobInput>(input)?;
        let space = space_of(&i.space_id)?;
        let job = crate::store::item_id(&i.job_id)?;
        return blocking(state, move |s| s.content().rename_job_status(space, job)).await;
    }
    let (fp, input) = super::request::fingerprint(op, input).await?;
    let i = decode::<calternal_core::NoteJobInput>(input)?;
    let space = space_of(&i.space_id)?;
    let job = crate::store::item_id(&i.job_id)?;
    let (rid, op) = (rid.to_owned(), op.to_owned());
    blocking(state, move |s| {
        let request = calternal_content::Request {
            account: actor.account,
            via: actor.via.as_ref(),
            id: &rid,
            operation: &op,
            fingerprint: &fp,
        };
        let result = s.content().undo_rename(space, job, request)?;
        s.notes_maintenance_wake.notify_one();
        Ok(result)
    })
    .await
}
