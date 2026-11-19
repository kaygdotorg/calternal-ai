//! Live room admission and revocation through the same cookie-authenticated HTTP upgrade as the browser.

use super::*;
use std::net::TcpStream;
use tungstenite::{Message as Frame, WebSocket, client::IntoClientRequest, stream::MaybeTlsStream};
use yrs::{
    Doc, Text, Transact, Update,
    encoding::{
        read::{Cursor, Read},
        write::Write,
    },
    sync::{Message, SyncMessage},
    updates::{decoder::Decode, encoder::Encode},
};

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

struct SharedNote {
    id: String,
    room: String,
    revision: Value,
    grant: Value,
}

impl SharedNote {
    async fn create(f: &Fixture) -> Self {
        // A common group makes the guest discoverable without granting space access.
        let group = f
            .op(0, "groups.create", json!({"name":"Room recipients"}))
            .await;
        group.ok();
        for account in [&f.ids[1], &f.ids[2]] {
            f.op(
                0,
                "groups.add_member",
                json!({"group_id":group.body["data"]["group_id"],"account_id":account}),
            )
            .await
            .ok();
        }
        let space = &f.spaces[0];
        let note = f.note(1, space).await;
        let id = note["id"].as_str().unwrap().to_owned();
        let grant = f.op(
            1,
            "sharing.grant",
            json!({"item":{"kind":"notes","id":id},"audience":{"account_id":f.ids[2]},"permission":"edit"}),
        )
        .await;
        grant.ok();
        Self {
            room: format!("{space}/notes/{id}"),
            id,
            revision: note["revision"].clone(),
            grant: grant.body["data"]["grant_id"].clone(),
        }
    }

    async fn revoke(&self, f: &Fixture) {
        f.op(
            1,
            "sharing.revoke",
            json!({"item":{"kind":"notes","id":self.id},"grant_id":self.grant}),
        )
        .await
        .ok();
    }
}

struct Connector {
    endpoint: String,
    cookie: String,
}

impl Connector {
    fn new(f: &Fixture, who: usize) -> Self {
        Self {
            endpoint: format!("{}/api/v1/live", f.base.replacen("http://", "ws://", 1)),
            cookie: f.cookies[who].clone(),
        }
    }

    fn open(&self) -> Socket {
        let mut request = self.endpoint.as_str().into_client_request().unwrap();
        let headers = request.headers_mut();
        for (name, value) in [
            ("host", "localhost:18080"),
            ("origin", ORIGIN),
            ("cookie", self.cookie.as_str()),
            ("sec-websocket-protocol", "calternal-live-rooms.v1"),
        ] {
            headers.insert(name, value.parse().unwrap());
        }
        let (mut socket, response) = tungstenite::connect(request).unwrap();
        assert_eq!(
            response.headers()["sec-websocket-protocol"],
            "calternal-live-rooms.v1"
        );
        let MaybeTlsStream::Plain(stream) = socket.get_mut() else {
            panic!("the fixture uses loopback HTTP");
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
    }
}

fn control(socket: &mut Socket) -> Value {
    let frame = socket.read().unwrap();
    let Frame::Text(value) = frame else {
        panic!("expected room control, received {frame:?}");
    };
    serde_json::from_str(value.as_str()).unwrap()
}

fn join(socket: &mut Socket, room: &str, membership: u64, lineage: Option<&str>) -> Value {
    socket
        .send(Frame::text(
            json!({
                "type":"join","room":room,"membership":membership,"lineage":lineage,
            })
            .to_string(),
        ))
        .unwrap();
    control(socket)
}

fn joined(socket: &mut Socket, room: &str) -> String {
    let reply = join(socket, room, 1, None);
    assert_eq!(reply["type"], "joined", "{reply}");
    assert_eq!(reply["room"], room);
    assert_eq!(reply["info"]["text"], "md");
    reply["info"]["lineage"].as_str().unwrap().to_owned()
}

fn ping(socket: &mut Socket) {
    socket.send(Frame::text(r#"{"type":"ping"}"#)).unwrap();
    assert_eq!(control(socket), json!({"type":"pong"}));
}

fn read_initial(socket: &mut Socket, room: &str) -> Doc {
    let doc = Doc::new();
    let address = format!("@1/{room}");
    loop {
        let frame = socket.read().unwrap();
        let Frame::Binary(bytes) = frame else {
            panic!("expected initial room state, received {frame:?}");
        };
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(cursor.read_string().unwrap(), address);
        match Message::decode_v1(&bytes[cursor.next..]).unwrap() {
            Message::Sync(SyncMessage::SyncStep2(update)) => {
                doc.transact_mut()
                    .apply_update(Update::decode_v1(&update).unwrap())
                    .unwrap();
            }
            Message::Awareness(_) => return doc,
            other => assert!(matches!(other, Message::Sync(SyncMessage::SyncStep1(_)))),
        }
    }
}

#[tokio::test]
async fn live_room_joins_with_real_owner_and_item_grantee_sessions() {
    let f = Fixture::new().await;
    let note = SharedNote::create(&f).await;
    let owner = Connector::new(&f, 1);
    let guest = Connector::new(&f, 2);
    tokio::task::spawn_blocking(move || {
        let mut owner = owner.open();
        let mut guest = guest.open();
        assert_eq!(
            joined(&mut owner, &note.room),
            joined(&mut guest, &note.room)
        );
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn live_room_reconnect_rechecks_the_revoked_item_grant() {
    let f = Fixture::new().await;
    let note = SharedNote::create(&f).await;
    let guest = Connector::new(&f, 2);
    let room = note.room.clone();
    let (guest, lineage) = tokio::task::spawn_blocking(move || {
        let mut socket = guest.open();
        let lineage = joined(&mut socket, &room);
        read_initial(&mut socket, &room);
        socket.close(None).unwrap();
        (guest, lineage)
    })
    .await
    .unwrap();

    note.revoke(&f).await;
    let hidden = f.op(2, "notes.get", json!({"id":note.id})).await;
    assert_eq!(hidden.status, 404, "{}", hidden.body);
    tokio::task::spawn_blocking(move || {
        let mut socket = guest.open();
        let reply = join(&mut socket, &note.room, 2, Some(&lineage));
        assert_eq!(reply["type"], "join_failed", "{reply}");
        assert_eq!(reply["membership"], 2);
        assert_eq!(reply["error"]["code"], "not_found");
        // Any initial state delivered after the refusal would arrive before the pong.
        ping(&mut socket);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn live_room_last_grant_revocation_ends_online_reads_and_writes() {
    let f = Fixture::new().await;
    let note = SharedNote::create(&f).await;
    let guest = Connector::new(&f, 2);
    let room = note.room.clone();
    let (mut socket, doc) = tokio::task::spawn_blocking(move || {
        let mut socket = guest.open();
        joined(&mut socket, &room);
        let doc = read_initial(&mut socket, &room);
        (socket, doc)
    })
    .await
    .unwrap();

    note.revoke(&f).await;
    let saved = f.op(
        1,
        "notes.update",
        json!({"id":note.id,"base_revision":note.revision,"markdown":"owner edit after revoke"}),
    ).await;
    saved.ok();
    let room = note.room.clone();
    tokio::task::spawn_blocking(move || {
        let text = doc.get_or_insert_text("md");
        let mut txn = doc.transact_mut();
        text.insert(&mut txn, 0, "guest edit after revoke");
        let mut bytes = Vec::new();
        bytes.write_string(&format!("@1/{room}"));
        bytes.extend(Message::Sync(SyncMessage::Update(txn.encode_update_v1())).encode_v1());
        // Submit before processing the server's eviction, as a client with queued input can.
        socket.send(Frame::binary(bytes)).unwrap();
        assert_eq!(
            control(&mut socket),
            json!({
                "type":"left","room":room,"membership":1,"reason":"access_revoked",
            })
        );
        // Any room update sent to the guest would arrive before this pong.
        ping(&mut socket);
    })
    .await
    .unwrap();

    let read = f.op(1, "notes.get", json!({"id":note.id})).await;
    read.ok();
    assert_eq!(
        read.body["data"]["note"]["markdown"],
        "owner edit after revoke"
    );
}
