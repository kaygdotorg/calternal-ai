//! Initial sync, author and observer delivery, and restart on a disposable server.
use super::{Server, TempDir, create, op, raw, start_full};
use serde_json::{Value, json};
use std::{
    net::TcpStream,
    path::Path,
    time::{Duration, Instant},
};
use tungstenite::{
    Message as Frame, WebSocket, client::IntoClientRequest, http::HeaderValue,
    stream::MaybeTlsStream,
};
use yrs::{
    Doc, GetString, ReadTxn, Text, Transact, Update,
    encoding::{
        read::{Cursor, Read},
        write::Write,
    },
    sync::{Message, SyncMessage},
    updates::{decoder::Decode, encoder::Encode},
};

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

fn socket(port: u16, token: &str) -> Socket {
    let mut request = format!("ws://127.0.0.1:{port}/api/v1/live")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", token.trim()).parse().unwrap(),
    );
    request.headers_mut().insert(
        "sec-websocket-protocol",
        HeaderValue::from_static("calternal-live-rooms.v1"),
    );
    let (mut socket, response) = tungstenite::connect(request).unwrap();
    assert_eq!(
        response.headers()["sec-websocket-protocol"],
        "calternal-live-rooms.v1"
    );
    let MaybeTlsStream::Plain(stream) = socket.get_mut() else {
        panic!("test socket uses loopback HTTP");
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
}

fn join(socket: &mut Socket, room: &str, lineage: Option<&str>) -> (Doc, String) {
    socket
        .send(Frame::text(
            json!({
                "type":"join","room":room,"membership":1,"lineage":lineage
            })
            .to_string(),
        ))
        .unwrap();
    let frame = socket.read().unwrap();
    let control: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
    assert_eq!(control["type"], "joined", "{control}");
    let lineage = control["info"]["lineage"].as_str().unwrap().into();
    let doc = Doc::new();
    doc.get_or_insert_text("md");
    let address = format!("@1/{room}");
    loop {
        if let Message::Sync(SyncMessage::SyncStep2(bytes)) = recv(socket, &address) {
            apply(&doc, &bytes);
            return (doc, lineage);
        }
    }
}

fn recv(socket: &mut Socket, address: &str) -> Message {
    let frame = socket.read().unwrap();
    let Frame::Binary(bytes) = frame else {
        panic!("expected binary room data, got {frame:?}")
    };
    let mut reader = Cursor::new(&bytes);
    assert_eq!(reader.read_string().unwrap(), address);
    Message::decode_v1(&bytes[reader.next..]).unwrap()
}

fn send(socket: &mut Socket, address: &str, message: Message) {
    let mut bytes = Vec::new();
    bytes.write_string(address);
    bytes.extend_from_slice(&message.encode_v1());
    socket.send(Frame::binary(bytes)).unwrap();
}

fn apply(doc: &Doc, bytes: &[u8]) {
    doc.transact_mut()
        .apply_update(Update::decode_v1(bytes).unwrap())
        .unwrap();
}

fn body(doc: &Doc) -> String {
    doc.get_or_insert_text("md").get_string(&doc.transact())
}

fn guest_id(srv: &Server) -> String {
    let identity = raw(
        srv.port,
        "GET",
        "/api/v1/describe",
        &[("Authorization", &format!("Bearer {}", srv.guest))],
        None,
    )
    .unwrap();
    assert_eq!(identity.status, 200, "{}", identity.body);
    identity.body["principal"]["id"].as_str().unwrap().into()
}

#[test]
fn live_room_typing_reaches_the_peer_and_restarts_from_durable_state() {
    let data = TempDir::new("live-room");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let item = create(&srv, "room", &srv.shared, "abc");
    let room = format!("{}/notes/{}", srv.shared, item["id"].as_str().unwrap());
    let address = format!("@1/{room}");
    let (mut author, mut observer) = (socket(srv.port, &srv.owner), socket(srv.port, &srv.guest));
    let (authored, lineage) = join(&mut author, &room, None);
    let (observed, peer_lineage) = join(&mut observer, &room, None);
    assert_eq!(lineage, peer_lineage);
    assert_eq!(body(&observed), "abc");
    let text = authored.get_or_insert_text("md");
    let vector = authored.transact().state_vector();
    text.insert(&mut authored.transact_mut(), 3, "!");
    let update = authored.transact().encode_diff_v1(&vector);
    send(
        &mut author,
        &address,
        Message::Sync(SyncMessage::Update(update)),
    );
    send(&mut author, &address, Message::Custom(100, vec![1]));
    loop {
        if let Message::Sync(SyncMessage::Update(bytes)) = recv(&mut observer, &address) {
            apply(&observed, &bytes);
            break;
        }
    }
    assert_eq!(body(&observed), "abc!");
    loop {
        if let Message::Custom(101, bytes) = recv(&mut author, &address) {
            assert_eq!(bytes, [1]);
            break;
        }
    }
    drop((author, observer, srv));
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let (loaded, after_lineage) = join(&mut socket(srv.port, &srv.owner), &room, Some(&lineage));
    assert_eq!(body(&loaded), "abc!");
    assert_eq!(after_lineage, lineage);
}

#[cfg(debug_assertions)]
#[test]
fn crash_before_room_commit_keeps_replay_pending() {
    let data = TempDir::new("live-room-crash");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let item = create(&srv, "room-before-crash", &srv.shared, "abc");
    let room = format!("{}/notes/{}", srv.shared, item["id"].as_str().unwrap());
    let address = format!("@1/{room}");
    drop(srv);

    let mut srv = start_full(&data.0, Some("before_room_commit"), None, Some(&plugins));
    let mut author = socket(srv.port, &srv.owner);
    let (authored, lineage) = join(&mut author, &room, None);
    let text = authored.get_or_insert_text("md");
    let vector = authored.transact().state_vector();
    text.insert(&mut authored.transact_mut(), 3, "!");
    let update = authored.transact().encode_diff_v1(&vector);
    send(
        &mut author,
        &address,
        Message::Sync(SyncMessage::Update(update.clone())),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = srv.child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "room did not reach its crash point"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop((author, srv));

    let srv = start_full(&data.0, None, None, Some(&plugins));
    let mut author = socket(srv.port, &srv.owner);
    let (loaded, after_lineage) = join(&mut author, &room, Some(&lineage));
    assert_eq!(body(&loaded), "abc");
    assert_eq!(after_lineage, lineage);
    send(
        &mut author,
        &address,
        Message::Sync(SyncMessage::SyncStep2(update)),
    );
    send(&mut author, &address, Message::Custom(100, vec![1]));
    loop {
        if let Message::Custom(101, bytes) = recv(&mut author, &address) {
            assert_eq!(bytes, [1]);
            break;
        }
    }
    drop((author, srv));
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let (loaded, _) = join(&mut socket(srv.port, &srv.owner), &room, Some(&lineage));
    assert_eq!(body(&loaded), "abc!");
}

#[test]
fn an_offline_replica_rejoins_over_a_committed_api_edit() {
    let data = TempDir::new("live-room-offline");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let item = create(&srv, "room-offline", &srv.shared, "abc");
    let room = format!("{}/notes/{}", srv.shared, item["id"].as_str().unwrap());
    let address = format!("@1/{room}");
    let mut offline_socket = socket(srv.port, &srv.owner);
    let (offline, lineage) = join(&mut offline_socket, &room, None);
    offline
        .get_or_insert_text("md")
        .insert(&mut offline.transact_mut(), 3, " offline");
    drop(offline_socket);

    let api_edit = super::op(
        &srv,
        &srv.owner,
        "room-other-writer",
        "notes.update",
        json!({
            "id":item["id"], "space_id":srv.shared, "base_revision":item["revision"],
            "markdown":"other writer abc"
        }),
    );
    assert_eq!(api_edit.status, 200, "{}", api_edit.body);
    drop(srv);

    let srv = start_full(&data.0, None, None, Some(&plugins));
    let mut online = socket(srv.port, &srv.owner);
    let (host, same_lineage) = join(&mut online, &room, Some(&lineage));
    assert_eq!(same_lineage, lineage);
    assert_eq!(body(&host), "other writer abc");
    let local = offline
        .transact()
        .encode_diff_v1(&host.transact().state_vector());
    send(
        &mut online,
        &address,
        Message::Sync(SyncMessage::SyncStep2(local)),
    );
    send(&mut online, &address, Message::Custom(100, vec![1]));
    loop {
        if let Message::Custom(101, bytes) = recv(&mut online, &address) {
            assert_eq!(bytes, [1]);
            break;
        }
    }
    drop((online, srv));
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let (merged, _) = join(&mut socket(srv.port, &srv.owner), &room, Some(&lineage));
    assert_eq!(body(&merged), "other writer abc offline");
}

#[test]
fn room_join_hides_foreign_items_like_missing_items() {
    let data = TempDir::new("live-room-foreign");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let private = create(&srv, "room-private", &srv.personal, "private");
    let mut guest = socket(srv.port, &srv.guest);
    let mut failure = |item: &str, membership| -> Value {
        let room = format!("{}/notes/{item}", srv.shared);
        guest
            .send(Frame::text(
                json!({"type":"join","room":room,"membership":membership}).to_string(),
            ))
            .unwrap();
        let reply = guest.read().unwrap();
        let reply: Value = serde_json::from_str(reply.to_text().unwrap()).unwrap();
        assert_eq!(reply["type"], "join_failed");
        assert_eq!(reply["error"]["code"], "not_found");
        reply["error"].clone()
    };
    let foreign = failure(private["id"].as_str().unwrap(), 1);
    let missing = failure(&uuid::Uuid::new_v4().to_string(), 2);
    assert_eq!(foreign, missing);
}

#[test]
fn losing_view_ends_only_that_membership_and_requests_rescue() {
    let data = TempDir::new("live-room-revoked");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let item = create(&srv, "revocation-room", &srv.shared, "original");
    let room = format!("{}/notes/{}", srv.shared, item["id"].as_str().unwrap());
    let mut guest = socket(srv.port, &srv.guest);
    join(&mut guest, &room, None);
    assert!(matches!(
        recv(&mut guest, &format!("@1/{room}")),
        Message::Awareness(_)
    ));
    let removed = op(
        &srv,
        &srv.owner,
        "revoke-room-view",
        "spaces.remove_member",
        json!({"space_id":srv.shared,"account_id":guest_id(&srv)}),
    );
    assert_eq!(removed.status, 200, "{}", removed.body);
    let saved = op(
        &srv,
        &srv.owner,
        "revoked-room-write",
        "notes.update",
        json!({"id":item["id"],"base_revision":item["revision"],"markdown":"private edit"}),
    );
    assert_eq!(saved.status, 200, "{}", saved.body);
    let Frame::Text(left) = guest.read().unwrap() else {
        panic!("a revoked membership must not receive the edit");
    };
    assert_eq!(
        serde_json::from_str::<Value>(&left).unwrap(),
        json!({"type":"left","room":room,"membership":1,"reason":"access_revoked"})
    );
    guest.send(Frame::text(r#"{"type":"ping"}"#)).unwrap();
    assert_eq!(
        guest.read().unwrap().to_text().unwrap(),
        r#"{"type":"pong"}"#
    );
}

#[test]
fn item_only_edit_admits_a_room_and_last_grant_revocation_evicts_it() {
    let data = TempDir::new("live-room-item-grant");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let srv = start_full(&data.0, None, None, Some(&plugins));
    let note = create(&srv, "item-grant-room", &srv.personal, "original");
    let id = note["id"].as_str().unwrap();
    let item = json!({"kind":"notes","id":id});
    let grant = op(
        &srv,
        &srv.owner,
        "grant-room-edit",
        "sharing.grant",
        json!({"item":item,"audience":{"account_id":guest_id(&srv)},"permission":"edit"}),
    );
    assert_eq!(grant.status, 200, "{}", grant.body);
    let room = format!("{}/notes/{id}", srv.personal);
    let address = format!("@1/{room}");
    let mut guest = socket(srv.port, &srv.guest);
    let (doc, _) = join(&mut guest, &room, None);
    let text = doc.get_or_insert_text("md");
    let update = {
        let mut txn = doc.transact_mut();
        text.insert(&mut txn, 8, "!");
        txn.encode_update_v1()
    };
    send(
        &mut guest,
        &address,
        Message::Sync(SyncMessage::Update(update)),
    );
    send(&mut guest, &address, Message::Custom(100, vec![1]));
    loop {
        if let Message::Custom(101, ack) = recv(&mut guest, &address) {
            assert_eq!(ack, vec![1]);
            break;
        }
    }
    let removed = op(
        &srv,
        &srv.owner,
        "remove-grant-room-edit",
        "sharing.revoke",
        json!({"item":item,"grant_id":grant.body["data"]["grant_id"]}),
    );
    assert_eq!(removed.status, 200, "{}", removed.body);
    loop {
        if let Frame::Text(left) = guest.read().unwrap() {
            assert_eq!(
                serde_json::from_str::<Value>(&left).unwrap(),
                json!({"type":"left","room":room,"membership":1,"reason":"access_revoked"})
            );
            break;
        }
    }
}
