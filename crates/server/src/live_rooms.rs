//! Versioned WebSocket transport for opted-in host content rooms.
mod access;
mod hub;
mod update;
mod wire;

use access::Proof;
pub(crate) use hub::Hub;
use hub::{Key, Lease, Termination, closed, disk};

use crate::{
    api::Shared,
    auth::{Directory, Identity},
    live::{self, LiveQuery},
    plugins::{Plugin, Session, activation},
};
use axum::{
    Extension,
    extract::{FromRequestParts, Query, Request, State, ws},
    http::header,
    response::{IntoResponse, Response},
};
use calternal_content::Content;
use calternal_types::{AccountId, Error};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{OwnedSemaphorePermit, mpsc, watch},
    task::JoinHandle,
    time::Instant,
};

const VERSION: &str = "calternal-live-rooms.v1";
const MEMBERSHIPS: usize = 128;
const CONTROL_BYTES: usize = 4096;
const SAFE_INT: u64 = (1 << 53) - 1;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const ACCESS_REVOKED: &str = "access_revoked";

pub(crate) async fn entry<D: Directory>(
    State(host): State<Shared<D>>,
    Extension(content): Extension<Arc<Content>>,
    req: Request,
) -> Response {
    let upgrade = req.headers().contains_key(header::SEC_WEBSOCKET_KEY)
        || req
            .headers()
            .get(header::UPGRADE)
            .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if !upgrade {
        return match Query::<LiveQuery>::try_from_uri(req.uri()) {
            Ok(query) => live::events(State(host), Extension(content), query, req).await,
            Err(error) => error.into_response(),
        };
    }
    let (mut parts, _) = req.into_parts();
    let Some(identity) = parts.extensions.get::<Identity>().cloned() else {
        return crate::api::error_response(Error::unauthorized("missing or invalid session"));
    };
    let Some(session) = parts.extensions.get::<Session>().cloned() else {
        return crate::api::error_response(Error::unauthorized("missing or invalid session"));
    };
    if matches!(session, Session::Installation { .. })
        && !parts.headers.contains_key(header::ORIGIN)
    {
        return crate::api::error_response(Error::forbidden(
            "room socket requires a browser origin",
        ));
    }
    let supported = parts
        .headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(',').map(str::trim))
        .any(|protocol| protocol == VERSION);
    if !supported {
        return crate::api::error_response(Error::invalid_input(
            "unsupported live room protocol version",
        ));
    }
    let ws = match ws::WebSocketUpgrade::from_request_parts(&mut parts, &host).await {
        Ok(ws) => ws,
        Err(error) => return error.into_response(),
    };
    let socket_memory = host
        .store
        .live_rooms()
        .memory()
        .clone()
        .try_acquire_many_owned(wire::FRAME_BYTES as u32);
    let Ok(socket_memory) = socket_memory else {
        return crate::api::error_response(Error::rate_limited("live socket capacity is busy"));
    };
    ws.protocols([VERSION])
        .max_frame_size(wire::FRAME_BYTES)
        .max_message_size(wire::FRAME_BYTES)
        .on_upgrade(move |socket| serve(socket, host, content, identity.id, session, socket_memory))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Control {
    Join {
        room: String,
        membership: u64,
        lineage: Option<String>,
        tz: Option<String>,
    },
    Leave {
        room: String,
        membership: u64,
    },
    Ping,
}

struct Member {
    key: Key,
    plugin: Arc<Plugin>,
    generation: u32,
    proof: Arc<Proof>,
    lineage: String,
    lease: Lease,
    resync: bool,
    held: Option<u64>,
    check: u64,
    relay: JoinHandle<()>,
    termination: watch::Receiver<Option<Termination>>,
}

impl Drop for Member {
    fn drop(&mut self) {
        self.relay.abort();
    }
}

enum Event {
    Packet {
        address: String,
        frame: Arc<hub::Frame>,
    },
    Left(String),
}

struct Connection<D: Directory> {
    socket: ws::WebSocket,
    host: Shared<D>,
    content: Arc<Content>,
    account: AccountId,
    session: Session,
    members: HashMap<String, Member>,
    event_tx: mpsc::Sender<Event>,
    reassembly: wire::Reassembly,
    highest: u64,
}

async fn serve<D: Directory>(
    socket: ws::WebSocket,
    host: Shared<D>,
    content: Arc<Content>,
    account: AccountId,
    session: Session,
    _socket_memory: OwnedSemaphorePermit,
) {
    let (event_tx, mut events) = mpsc::channel(64);
    let mut conn = Connection {
        socket,
        host,
        content,
        account,
        session,
        members: HashMap::new(),
        event_tx,
        reassembly: wire::Reassembly::default(),
        highest: 0,
    };
    let mut check = tokio::time::interval(Duration::from_secs(2));
    check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let deadline = conn.reassembly.deadline();
        let expire = async {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let result = tokio::select! {
            message = conn.socket.recv() => match message {
                Some(Ok(ws::Message::Text(value))) => conn.control(value.as_str()).await,
                Some(Ok(ws::Message::Binary(value))) => conn.binary(&value).await,
                Some(Ok(ws::Message::Ping(_) | ws::Message::Pong(_))) => Ok(()),
                _ => break,
            },
            event = events.recv() => match event {
                Some(event) => conn.event(event).await,
                None => break,
            },
            _ = check.tick() => conn.sweep().await,
            _ = expire => Err(Error::invalid_input("room chunk expired")),
        };
        if result.is_err() {
            break;
        }
    }
}

impl<D: Directory> Connection<D> {
    async fn control(&mut self, value: &str) -> Result<(), Error> {
        if value.len() > CONTROL_BYTES {
            return Err(Error::invalid_input("room control is too large"));
        }
        let control: Control = serde_json::from_str(value)
            .map_err(|_| Error::invalid_input("invalid room control"))?;
        match control {
            Control::Ping => self.json(json!({"type":"pong"})).await,
            Control::Leave { room, membership } => {
                let address = address(&room, membership);
                self.reassembly.discard(&address);
                self.members.remove(&address);
                Ok(())
            }
            Control::Join {
                room,
                membership,
                lineage,
                tz,
            } => {
                let result = if tz.as_deref().is_some_and(|value| value.len() > 32) {
                    Err(Error::invalid_input("invalid room join metadata"))
                } else {
                    self.join(&room, membership, lineage.as_deref()).await
                };
                if let Err(error) = result {
                    self.json(json!({"type":"join_failed","room":room,"membership":membership,"error":error})).await?;
                }
                Ok(())
            }
        }
    }

    async fn join(
        &mut self,
        room: &str,
        membership: u64,
        lineage: Option<&str>,
    ) -> Result<(), Error> {
        let invalid = || Error::invalid_input("invalid room join");
        if membership <= self.highest
            || membership > SAFE_INT
            || lineage.is_some_and(|lineage| lineage.len() > 64)
        {
            return Err(invalid());
        }
        self.highest = membership;
        let key = Key::parse(room).ok_or_else(invalid)?;
        if self.members.len() >= MEMBERSHIPS {
            return Err(Error::rate_limited("socket room limit reached"));
        }
        if self.members.values().any(|existing| existing.key == key) {
            return Err(invalid());
        }
        let plugin = self.catalog(&key).ok_or_else(hidden)?;
        let (proof, generation) = self.admit(&key, &plugin).await?;
        let joined = self
            .host
            .store
            .live_rooms()
            .join(self.content.clone(), key.clone(), self.account, lineage)
            .await?;
        let address = address(room, membership);
        let bind = address.clone();
        let tx = self.event_tx.clone();
        let termination = joined.termination.clone();
        let (mut recv, mut ended) = (joined.frames, joined.termination);
        let relay = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = ended.changed() => break,
                    event = recv.recv() => match event {
                        Ok(frame) => {
                            if tx.try_send(Event::Packet { address: bind.clone(), frame }).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            let _ = tx.send(Event::Left(bind)).await;
        });
        let lineage = joined.lineage;
        self.members.insert(
            address.clone(),
            Member {
                key,
                plugin,
                generation,
                proof,
                lineage: lineage.clone(),
                lease: joined.lease,
                resync: false,
                held: None,
                check: 0,
                relay,
                termination,
            },
        );
        if let Err(error) = self.authorize_member(&self.members[&address], false).await {
            self.members.remove(&address);
            return Err(if access_revoked(&error) {
                hidden()
            } else {
                error
            });
        }
        self.json(json!({
            "type":"joined","room":room,"membership":membership,
            "info":{"id":self.members[&address].key.item,"lineage":lineage,"text":"md"},
        }))
        .await?;
        for frame in joined.initial {
            self.send(&address, &frame).await?;
        }
        Ok(())
    }

    async fn sweep(&mut self) -> Result<(), Error> {
        if !self.session.is_live(&self.host.directory).await {
            return Err(Error::unauthorized("room session ended"));
        }
        let mut removed = Vec::new();
        let mut scan = None;
        for (address, member) in &self.members {
            if member.termination.borrow().is_some() {
                removed.push(address.clone());
            } else if scan.is_none()
                && member.lease.file_scan_due()
                && self.authorize_member(member, true).await.is_ok()
                && member.termination.borrow().is_none()
                && member.lease.claim_file_scan()
            {
                scan = Some(member.key.clone());
            }
        }
        for address in removed {
            self.left(&address, None).await?;
        }
        if let Some(key) = scan {
            let (content, account) = (self.content.clone(), self.account);
            if let Err(error) =
                disk(move || content.reconcile_room_file(key.space, &key.kind, key.item, account))
                    .await
            {
                tracing::debug!(code = ?error.code, "room outside-file scan deferred");
            }
        }
        Ok(())
    }

    async fn event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Left(address) => self.left(&address, None).await,
            Event::Packet { address, frame } => {
                let Some(member) = self.members.get(&address) else {
                    return Ok(());
                };
                if let Err(error) = self.authorize_member(member, false).await {
                    return self.left(&address, Some(&error)).await;
                }
                self.send(&address, &frame.bytes).await
            }
        }
    }

    async fn left(&mut self, address: &str, error: Option<&Error>) -> Result<(), Error> {
        self.reassembly.discard(address);
        if let Some(member) = self.members.remove(address) {
            let revoked = *member.termination.borrow() == Some(Termination::AccessRevoked)
                || error.is_some_and(access_revoked);
            drop(member);
            let (room, membership) = unaddress(address).ok_or_else(closed)?;
            let mut control = json!({"type":"left","room":room,"membership":membership});
            if revoked {
                control["reason"] = json!(ACCESS_REVOKED);
            }
            self.json(control).await?;
        }
        Ok(())
    }

    async fn send(&mut self, address: &str, bytes: &[u8]) -> Result<(), Error> {
        for frame in wire::frames(address, bytes) {
            let Some(member) = self.members.get(address) else {
                return Ok(());
            };
            let hub = self.host.store.live_rooms();
            let refresh = member.proof.refresh_needed(
                hub.item_authority_epoch(),
                hub.activation_epoch(member.key.space),
            );
            if member.termination.borrow().is_some() {
                return self.left(address, None).await;
            }
            if refresh && let Err(error) = self.authorize_member(member, false).await {
                return self.left(address, Some(&error)).await;
            }
            self.frame(ws::Message::Binary(frame.into())).await?;
        }
        Ok(())
    }

    async fn json(&mut self, value: Value) -> Result<(), Error> {
        self.frame(ws::Message::Text(value.to_string().into()))
            .await
    }

    async fn frame(&mut self, frame: ws::Message) -> Result<(), Error> {
        tokio::time::timeout_at(Instant::now() + IO_TIMEOUT, self.socket.send(frame))
            .await
            .map_err(|_| closed())?
            .map_err(|_| closed())
    }
}

fn address(room: &str, membership: u64) -> String {
    format!("@{membership}/{room}")
}

fn unaddress(address: &str) -> Option<(&str, u64)> {
    let (membership, room) = address.strip_prefix('@')?.split_once('/')?;
    Some((room, membership.parse().ok()?))
}

fn hidden() -> Error {
    Error::not_found("live room not found")
}

fn access_revoked(error: &Error) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details["reason"].as_str())
        == Some(ACCESS_REVOKED)
}

fn revoked(error: Error) -> Error {
    error.with_details(json!({"reason":ACCESS_REVOKED}))
}
