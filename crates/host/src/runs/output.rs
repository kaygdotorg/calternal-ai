//! Translate installed harness output to calls while preserving the run grant.
mod decoder;

use self::decoder::{Action, Decoder};
use super::{Run, RunScopeGrant};
use crate::{Call, Host, worker_calls::Chain};
use calternal_machine::{
    protocol::{ExitInfo, Stream},
    run_service::{RunEvent, RunEventKind, RunHandle},
};
use calternal_plugin_host::operations::Code;
use calternal_types::{Actor, Error, MAX_FRAME_BYTES, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Weak},
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

pub(super) async fn monitor(
    host: Weak<Host>,
    run: Arc<Run>,
    mut handle: RunHandle,
    reply_call: Option<String>,
) {
    let mut decoder = Decoder::new(reply_call.is_some());
    let mut sequence = 0;
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
            && run.admit_callback().is_ok()
        {
            let bytes = bytes.clone();
            let decoded = tokio::task::spawn_blocking(move || {
                let actions = decoder.feed(&bytes);
                (decoder, actions)
            })
            .await;
            let Ok((next, actions)) = decoded else {
                run.cancelled.send_replace(true);
                break;
            };
            decoder = next;
            for action in actions {
                let Some(host) = host.upgrade() else { break };
                match action {
                    Action::Callback(request) => {
                        let id = request.id;
                        let outcome = dispatch(
                            &host,
                            &run,
                            request.call,
                            request.input,
                            format!("callback-{id}"),
                        )
                        .await;
                        if run.admit_callback().is_ok()
                            && let Ok(Some(bytes)) =
                                tokio::task::spawn_blocking(move || encode_callback(id, outcome))
                                    .await
                            && let Ok(service) = host.run_service()
                        {
                            let _ = service.send_input(run.grant.id, &run.actor, bytes).await;
                        }
                    }
                    Action::Reply { turn, text } => {
                        if let Some(call) = reply_call.as_deref() {
                            append_reply(&host, &run, call, turn, text).await;
                        }
                    }
                }
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

fn encode_callback(id: u64, result: Result<Value>) -> Option<Vec<u8>> {
    #[derive(Serialize)]
    struct Envelope {
        host_reply: Reply,
    }

    #[derive(Serialize)]
    #[serde(untagged)]
    enum Reply {
        Success { id: u64, result: Value },
        Failure { id: u64, error: Failure },
    }

    #[derive(Serialize)]
    struct Failure {
        code: Code,
    }

    let host_reply = match result {
        Ok(result) => Reply::Success { id, result },
        Err(error) => Reply::Failure {
            id,
            error: Failure {
                code: Code::from_error(&error),
            },
        },
    };
    let mut bytes =
        calternal_plugin_host::bounded_json(&Envelope { host_reply }, MAX_FRAME_BYTES).ok()?;
    bytes.push(b'\n');
    Some(bytes)
}

async fn append_reply(host: &Arc<Host>, run: &Run, call: &str, turn: u64, text: String) {
    let text: Arc<str> = text.into();
    let item = run.grant.item;
    let key = format!("reply-{turn}");
    // The slot owner may still be processing the original invocation.
    // Retry with the same key so the reply can be published only once.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(7);
    while run.admit_callback().is_ok() {
        let text = Arc::clone(&text);
        let Ok(input) =
            tokio::task::spawn_blocking(move || json!({"item":item,"text":&*text})).await
        else {
            break;
        };
        match dispatch(host, run, call.to_owned(), input, key.clone()).await {
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
    run.admit_callback()?;
    let dispatch = host.clone().dispatch(
        Actor::run(run.actor.account, plugin, id),
        space,
        Call {
            call,
            input,
            idempotency_key: Some(format!("run:{id}:{key}")),
            slot_selection: None,
        },
        chain,
    );
    tokio::select! {
        biased;
        _ = run.wait_cancelled() => Err(Error::not_found("Run has stopped.")),
        result = dispatch => result,
    }
}
