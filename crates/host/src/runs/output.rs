//! Translate installed harness output to calls while preserving the run grant.
use super::{Run, RunScopeGrant};
use crate::{Call, Host, decode, worker_calls::Chain};
use calternal_machine::{
    protocol::{ExitInfo, Stream},
    run_service::{RunEvent, RunEventKind, RunHandle},
};
use calternal_plugin_host::operations::Code;
use calternal_types::{Actor, Error, MAX_FRAME_BYTES, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Weak, atomic::Ordering},
    time::Duration,
};

pub(super) async fn resolve_prompt(host: &Arc<Host>, run: &Run, mut input: Value) -> Result<Value> {
    let Some(command) = input.get_mut("command").and_then(Value::as_object_mut) else {
        return Ok(input);
    };
    let Some(link) = command.get("prompt").and_then(Value::as_str) else {
        return Ok(input);
    };
    let Some(anchor) = link
        .strip_prefix("[Record](<")
        .and_then(|link| link.strip_suffix(">)"))
        .and_then(|link| link.rsplit_once("#^"))
        .map(|(_, anchor)| anchor)
    else {
        return Ok(input);
    };
    let item = run.grant.item;
    let anchor = anchor.to_owned();
    let actor = run.actor.clone();
    let body = host
        .run_storage(actor.clone(), run.grant.space, move |storage| {
            let row = storage.row(item)?;
            storage.permits(&actor, &row.head.kind, false)?;
            let records = storage.records(item)?;
            let (position, _) = records
                .iter()
                .enumerate()
                .find(|(_, record)| record.anchor == anchor)
                .ok_or_else(|| {
                    Error::forbidden("The prompt record is outside this run's session.")
                })?;
            let bytes = storage.read_verified(&row.head, &records)?;
            let kind = storage.kind(&row.head.kind)?;
            let views = storage.record_views(&row.head, &bytes, &records, &kind)?;
            views
                .get(position)
                .and_then(|view| view["body"].as_str())
                .map(str::to_owned)
                .ok_or_else(|| Error::forbidden("The prompt record is unavailable."))
        })
        .await?;
    command.insert("prompt".into(), json!(body));
    Ok(input)
}

#[derive(Deserialize)]
struct Callback {
    id: u64,
    call: String,
    input: Value,
}

pub(super) async fn monitor(host: Weak<Host>, run: Arc<Run>, mut handle: RunHandle, append: bool) {
    let mut pending = Vec::<u8>::new();
    let mut reply = String::new();
    let mut overflow = false;
    let mut turn = 0_u64;
    let mut sequence = 0;
    let mut callback = 0;
    while let Some(event) = handle.events.recv().await {
        sequence = event.sequence;
        if let RunEventKind::Exit(_) = event.event {
            run.finish(event);
            return;
        }
        if let RunEventKind::Data {
            stream: Stream::Stdout,
            bytes,
        } = &event.event
        {
            pending.extend(bytes);
            let mut consumed = 0;
            for line in pending.split_inclusive(|byte| *byte == b'\n') {
                if !line.ends_with(b"\n") {
                    break;
                }
                consumed += line.len();
                if line.len() > MAX_FRAME_BYTES {
                    overflow = true;
                    continue;
                }
                let Ok(frame) = serde_json::from_slice::<Value>(line) else {
                    continue;
                };
                if let Some(request) = frame.get("host_call") {
                    if let Ok(request) = decode::<Callback>(request.clone())
                        && request.id > callback
                        && let Some(host) = host.upgrade()
                    {
                        callback = request.id;
                        let outcome = dispatch(
                            &host,
                            &run,
                            request.call,
                            request.input,
                            format!("callback-{callback}"),
                        )
                        .await;
                        let response = match outcome {
                            Ok(result) => json!({"host_reply":{"id":callback,"result":result}}),
                            Err(error) => json!({"host_reply":{"id":callback,"error":{
                                "code": Code::from_error(&error)
                            }}}),
                        };
                        if let Ok(mut bytes) = serde_json::to_vec(&response) {
                            bytes.push(b'\n');
                            if let Ok(service) = host.run_service() {
                                let _ = service.send_input(run.grant.id, &run.actor, bytes).await;
                            }
                        }
                    }
                    continue;
                }
                if !append {
                    continue;
                }
                let event = &frame["event"];
                match event["type"].as_str() {
                    Some("text_delta") => {
                        if let Some(text) = event["text"].as_str() {
                            if reply.len() + text.len() <= MAX_FRAME_BYTES {
                                reply.push_str(text);
                            } else {
                                overflow = true;
                            }
                        }
                    }
                    Some("done") => {
                        turn += 1;
                        if event["outcome"] == "done"
                            && !reply.trim().is_empty()
                            && !overflow
                            && !run.stopped.load(Ordering::Acquire)
                            && let Some(host) = host.upgrade()
                        {
                            append_reply(&host, &run, turn, std::mem::take(&mut reply)).await;
                        }
                        reply.clear();
                        overflow = false;
                    }
                    _ => {}
                }
            }
            pending.drain(..consumed);
            if pending.len() > MAX_FRAME_BYTES {
                pending.clear();
                overflow = true;
            }
        }
        run.publish(event);
    }
    run.finish(RunEvent {
        sequence: sequence + 1,
        event: RunEventKind::Exit(ExitInfo {
            code: None,
            signal: None,
            error: None,
        }),
    });
}

async fn append_reply(host: &Arc<Host>, run: &Run, turn: u64, text: String) {
    let input = json!({"item":run.grant.item,"text":text});
    let key = format!("reply-{turn}");
    // Chat may still hold its single worker admission slot for the original
    // Send. Retry with the same append receipt key through that admission.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(7);
    loop {
        match dispatch(
            host,
            run,
            "calternal-chat.chats.send".into(),
            input.clone(),
            key.clone(),
        )
        .await
        {
            Err(error)
                if error.code == calternal_types::ErrorCode::RetryAfter
                    && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(error) => {
                eprintln!("calternal-host: run reply failed ({:?})", error.code);
                break;
            }
            Ok(_) => break,
        }
    }
}

async fn dispatch(
    host: &Arc<Host>,
    run: &Run,
    call: String,
    input: Value,
    key: String,
) -> Result<Value> {
    let RunScopeGrant { id, space, .. } = run.grant;
    let plugin = crate::identity(&run.actor).0;
    let mut chain = Chain::default();
    chain.run = Some(run.grant);
    host.clone()
        .dispatch(
            Actor::run(run.actor.account, plugin, id),
            space,
            Call {
                call,
                input,
                idempotency_key: Some(format!("run:{id}:{key}")),
                slot_selection: None,
            },
            chain,
        )
        .await
}
