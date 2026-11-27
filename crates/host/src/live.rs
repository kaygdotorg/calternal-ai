//! Per-space live window subscriptions over one WebSocket per browser session.
//!
//! Each space has an independent bounded broadcast channel so writes and state
//! updates wake only sessions watching that space, with no global lock on the
//! request path. Subscriptions reuse the host's authorized `s.query` and
//! `s.read` paths so viewer-scoped formulas and access checks are never
//! duplicated.
use crate::{Call, Host};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use calternal_types::{AccountId, Actor, Error, ErrorCode, SpaceId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Maximum bytes accepted in a single client WebSocket text frame.
pub const MAX_CLIENT_FRAME_BYTES: usize = 64 * 1024;

/// Maximum concurrent live subscriptions allowed on one WebSocket session.
pub const MAX_SUBSCRIPTIONS: usize = 64;

const SPACE_CHANNEL_CAPACITY: usize = 256;

/// Per-space live notification channels, separated by viewer. Channels carry
/// invalidations only. Every outgoing row and record is re-read with the
/// receiving account's current authority before it enters a socket frame.
#[derive(Default)]
pub struct Hub {
    spaces: Mutex<HashMap<SpaceId, HashMap<AccountId, broadcast::Sender<()>>>>,
}

impl Hub {
    /// Subscribe one account to a space's invalidations. The caller verifies
    /// space access before registering. Holds the map lock for memory work only.
    pub fn subscribe(&self, space: SpaceId, account: AccountId) -> broadcast::Receiver<()> {
        let mut spaces = self.spaces.lock().expect("live hub lock");
        let viewers = spaces.entry(space).or_default();
        viewers.retain(|_, sender| sender.receiver_count() != 0);
        viewers
            .entry(account)
            .or_insert_with(|| broadcast::channel(SPACE_CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Publish a committed change. A personal state invalidation is queued
    /// only for that person, including when other sessions fall behind.
    pub fn notify(&self, space: SpaceId, viewer: Option<AccountId>) {
        let mut spaces = self.spaces.lock().expect("live hub lock");
        if let Some(viewers) = spaces.get_mut(&space) {
            viewers.retain(|account, sender| {
                if viewer.is_none_or(|viewer| viewer == *account) {
                    let _ = sender.send(());
                }
                sender.receiver_count() != 0
            });
            if viewers.is_empty() {
                spaces.remove(&space);
            }
        }
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
    let Some(token) = crate::browser::cookie(&headers).map(str::to_owned) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(account) = host.identify_session(&token).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let actor = Actor::account(account);
    let check = host
        .clone()
        .trusted_call(
            actor.clone(),
            space,
            Call {
                call: "s.space".into(),
                input: json!({}),
                idempotency_key: None,
                slot_selection: None,
            },
        )
        .await;
    if let Err(error) = check {
        return StatusCode::from_u16(error.code.http_status())
            .expect("shared HTTP status")
            .into_response();
    }
    ws.max_message_size(MAX_CLIENT_FRAME_BYTES)
        .on_upgrade(move |socket| serve_socket(host, space, actor, token, socket))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientFrame {
    Subscribe {
        sub: String,
        query: Option<Value>,
        records: Option<RecordsRequest>,
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
    Query {
        input: Value,
        last_payload: Option<String>,
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
    token: String,
    mut socket: WebSocket,
) {
    let mut authority = host.auth.spaces().changes();
    let mut expiry = tokio::time::interval(std::time::Duration::from_secs(60));
    if send_frame(
        &mut socket,
        &ServerFrame::Ready {
            account: actor.account.to_string(),
            space: space.to_string(),
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let mut rx = host.live().subscribe(space, actor.account);
    let mut subs: HashMap<String, Subscription> = HashMap::new();

    loop {
        tokio::select! {
            _ = authority.changed() => {
                if host.auth.check_session(&token).await.map(|session| session.0) != Some(actor.account) {
                    break;
                }
            }
            _ = expiry.tick() => {
                if host.auth.check_session(&token).await.map(|session| session.0) != Some(actor.account) {
                    break;
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else { break };
                if host.identify_session(&token).await != Some(actor.account) {
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
                        if handle_client_frame(&host, &actor, space, frame, &mut subs, &mut socket).await.is_err() {
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
            event = rx.recv() => {
                if host.auth.check_session(&token).await.map(|session| session.0) != Some(actor.account) {
                    break;
                }
                match event {
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if refresh_all(&host, &actor, space, &mut subs, &mut socket).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn handle_client_frame(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    frame: ClientFrame,
    subs: &mut HashMap<String, Subscription>,
    socket: &mut WebSocket,
) -> std::result::Result<(), ()> {
    match frame {
        ClientFrame::Ping => send_frame(socket, &ServerFrame::Pong).await,
        ClientFrame::Unsubscribe { sub } => {
            subs.remove(&sub);
            Ok(())
        }
        ClientFrame::Subscribe {
            sub,
            query,
            records,
        } => {
            if sub.is_empty() || sub.len() > 128 {
                return send_error(
                    socket,
                    Some(sub),
                    &Error::invalid_input("Subscription ID must be 1 to 128 bytes."),
                )
                .await;
            }
            if !subs.contains_key(&sub) && subs.len() >= MAX_SUBSCRIPTIONS {
                return send_error(
                    socket,
                    Some(sub),
                    &Error::new(
                        ErrorCode::EffectiveScopeLimitExceeded,
                        "Too many live subscriptions on this session.",
                    ),
                )
                .await;
            }
            match (query, records) {
                (Some(input), None) => {
                    let mut subscription = Subscription::Query {
                        input,
                        last_payload: None,
                    };
                    if let Err(error) =
                        eval_subscription(host, actor, space, &sub, &mut subscription, None, socket)
                            .await
                    {
                        send_error(socket, Some(sub), &error).await?;
                    } else {
                        subs.insert(sub, subscription);
                    }
                    Ok(())
                }
                (None, Some(req)) => {
                    let mut subscription = Subscription::Records {
                        item: req.item,
                        last_anchor: req.after.clone(),
                        last_version: 0,
                        last_count: 0,
                        last_meta: None,
                    };
                    if let Err(error) = eval_subscription(
                        host,
                        actor,
                        space,
                        &sub,
                        &mut subscription,
                        Some(req.after),
                        socket,
                    )
                    .await
                    {
                        send_error(socket, Some(sub), &error).await?;
                    } else {
                        subs.insert(sub, subscription);
                    }
                    Ok(())
                }
                _ => {
                    send_error(
                        socket,
                        Some(sub),
                        &Error::invalid_input(
                            "Provide either query or records in a subscribe frame.",
                        ),
                    )
                    .await
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
    socket: &mut WebSocket,
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
    socket: &mut WebSocket,
) -> calternal_types::Result<()> {
    match sub {
        Subscription::Query {
            input,
            last_payload,
        } => {
            let value = host
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
            let encoded = serde_json::to_string(&value).unwrap_or_default();
            if last_payload.as_deref() == Some(encoded.as_str()) {
                return Ok(());
            }
            *last_payload = Some(encoded);
            let frame = ServerFrame::Rows {
                sub: sub_id.to_owned(),
                rows: value.get("rows").cloned().unwrap_or_else(|| json!([])),
                totals: value.get("totals").cloned().unwrap_or_else(|| json!({})),
                cursor: value.get("cursor").cloned().unwrap_or(Value::Null),
            };
            let _ = send_frame(socket, &frame).await;
            Ok(())
        }
        Subscription::Records {
            item,
            last_anchor,
            last_version,
            last_count,
            last_meta,
        } => {
            let value = host
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
                fields: value["fields"].clone(),
                computed: value["computed"].clone(),
                state: value["state"].clone(),
                when: value["when"].clone(),
            };
            let records = value
                .get("records")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
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
                let _ = send_frame(socket, &frame).await;
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
                    let _ = send_frame(
                        socket,
                        &ServerFrame::RecordAppend {
                            sub: sub_id.to_owned(),
                            item: *item,
                            version,
                            record: record.clone(),
                        },
                    )
                    .await;
                }
                if last_meta.as_ref() != Some(&metadata) {
                    let _ = send_frame(
                        socket,
                        &ServerFrame::ItemChange {
                            sub: sub_id.to_owned(),
                            item: *item,
                            version,
                            metadata: metadata.clone(),
                        },
                    )
                    .await;
                }
                *last_anchor = tail_anchor;
                *last_version = version;
                *last_count = records.len();
                *last_meta = Some(metadata.clone());
                return Ok(());
            }

            if version != *last_version || records.len() != *last_count {
                // Record edit, redaction, or replacement: send a clean snapshot.
                let frame = ServerFrame::Records {
                    sub: sub_id.to_owned(),
                    item: *item,
                    version,
                    metadata: metadata.clone(),
                    mode: "snapshot",
                    after: None,
                    records: records.clone(),
                    last_anchor: tail_anchor.clone(),
                };
                *last_anchor = tail_anchor;
                *last_version = version;
                *last_count = records.len();
                *last_meta = Some(metadata.clone());
                let _ = send_frame(socket, &frame).await;
            } else if last_meta.as_ref() != Some(&metadata) {
                *last_meta = Some(metadata.clone());
                let _ = send_frame(
                    socket,
                    &ServerFrame::ItemChange {
                        sub: sub_id.to_owned(),
                        item: *item,
                        version,
                        metadata,
                    },
                )
                .await;
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

async fn send_frame(socket: &mut WebSocket, frame: &ServerFrame) -> std::result::Result<(), ()> {
    let Ok(text) = serde_json::to_string(frame) else {
        return Err(());
    };
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}

async fn send_error(
    socket: &mut WebSocket,
    sub: Option<String>,
    error: &Error,
) -> std::result::Result<(), ()> {
    let (code, next) = match error.code {
        ErrorCode::Unauthorized => ("needs-sign-in", "Sign in and try again."),
        ErrorCode::Forbidden | ErrorCode::PasskeyReverificationRequired => {
            ("not-allowed", "Check your access.")
        }
        ErrorCode::NotFound => ("not-found", "Check the requested ID."),
        ErrorCode::Conflict => ("conflict", "Refresh before retrying."),
        ErrorCode::InvalidInput => ("invalid", "Check the subscription arguments."),
        ErrorCode::TooLarge => ("too-large", "Reduce the frame size."),
        ErrorCode::EffectiveScopeLimitExceeded | ErrorCode::RetryAfter => {
            ("limit", "Reduce active subscriptions and try again.")
        }
        ErrorCode::Internal | ErrorCode::Unavailable => ("unreachable", "Try again later."),
    };
    send_frame(
        socket,
        &ServerFrame::Error {
            sub,
            code,
            message: error.message.clone(),
            next,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_after_returns_only_subsequent_records_in_order() {
        let records = vec![
            json!({"anchor": "a1", "body": "one"}),
            json!({"anchor": "a2", "body": "two"}),
            json!({"anchor": "a3", "body": "three"}),
        ];
        let (mode, delta, after) = slice_after(&records, Some("a1"));
        assert_eq!(mode, "delta");
        assert_eq!(after.as_deref(), Some("a1"));
        assert_eq!(delta.len(), 2);
        assert_eq!(delta[0]["anchor"], "a2");
        assert_eq!(delta[1]["anchor"], "a3");
    }

    #[test]
    fn slice_after_falls_back_to_snapshot_for_unknown_anchor() {
        let records = vec![
            json!({"anchor": "a1", "body": "one"}),
            json!({"anchor": "a2", "body": "two"}),
        ];
        let (mode, out, after) = slice_after(&records, Some("gone"));
        assert_eq!(mode, "snapshot");
        assert!(after.is_none());
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn hub_notifies_only_matching_space() {
        let hub = Hub::default();
        let s1 = SpaceId::new();
        let s2 = SpaceId::new();
        let owner = AccountId::new();
        let other = AccountId::new();
        let mut rx1 = hub.subscribe(s1, owner);
        let mut rx1_other = hub.subscribe(s1, other);
        let mut rx2 = hub.subscribe(s2, owner);
        hub.notify(s1, None);
        assert!(rx1.try_recv().is_ok());
        assert!(rx1_other.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());

        // Simulate an idle peer in a multi-person space. Private changes
        // must neither enqueue an event for the peer nor lag its channel.
        for _ in 0..(SPACE_CHANNEL_CAPACITY + 1) {
            hub.notify(s1, Some(owner));
        }
        assert!(matches!(
            rx1.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(_))
        ));
        assert!(matches!(
            rx1_other.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        assert!(matches!(
            rx2.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }
}
