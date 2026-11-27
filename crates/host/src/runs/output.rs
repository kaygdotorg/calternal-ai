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
            let kind = storage.kind(&row.head.kind)?;
            storage.linked_record_body(&row.head, &kind, &anchor)
        })
        .await?;
    command.insert("prompt".into(), Value::String(body));
    Ok(input)
}

pub(super) async fn monitor(
    host: Weak<Host>,
    run: Arc<Run>,
    handle: RunHandle,
    reply_call: Option<String>,
) {
    let exit = handle.wait_exit();
    monitor_stream(host, run, handle.events, exit, reply_call).await;
}

async fn monitor_stream(
    host: Weak<Host>,
    run: Arc<Run>,
    mut events: tokio::sync::mpsc::Receiver<RunEvent>,
    exit: impl Future<Output = ExitInfo>,
    reply_call: Option<String>,
) {
    let mut decoder = Decoder::new(reply_call.is_some());
    let mut sequence = 0;
    while let Some(event) = events.recv().await {
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
        sequence = event.sequence;
        run.publish(event);
    }
    drop(events);
    run.finish(RunEvent {
        sequence: sequence + 1,
        event: RunEventKind::Exit(exit.await),
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

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_types::{AccountId, SpaceId};
    use tokio::sync::{broadcast, mpsc, watch};
    use uuid::Uuid;

    #[tokio::test]
    async fn full_output_channel_preserves_the_watched_exit() {
        let grant = RunScopeGrant {
            id: Uuid::now_v7(),
            space: SpaceId::new(),
            item: Uuid::now_v7(),
        };
        let run = Arc::new(Run {
            grant,
            actor: Actor::account(AccountId::new()),
            target_kind: "test.kind".into(),
            cancelled: watch::channel(true).0,
            log: Default::default(),
            notify: broadcast::channel(1).0,
            stopped: Default::default(),
            sends: Default::default(),
        });
        let (sender, events) = mpsc::channel(1);
        sender
            .try_send(RunEvent {
                sequence: 1,
                event: RunEventKind::Data {
                    stream: Stream::Stdout,
                    bytes: vec![b'x'],
                },
            })
            .unwrap();
        let exit = ExitInfo {
            signal: Some(9),
            ..ExitInfo::default()
        };
        assert!(
            sender
                .try_send(RunEvent {
                    sequence: 2,
                    event: RunEventKind::Exit(exit.clone())
                })
                .is_err()
        );
        drop(sender);
        monitor_stream(
            Weak::new(),
            run.clone(),
            events,
            std::future::ready(exit),
            None,
        )
        .await;
        let batch = serde_json::to_value(run.log.after(grant.id, Some(0)).unwrap()).unwrap();
        assert_eq!(batch["offset"], 2);
        assert_eq!(batch["stopped"], true);
        assert_eq!(batch["chunks"][1]["signal"], 9);
    }
}
