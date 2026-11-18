//! Real WebSocket and Yjs clients on the perf VM's monotonic clock.
mod access_storm;

use std::{
    collections::BTreeMap,
    net::TcpStream,
    path::Path,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tungstenite::{Message as Frame, WebSocket, client::IntoClientRequest, stream::MaybeTlsStream};
use yrs::{
    Doc, GetString, ReadTxn, Text, Transact, Update,
    encoding::{
        read::{Cursor, Read},
        write::Write,
    },
    sync::{Message, SyncMessage},
    updates::{decoder::Decode, encoder::Encode},
};

use crate::{
    http::{Http, ORIGIN, Result},
    live, timings,
};

pub(super) const PEER: &str = "sync.round_trip";
pub(super) const SAVE: &str = "sync.save_ack";
pub(super) use access_storm::sample as sample_access_storm;
pub(super) use access_storm::{METRIC as ACCESS_STORM, SAMPLES as ACCESS_STORM_SAMPLES};

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

pub(super) fn sample(
    http: &Http,
    space: &str,
    item: &str,
    count: usize,
    require_commit_timings: bool,
    log: &Path,
) -> Result<BTreeMap<String, Vec<f64>>> {
    let room = format!("{space}/notes/{item}");
    let address = format!("@1/{room}");
    let mut writer = connect(http, &room)?;
    let mut peer = connect(http, &room)?;
    let (author, observer) = (
        initial(&mut writer, &address)?,
        initial(&mut peer, &address)?,
    );
    let mut journal = live::Live::open(
        http,
        "/api/v1/live",
        &[(
            "subscriptions".into(),
            json!([{"space_id":space,"bookmark":null}]).to_string(),
        )],
    )?;
    journal.wait_type("ready")?;
    let mut peer_times = Vec::with_capacity(count);
    let mut save_times = Vec::with_capacity(count);
    let mut deliveries = BTreeMap::new();
    let item = uuid::Uuid::parse_str(item)?;
    let author_text = author.get_or_insert_text("md");
    let observer_text = observer.get_or_insert_text("md");
    for n in 1..=count {
        let insert = format!("[room-{n}]");
        let start = edit_send(
            &mut writer,
            &address,
            &author,
            &author_text,
            &insert,
            n as u64,
        )?;
        let (peer_ms, save_ms, entry, delivered_ns) = std::thread::scope(|scope| -> Result<_> {
            let saved = scope.spawn(|| {
                save_receipt(&mut writer, &address, n as u64, start).map_err(|e| e.to_string())
            });
            let delivered = scope.spawn(|| -> std::result::Result<_, String> {
                let event = journal.wait_type("entry").map_err(|e| e.to_string())?;
                let timestamp = calternal_types::perf_monotonic_ns();
                Ok((event, timestamp))
            });
            let peer_ms = loop {
                if let Message::Sync(SyncMessage::Update(update)) = received(&mut peer, &address)? {
                    apply(&observer, &update)?;
                    let elapsed = start.elapsed().as_secs_f64() * 1000.;
                    if observer_text
                        .get_string(&observer.transact())
                        .ends_with(&insert)
                    {
                        break elapsed;
                    }
                }
            };
            let save_ms = saved.join().map_err(|_| "room receipt reader panicked")??;
            let (entry, delivered_ns) =
                delivered.join().map_err(|_| "journal reader panicked")??;
            Ok((peer_ms, save_ms, entry, delivered_ns))
        })?;
        if entry["entry"]["item"] != item.to_string() {
            return Err("room keystroke SSE entry has a different item".into());
        }
        let receipt_id = entry["entry"]["request_id"]
            .as_str()
            .ok_or("missing room request identity")?;
        if !receipt_id.starts_with("room/")
            || deliveries.insert(receipt_id.into(), delivered_ns).is_some()
        {
            return Err("room keystroke received an invalid or repeated SSE entry".into());
        }
        peer_times.push(peer_ms);
        save_times.push(save_ms);
    }
    if author_text.get_string(&author.transact()) != observer_text.get_string(&observer.transact())
    {
        return Err("room clients did not converge".into());
    }
    let mut samples = BTreeMap::from([(PEER.into(), peer_times), (SAVE.into(), save_times)]);
    timings::collect(log, &deliveries, &mut samples, require_commit_timings)?;
    drop((writer, peer, journal));
    samples.insert(
        "sync.storm.save_ack".into(),
        storm(http, &room, &address, item)?,
    );
    Ok(samples)
}

fn storm(http: &Http, room: &str, address: &str, item: uuid::Uuid) -> Result<Vec<f64>> {
    const EDITORS: usize = 4;
    const EDITS: usize = 25;
    let mut replicas = Vec::with_capacity(EDITORS);
    for _ in 0..EDITORS {
        let mut socket = connect(http, room)?;
        let doc = initial(&mut socket, address)?;
        replicas.push((socket, doc));
    }
    let start = std::sync::Barrier::new(EDITORS);
    let timings = std::thread::scope(|scope| -> Result<Vec<f64>> {
        let mut writers = Vec::with_capacity(EDITORS);
        for (author, (mut socket, doc)) in replicas.into_iter().enumerate() {
            let start = &start;
            writers.push(
                scope.spawn(move || -> std::result::Result<Vec<f64>, String> {
                    let text = doc.get_or_insert_text("md");
                    start.wait();
                    let mut samples = Vec::with_capacity(EDITS);
                    for edit in 1..=EDITS {
                        let insert = format!("[storm-{author}-{edit}]");
                        let check = edit as u64;
                        let start = edit_send(&mut socket, address, &doc, &text, &insert, check)
                            .map_err(|error| error.to_string())?;
                        samples.push(
                            save_receipt(&mut socket, address, check, start)
                                .map_err(|error| error.to_string())?,
                        );
                    }
                    Ok(samples)
                }),
            );
        }
        let mut timings = Vec::with_capacity(EDITORS * EDITS);
        for writer in writers {
            timings.extend(writer.join().map_err(|_| "storm editor panicked")??);
        }
        Ok(timings)
    })?;
    let (saved, _) = http.op_id("notes.get", json!({"id":item}), "room-storm-readback")?;
    let body = saved["note"]["markdown"]
        .as_str()
        .ok_or("storm text is unreadable")?;
    for author in 0..EDITORS {
        for edit in 1..=EDITS {
            let marker = format!("[storm-{author}-{edit}]");
            if body.match_indices(&marker).take(2).count() != 1 {
                return Err(
                    format!("the storm lost or duplicated editor {author} edit {edit}").into(),
                );
            }
        }
    }
    Ok(timings)
}

fn edit_send(
    socket: &mut Socket,
    address: &str,
    doc: &Doc,
    text: &yrs::TextRef,
    insert: &str,
    check: u64,
) -> Result<Instant> {
    let (start, update) = {
        let mut txn = doc.transact_mut();
        let at = text.len(&txn);
        let start = Instant::now();
        text.insert(&mut txn, at, insert);
        (start, txn.encode_update_v1())
    };
    send(socket, address, Message::Sync(SyncMessage::Update(update)))?;
    let mut receipt = Vec::new();
    receipt.write_var(check);
    send(socket, address, Message::Custom(100, receipt))?;
    Ok(start)
}

fn connect(http: &Http, room: &str) -> Result<Socket> {
    let mut socket = open(http)?;
    join_room(&mut socket, room, 1)?;
    Ok(socket)
}

fn open(http: &Http) -> Result<Socket> {
    let base = http
        .base
        .strip_prefix("http://")
        .ok_or("expected the local perf server")?;
    let mut request = format!("ws://{base}/api/v1/live").into_client_request()?;
    request
        .headers_mut()
        .insert("host", "localhost:18080".parse()?);
    request.headers_mut().insert("origin", ORIGIN.parse()?);
    request.headers_mut().insert("cookie", http.cookie.parse()?);
    request
        .headers_mut()
        .insert("sec-websocket-protocol", "calternal-live-rooms.v1".parse()?);
    let (mut socket, _) = tungstenite::connect(request)?;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    }
    Ok(socket)
}

fn join_room(socket: &mut Socket, room: &str, membership: usize) -> Result<()> {
    socket.send(Frame::text(
        json!({
            "type":"join","room":room,"membership":membership
        })
        .to_string(),
    ))?;
    loop {
        match socket.read()? {
            Frame::Binary(_) => continue,
            Frame::Text(control) => {
                let value: Value = serde_json::from_str(&control)?;
                if value["type"] == "joined"
                    && value["room"] == room
                    && value["membership"] == membership
                {
                    return Ok(());
                }
                return Err(format!("room join failed: {value}").into());
            }
            frame => return Err(format!("room join interrupted: {frame:?}").into()),
        }
    }
}

fn initial(socket: &mut Socket, address: &str) -> Result<Doc> {
    let doc = Doc::new();
    doc.get_or_insert_text("md");
    loop {
        if let Message::Sync(SyncMessage::SyncStep2(update)) = received(socket, address)? {
            apply(&doc, &update)?;
            return Ok(doc);
        }
    }
}

fn apply(doc: &Doc, bytes: &[u8]) -> Result<()> {
    doc.transact_mut().apply_update(Update::decode_v1(bytes)?)?;
    Ok(())
}

fn received(socket: &mut Socket, address: &str) -> Result<Message> {
    let (received, message) = any_message(socket)?;
    if received != address {
        return Err("room returned a stale membership".into());
    }
    Ok(message)
}

fn any_message(socket: &mut Socket) -> Result<(String, Message)> {
    let frame = socket.read()?;
    let Frame::Binary(bytes) = frame else {
        return Err(format!("room unexpectedly closed: {frame:?}").into());
    };
    let mut cursor = Cursor::new(&bytes);
    let address = cursor.read_string()?.to_owned();
    let message = Message::decode_v1(&bytes[cursor.next..])?;
    if matches!(message, Message::Custom(102 | 103, _)) {
        return Err(format!("room refused a benchmark edit: {message:?}").into());
    }
    Ok((address, message))
}

fn send(socket: &mut Socket, address: &str, message: Message) -> Result<()> {
    let mut bytes = Vec::new();
    bytes.write_string(address);
    bytes.extend_from_slice(&message.encode_v1());
    socket.send(Frame::binary(bytes))?;
    Ok(())
}

fn save_receipt(socket: &mut Socket, address: &str, check: u64, start: Instant) -> Result<f64> {
    loop {
        if let Message::Custom(101, ack) = received(socket, address)? {
            if Cursor::new(&ack).read_var::<u64>()? != check {
                return Err("room save receipt did not match its keystroke".into());
            }
            return Ok(start.elapsed().as_secs_f64() * 1000.);
        }
    }
}
