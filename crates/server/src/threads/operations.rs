//! Blocking thread file operations, all behind manifest policy and revision guards.
use super::*;
use calternal_content::Request;

/// Serve one authorized thread operation on a blocking worker. List reads the
/// bounded index. Get reads one document and its job, or one immutable checkpoint
/// range. Writes retain content's revision checks, receipts and publication.
/// Storage failures propagate.
pub(super) fn apply<D: Directory>(
    st: &Shared<D>,
    who: &Identity,
    space: SpaceId,
    operation: &str,
    request_id: &str,
    data: &JsonObject,
    id: Option<Uuid>,
    actor: &calternal_types::Actor,
) -> Result<Value, Error> {
    let content = st.store.content();
    if operation == "threads.get"
        && let Some(since) = data.get("since")
    {
        return super::delta::read(
            &content,
            space,
            id.ok_or_else(|| Error::invalid_input("id is required"))?,
            since,
        );
    }
    let fp = crate::api::fingerprint(operation, data);
    let request = Request {
        account: who.id,
        id: request_id,
        operation: operation,
        fingerprint: &fp,
    };
    let saved = if operation != "threads.get" && operation != "threads.list" {
        content.receipt(space, who.id, request_id)?
    } else {
        None
    };
    if let Some(saved) = saved {
        if saved.operation != operation || saved.fingerprint != fp {
            return Err(Error::request_reused());
        }
        if let Some(item) = saved.item {
            content.materialize(space, item)?;
        }
        if matches!(operation, "threads.delete" | "threads.archive") {
            return finish_lifecycle(
                st,
                space,
                id.ok_or_else(|| Error::not_found("item not found"))?,
                request,
                saved.result,
            );
        }
        return Ok(saved.result);
    }
    if operation == "threads.list" {
        let rows = super::index::page(st, space, who.id)?;
        let parent = data.get("parent_thread").and_then(Value::as_str);
        let include_archived = parent.is_some()
            || data
                .get("archived")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let limit = data.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
        let threads: Vec<_> = rows
            .into_iter()
            .filter(|r| {
                (parent.is_some() || r.metadata["deleted"] != true)
                    && (include_archived || r.metadata["archived"] != true)
                    && parent.is_none_or(|p| r.metadata["parent_thread"] == p)
            })
            .take(limit)
            .map(|r| json!({"id":r.id,"address":format!("/threads/{}",r.id),"revision":r.revision,"metadata":document::public_metadata(r.metadata),"turn_state":r.turn_state,"turn_reason":r.turn_reason}))
            .collect();
        return Ok(json!({"threads":threads}));
    }
    if operation == "threads.create" {
        for (kind, placement, format) in st
            .registry
            .get("calternal-orchestrator")
            .expect("checked plugin")
            .kind_registrations()
        {
            content.register_kind(space, &kind, placement, format)?;
        }
        if let Some(parent) = data.get("parent_thread").and_then(Value::as_str) {
            let parent = content.get(
                space,
                Uuid::parse_str(parent).map_err(|_| Error::not_found("item not found"))?,
            )?;
            if parent.kind != "threads" || parent.metadata["deleted"] == true {
                return Err(Error::not_found("item not found"));
            }
        }
        if data.contains_key("spawned_by") && !data.contains_key("parent_thread") {
            return Err(Error::invalid_input("spawned_by requires parent_thread"));
        }
        let now = calternal_content::now_rfc3339();
        let provider = data
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("calternal-claude-code");
        let mut metadata = json!({
            "title": data["title"],
            "provider": provider,
            "model": document::default_model(st, provider),
            "mode": data.get("mode").cloned().unwrap_or(json!(AgentMode::default())),
            "mode_for": who.id,
            "created_at": now,
            "updated_at": now,
            "last_activity_at": now,
            "message_count": 0,
            "archived": false,
            "deleted": false,
        });
        for key in ["parent_thread", "spawned_by"] {
            if let Some(v) = data.get(key) {
                metadata[key] = v.clone();
            }
        }
        document::validate_metadata(&metadata)?;
        let bytes = document::encode(&metadata, "");
        return content
            .write(
                space,
                None,
                "threads",
                None,
                metadata,
                bytes.as_bytes(),
                request,
                document::view,
            )
            .map(|item| document::view(&item));
    }
    let mut item = content.get(
        space,
        id.ok_or_else(|| Error::invalid_input("id is required"))?,
    )?;
    if item.kind != "threads" {
        return Err(Error::not_found("item not found"));
    }
    if operation == "threads.get" {
        let mut result = document::view(&item);
        result["thread"]["turn_state"] = json!("idle");
        result["thread"]["turn_reason"] = crate::turns::reason::describe(None, &Value::Null, false);
        if item.metadata["deleted"] != true
            && let Some(job) = st
                .jobs
                .thread_latest(&space.to_string(), &item.id.to_string())?
        {
            let reason = crate::turns::reason::describe(
                Some(job.state),
                &job.checkpoint,
                who.id == job.account,
            );
            result["thread"]["turn_state"] = json!(super::turn_status::label(job.state));
            result["thread"]["turn_reason"] = reason;
            if !job.state.is_final() {
                result["job"] = json!(job.view_for(who.id));
            }
        }
        return Ok(result);
    }
    if item.metadata["deleted"] == true {
        if operation == "threads.delete" {
            content.materialize(space, item.id)?;
            return finish_lifecycle(st, space, item.id, request, document::view(&item));
        }
        return Err(Error::not_found("item not found"));
    }
    if operation == "threads.send" {
        if item.metadata["archived"] == true {
            return Err(Error::conflict("this thread is archived"));
        }
        let latest = st
            .jobs
            .thread_latest(&space.to_string(), &item.id.to_string())?;
        if latest.as_ref().is_some_and(|j| !j.state.is_final()) {
            return Err(Error::conflict(
                "this thread already has active or interrupted work",
            ));
        }
        if item.metadata["pending"].is_object() {
            let pending = &item.metadata["pending"];
            let account = pending["account"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| Error::internal("invalid pending account"))?;
            let key = format!(
                "thread-admit/{}",
                pending["request_id"].as_str().unwrap_or("")
            );
            if content.receipt(space, account, &key)?.is_none()
                && calternal_types::now_ms()
                    .saturating_sub(pending["created_ms"].as_i64().unwrap_or(i64::MAX))
                    < content
                        .receipt_retention()
                        .as_millis()
                        .min(i64::MAX as u128) as i64
            {
                return Err(Error::conflict(
                    "retry the pending send with its original request_id before sending another message",
                ));
            }
        }
        let prompt = data["message"].as_str().unwrap_or("");
        if prompt.trim().is_empty() || prompt.len() > 16384 {
            return Err(Error::invalid_input(
                "message must contain 1 to 16384 UTF-8 bytes",
            ));
        }
        let mode = if let Some(mode) = data.get("mode") {
            serde_json::from_value::<AgentMode>(mode.clone())
                .map_err(|_| Error::invalid_input("invalid permission mode"))?
        } else if let Some(saved) =
            st.jobs
                .agent_saved_mode(&space.to_string(), &item.id.to_string(), who.id)?
        {
            saved
        } else if item.metadata["mode_for"]
            .as_str()
            .and_then(|id| calternal_types::AccountId::parse(id).ok())
            == Some(who.id)
        {
            serde_json::from_value::<AgentMode>(item.metadata["mode"].clone())
                .map_err(|_| Error::internal("invalid initial thread mode"))?
        } else {
            AgentMode::default()
        };
        item.metadata["mode"] = json!(mode);
        item.metadata["mode_for"] = json!(who.id);
        for key in ["provider"] {
            if let Some(v) = data.get(key) {
                item.metadata[key] = v.clone();
            }
        }
        document::validate_metadata(&item.metadata)?;
        let provider = item
            .metadata
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("");
        let model = data.get("model").and_then(Value::as_str).unwrap_or("");
        if model.len() > 128 {
            return Err(Error::invalid_input("model exceeds 128 UTF-8 bytes"));
        }
        item.metadata["model"] = if model.is_empty() {
            document::default_model(st, provider)
        } else {
            json!(model)
        };
        let now = calternal_content::now_rfc3339();
        document::record_message(&mut item.metadata, &now);
        item.metadata["pending"] = json!({"account":who.id,"request_id":request_id,"fingerprint":fp,"created_ms":calternal_types::now_ms()});
        let body = format!(
            "{}\n<!-- message:{} role:person actor:{} time:{now} -->\n{}\n<!-- /message -->\n",
            document::body(&item)?,
            Uuid::now_v7(),
            actor,
            prompt
        );
        let bytes = document::encode(&item.metadata, &body);
        return content
            .write(
                space,
                Some(item.id),
                "threads",
                Some(&item.revision),
                item.metadata.clone(),
                bytes.as_bytes(),
                request,
                document::view,
            )
            .map(|i| document::view(&i));
    }
    if operation == "threads.stop" {
        return content.admit_item(space, item.id, request, |current, saved| {
            if let Some(saved) = saved {
                return Ok(saved);
            }
            if current.metadata["deleted"] == true {
                return Err(Error::not_found("item not found"));
            }
            let mut result = document::view(current);
            if let Some(job) = st
                .jobs
                .thread_latest(&space.to_string(), &item.id.to_string())?
            {
                if job.account != who.id
                    && !st
                        .directory
                        .capabilities(who.id, space)
                        .is_some_and(|c| c.contains(calternal_types::Capability::PluginsManage))
                {
                    return Err(Error::not_found("item not found"));
                }
                st.jobs.cancel(&job.space, &job.id)?;
                st.jobs.agent_revoke(&job.space, &job.id)?;
                result["job"] = json!(
                    st.jobs
                        .get(&job.space, &job.id)?
                        .ok_or_else(|| Error::not_found("turn not found"))?
                        .view_for(who.id)
                );
            }
            Ok(result)
        });
    }
    match operation {
        "threads.rename" => item.metadata["title"] = data["title"].clone(),
        "threads.archive" => item.metadata["archived"] = json!(true),
        "threads.unarchive" => item.metadata["archived"] = json!(false),
        "threads.delete" => item.metadata["deleted"] = json!(true),
        _ => return Err(Error::not_found("unknown operation")),
    }
    document::validate_metadata(&item.metadata)?;
    item.metadata["updated_at"] = json!(calternal_content::now_rfc3339());
    let bytes = document::encode(&item.metadata, document::body(&item)?);
    if operation == "threads.delete" {
        let deleted = content.trash(
            &item,
            item.metadata.clone(),
            bytes.as_bytes(),
            request,
            document::view,
            || {
                if st
                    .jobs
                    .thread_latest(&space.to_string(), &item.id.to_string())?
                    .is_some_and(|j| !j.state.is_final())
                {
                    return Err(Error::conflict("stop this thread before deleting it"));
                }
                Ok(())
            },
        )?;
        finish_lifecycle(st, space, deleted.id, request, document::view(&deleted))
    } else {
        let archived = content.write(
            space,
            Some(item.id),
            "threads",
            Some(&item.revision),
            item.metadata.clone(),
            bytes.as_bytes(),
            request,
            document::view,
        )?;
        finish_lifecycle(st, space, archived.id, request, document::view(&archived))
    }
}

/// Cleanup follows the durable archive marker or tombstone. A failed content commit must never
/// forget a saved mode. Receipt replay and a fresh delete of an existing
/// tombstone both finish cleanup, even after the original receipt expires.
fn finish_lifecycle<D: Directory>(
    st: &Shared<D>,
    space: SpaceId,
    id: Uuid,
    request: Request<'_>,
    result: Value,
) -> Result<Value, Error> {
    let mut recipients = Vec::new();
    let result = st
        .store
        .content()
        .admit_item(space, id, request, |current, saved| {
            if current.kind != "threads" {
                return Err(Error::not_found("item not found"));
            }
            if current.metadata["deleted"] != true && current.metadata["archived"] != true {
                return saved
                    .ok_or_else(|| Error::conflict("thread changed before deletion cleanup"));
            }
            if current.metadata["archived"] == true
                && let Some(job) = st.jobs.thread_latest(&space.to_string(), &id.to_string())?
                && !job.state.is_final()
            {
                st.jobs.cancel(&job.space, &job.id)?;
            }
            recipients = st
                .jobs
                .agent_delete_thread(&space.to_string(), &id.to_string())?;
            Ok(saved.unwrap_or(result))
        })?;
    for account in recipients {
        st.inputs.invalidate(account)?;
    }
    Ok(result)
}
