//! Per-space live window subscriptions over one WebSocket per browser session.
//!
//! Each space has an independent bounded broadcast channel so writes and state
//! updates wake only sessions watching that space, with no global lock on the
//! request path. Subscriptions reuse the host's authorized `s.query` and
//! `s.read` paths so viewer-scoped formulas and access checks are never
//! duplicated.
mod limits;
mod run_logs;
mod subscribe;
mod wire;

use crate::{Call, Host, Session};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use calternal_types::{AccountId, Actor, Error, ErrorCode, SpaceId};
pub(crate) use limits::Accounts;
use limits::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;
use wire::{LiveSocket, send_error, send_frame};

/// Maximum bytes accepted in a single client WebSocket text frame.
pub const MAX_CLIENT_FRAME_BYTES: usize = 64 * 1024;

/// Maximum concurrent live subscriptions across one account's WebSockets.
pub const MAX_SUBSCRIPTIONS: usize = 24;

const SPACE_CHANNEL_CAPACITY: usize = 256;

/// Live notification channels for one space, separated by viewer. Channels
/// carry invalidations only. Every outgoing row and record is re-read with
/// the receiving account's authority before it enters a socket frame.
#[derive(Default)]
pub(crate) struct Hub {
    viewers: HashMap<AccountId, broadcast::Sender<()>>,
}

impl Hub {
    pub(crate) fn subscribe(&mut self, account: AccountId) -> broadcast::Receiver<()> {
        self.viewers
            .retain(|_, sender| sender.receiver_count() != 0);
        self.viewers
            .entry(account)
            .or_insert_with(|| broadcast::channel(SPACE_CHANNEL_CAPACITY).0)
            .subscribe()
    }

    pub(crate) fn has_subscribers(&self) -> bool {
        self.viewers
            .values()
            .any(|sender| sender.receiver_count() != 0)
    }

    // The space lock serializes this nonblocking publication with the commit.
    // A private invalidation is kept out of other accounts' lagged streams.
    pub(crate) fn notify(&mut self, viewer: Option<AccountId>) {
        self.viewers.retain(|account, sender| {
            if viewer.is_none_or(|viewer| viewer == *account) {
                let _ = sender.send(());
            }
            sender.receiver_count() != 0
        });
    }
}

pub(crate) async fn ws_route(
    State(host): State<Arc<Host>>,
    Path(space): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let Ok(space) = space.parse::<SpaceId>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(token) = crate::browser::cookie(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(session) = host.authenticate(token).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let account = session.account;
    let Ok(admission) = host.live_accounts.connect(account) else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let actor = Actor::account(account);
    let storage_host = host.clone();
    let viewer = actor.clone();
    let access = tokio::task::spawn_blocking(move || {
        storage_host.with_space_slot(&viewer, space, |storage, slot| {
            Ok((
                storage.live.subscribe(account),
                storage.runs.logs.subscribe(),
                slot.live_codecs.clone(),
            ))
        })
    })
    .await
    .map_err(crate::internal)
    .and_then(|access| access);
    let (receiver, run_logs, codecs) = match access {
        Ok(receiver) => receiver,
        Err(error) => {
            return StatusCode::from_u16(error.code.http_status())
                .expect("shared HTTP status")
                .into_response();
        }
    };
    ws.max_message_size(MAX_CLIENT_FRAME_BYTES)
        .max_frame_size(MAX_CLIENT_FRAME_BYTES)
        .max_write_buffer_size(wire::MAX_OUTGOING_BYTES + 128 * 1024)
        .on_upgrade(move |socket| {
            serve_socket(
                host, space, actor, session, admission, receiver, run_logs, codecs, socket,
            )
        })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientFrame {
    Subscribe {
        sub: String,
        query: Option<Value>,
        records: Option<RecordsRequest>,
        run_log: Option<run_logs::Request>,
    },
    Unsubscribe {
        sub: String,
    },
    Ping,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordsRequest {
    item: Uuid,
    after: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerFrame {
    Ready {
        account: String,
        space: String,
    },
    Rows {
        sub: String,
        rows: Value,
        totals: Value,
        #[serde(skip_serializing_if = "Value::is_null")]
        groups: Value,
        #[serde(rename = "namedTotals", skip_serializing_if = "Value::is_null")]
        named_totals: Value,
        cursor: Value,
    },
    Records {
        sub: String,
        item: Uuid,
        version: u64,
        #[serde(flatten)]
        metadata: RowMetadata,
        mode: &'static str,
        after: Option<String>,
        records: Vec<Value>,
        last_anchor: Option<String>,
    },
    RecordAppend {
        sub: String,
        item: Uuid,
        version: u64,
        record: Value,
    },
    ItemChange {
        sub: String,
        item: Uuid,
        version: u64,
        #[serde(flatten)]
        metadata: RowMetadata,
    },
    RunLog {
        sub: String,
        #[serde(flatten)]
        batch: crate::runs::LogBatch,
    },
    Error {
        sub: Option<String>,
        code: &'static str,
        message: String,
        next: &'static str,
    },
    Pong,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct RowMetadata {
    fields: Value,
    computed: Value,
    state: Value,
    when: Value,
}

enum Subscription {
    RunLog {
        run: Uuid,
        after: Option<u64>,
        initial: bool,
        finished: bool,
    },
    Query {
        input: Value,
        last_payload: Option<blake3::Hash>,
    },
    Records {
        item: Uuid,
        last_anchor: Option<String>,
        last_version: u64,
        last_count: usize,
        last_meta: Option<RowMetadata>,
    },
}

async fn serve_socket(
    host: Arc<Host>,
    space: SpaceId,
    actor: Actor,
    session: Session,
    mut admission: Connection,
    mut rx: broadcast::Receiver<()>,
    mut run_logs: broadcast::Receiver<()>,
    codecs: Arc<tokio::sync::Semaphore>,
    socket: WebSocket,
) {
    let mut socket = LiveSocket::new(socket, host.clone(), session, codecs);
    let mut authority = host.auth.spaces().changes();
    let mut expiry = tokio::time::interval(std::time::Duration::from_secs(60));
    if send_frame(
        &mut socket,
        ServerFrame::Ready {
            account: actor.account.to_string(),
            space: space.to_string(),
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let mut subs: HashMap<String, Subscription> = HashMap::new();
    let mut pending_rows = false;
    let mut pending_logs = false;
    let mut refresh = tokio::time::interval(std::time::Duration::from_millis(125));
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = authority.changed() => {
                if socket.check().await.is_err() {
                    break;
                }
            }
            _ = expiry.tick() => {
                if socket.check().await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else { break };
                if socket.check().await.is_err() {
                    break;
                }
                match message {
                    Message::Text(text) => {
                        if text.len() > MAX_CLIENT_FRAME_BYTES {
                            let _ = send_error(&mut socket, None, &Error::new(ErrorCode::TooLarge, "Frame exceeds 64 KiB.")).await;
                            break;
                        }
                        let frame: ClientFrame = match serde_json::from_str(&text) {
                            Ok(frame) => frame,
                            Err(_) => {
                                let _ = send_error(&mut socket, None, &Error::invalid_input("Expected a valid live protocol frame.")).await;
                                continue;
                            }
                        };
                        if subscribe::handle_client_frame(&host, &actor, space, frame, &mut subs, &mut admission, &mut socket).await.is_err() {
                            break;
                        }
                    }
                    Message::Ping(payload) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            event = run_logs.recv() => {
                if matches!(event, Err(broadcast::error::RecvError::Closed)) { break; }
                pending_logs = true;
            }
            event = rx.recv() => {
                if matches!(event, Err(broadcast::error::RecvError::Closed)) { break; }
                pending_rows = true;
            }
            _ = refresh.tick() => {
                if pending_rows {
                    let count = subs.values().filter(|sub| !matches!(sub, Subscription::RunLog { .. })).count();
                    if admission.refresh(count) {
                        pending_rows = false;
                        if refresh_all(&host, &actor, space, &mut subs, &mut socket).await.is_err() {
                            break;
                        }
                    }
                }
                if pending_logs {
                    let count = subs.values().filter(|sub| matches!(sub, Subscription::RunLog { finished: false, .. })).count();
                    if admission.refresh(count) {
                        pending_logs = false;
                        let mut revoked = Vec::new();
                        for (sub, subscription) in &mut subs {
                            if let Err(error) = run_logs::refresh(&host, &actor, space, sub, subscription, &mut socket).await {
                                if send_error(&mut socket, Some(sub.clone()), &error).await.is_err() { return; }
                                revoked.push(sub.clone());
                            }
                        }
                        for sub in revoked {
                            subs.remove(&sub);
                            admission.unsubscribe();
                        }
                    }
                }
            }
        }
    }
}

async fn refresh_all(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    subs: &mut HashMap<String, Subscription>,
    socket: &mut LiveSocket,
) -> std::result::Result<(), ()> {
    let keys: Vec<String> = subs.keys().cloned().collect();
    for key in keys {
        if let Some(sub) = subs.get_mut(&key)
            && let Err(error) = eval_subscription(host, actor, space, &key, sub, None, socket).await
        {
            send_error(socket, Some(key), &error).await?;
        }
    }
    Ok(())
}

async fn eval_subscription(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    sub_id: &str,
    sub: &mut Subscription,
    initial_after: Option<Option<String>>,
    socket: &mut LiveSocket,
) -> calternal_types::Result<()> {
    match sub {
        Subscription::RunLog { .. } => Ok(()),
        Subscription::Query {
            input,
            last_payload,
        } => {
            let mut value = host
                .clone()
                .trusted_call(
                    actor.clone(),
                    space,
                    Call {
                        call: "s.query".into(),
                        input: input.clone(),
                        idempotency_key: None,
                        slot_selection: None,
                    },
                )
                .await?;
            let frame = ServerFrame::Rows {
                sub: sub_id.to_owned(),
                rows: std::mem::take(&mut value["rows"]),
                totals: std::mem::take(&mut value["totals"]),
                groups: std::mem::take(&mut value["groups"]),
                named_totals: std::mem::take(&mut value["namedTotals"]),
                cursor: std::mem::take(&mut value["cursor"]),
            };
            wire::send_distinct(socket, frame, last_payload).await
        }
        Subscription::Records {
            item,
            last_anchor,
            last_version,
            last_count,
            last_meta,
        } => {
            let mut value = host
                .clone()
                .trusted_call(
                    actor.clone(),
                    space,
                    Call {
                        call: "s.read".into(),
                        input: json!({ "item": item }),
                        idempotency_key: None,
                        slot_selection: None,
                    },
                )
                .await?;
            let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
            let metadata = RowMetadata {
                fields: std::mem::take(&mut value["fields"]),
                computed: std::mem::take(&mut value["computed"]),
                state: std::mem::take(&mut value["state"]),
                when: std::mem::take(&mut value["when"]),
            };
            let Value::Array(records) = std::mem::take(&mut value["records"]) else {
                return Err(Error::internal("Missing record projection."));
            };
            let tail_anchor = records
                .last()
                .and_then(|r| r.get("anchor"))
                .and_then(Value::as_str)
                .map(str::to_owned);

            // Initial subscribe or reconnect with explicit `after` anchor.
            if let Some(after) = initial_after {
                let (mode, out, used_after) = slice_after(&records, after.as_deref());
                *last_anchor = tail_anchor.clone();
                *last_version = version;
                *last_count = records.len();
                *last_meta = Some(metadata.clone());
                let frame = ServerFrame::Records {
                    sub: sub_id.to_owned(),
                    item: *item,
                    version,
                    metadata,
                    mode,
                    after: used_after,
                    records: out,
                    last_anchor: tail_anchor,
                };
                send_frame(socket, frame).await?;
                return Ok(());
            }

            // Incremental update after a space event.
            let pure_append = records.len() > *last_count
                && version.checked_sub(*last_version) == Some((records.len() - *last_count) as u64)
                && match last_anchor.as_deref() {
                    None => *last_count == 0,
                    Some(prev) => {
                        records
                            .get(last_count.saturating_sub(1))
                            .and_then(|r| r.get("anchor"))
                            .and_then(Value::as_str)
                            == Some(prev)
                    }
                };

            if pure_append {
                let appended = &records[*last_count..];
                for record in appended {
                    send_frame(
                        socket,
                        ServerFrame::RecordAppend {
                            sub: sub_id.to_owned(),
                            item: *item,
                            version,
                            record: record.clone(),
                        },
                    )
                    .await?;
                }
                if last_meta.as_ref() != Some(&metadata) {
                    send_frame(
                        socket,
                        ServerFrame::ItemChange {
                            sub: sub_id.to_owned(),
                            item: *item,
                            version,
                            metadata: metadata.clone(),
                        },
                    )
                    .await?;
                }
                *last_anchor = tail_anchor;
                *last_version = version;
                *last_count = records.len();
                *last_meta = Some(metadata.clone());
                return Ok(());
            }

            if version != *last_version || records.len() != *last_count {
                // Record edit, redaction, or replacement: send a clean snapshot.
                let count = records.len();
                let frame = ServerFrame::Records {
                    sub: sub_id.to_owned(),
                    item: *item,
                    version,
                    metadata: metadata.clone(),
                    mode: "snapshot",
                    after: None,
                    records,
                    last_anchor: tail_anchor.clone(),
                };
                *last_anchor = tail_anchor;
                *last_version = version;
                *last_count = count;
                *last_meta = Some(metadata.clone());
                send_frame(socket, frame).await?;
            } else if last_meta.as_ref() != Some(&metadata) {
                *last_meta = Some(metadata.clone());
                send_frame(
                    socket,
                    ServerFrame::ItemChange {
                        sub: sub_id.to_owned(),
                        item: *item,
                        version,
                        metadata,
                    },
                )
                .await?;
            }
            Ok(())
        }
    }
}

/// Slice `records` strictly after `after` when that anchor is present in the
/// ordered list. Falls back to a full snapshot when `after` is absent or not
/// found (e.g. redacted).
pub(crate) fn slice_after(
    records: &[Value],
    after: Option<&str>,
) -> (&'static str, Vec<Value>, Option<String>) {
    let Some(anchor) = after else {
        return ("snapshot", records.to_vec(), None);
    };
    if let Some(pos) = records
        .iter()
        .position(|r| r.get("anchor").and_then(Value::as_str) == Some(anchor))
    {
        (
            "delta",
            records[pos + 1..].to_vec(),
            Some(anchor.to_owned()),
        )
    } else {
        ("snapshot", records.to_vec(), None)
    }
}

#[cfg(test)]
mod tests;
