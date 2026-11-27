//! Exercise slices 1 and 2 through the same dispatcher as the HTTP transport.
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Call, Host};
use calternal_types::{AccountId, Actor, ErrorCode, SpaceId};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const KIND: &str = "calternal-chat.chats";

struct Fixture {
    root: tempfile::TempDir,
    auth: Auth,
    host: Host,
    owner: AccountId,
    second: AccountId,
    space: SpaceId,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let auth = Auth::open(
            &root.path().join("auth"),
            AuthConfig::new("http://localhost").unwrap(),
        )
        .unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let owner = rt.block_on(auth.create("Ada")).unwrap().id;
        let second = rt.block_on(auth.create("Lin")).unwrap().id;
        let space = auth
            .spaces()
            .list(owner)
            .unwrap()
            .into_iter()
            .find(|s| s.personal)
            .unwrap()
            .id;
        let host = Host::new(root.path().into(), auth.clone(), 7).unwrap();
        let plugin =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins-v2/calternal-chat");
        host.install_local(owner, space, &plugin, true).unwrap();
        Self {
            root,
            auth,
            owner,
            second,
            space,
            host,
        }
    }

    fn call(
        &self,
        actor: Actor,
        call: &str,
        input: Value,
        key: Option<&str>,
    ) -> calternal_types::Result<Value> {
        self.host.call(
            actor,
            self.space,
            Call {
                call: call.into(),
                input,
                idempotency_key: key.map(str::to_owned),
            },
        )
    }

    fn shell(&self, name: &str, input: Value) -> Value {
        self.call(self.owner.into(), name, input, None).unwrap()
    }

    fn plugin(&self, input: Value, key: Option<&str>) -> Value {
        self.call(
            Actor::plugin(self.owner, "calternal-chat"),
            "s.write",
            input,
            key,
        )
        .unwrap()
    }

    fn add(&self) -> Value {
        self.shell(
            "s.write",
            json!({"changes":[{"op":"add","kind":KIND,"fields":{"title":"A small chat"}}]}),
        )["changes"][0]["id"]
            .clone()
    }

    fn space_path(&self) -> PathBuf {
        self.root.path().join("spaces").join(self.space.to_string())
    }
}

#[test]
fn installs_chat_adds_markdown_and_rebuilds_query() {
    let f = Fixture::new();
    let id = f.add();
    let collection = json!({"collection":"calternal-chat.chats"});
    let before = f.shell("s.query", collection.clone());
    assert_eq!(before["rows"][0]["id"], id);
    assert_eq!(before["rows"][0]["computed"]["unread"], 0);
    assert_eq!(
        std::fs::read_to_string(f.space_path().join("Chats/A small chat/chat.md")).unwrap(),
        "---\narchived: false\ntitle: \"A small chat\"\n---\n"
    );
    let installed = f
        .space_path()
        .join(".calternal/plugins/calternal-chat/operations.ts");
    assert!(Path::new(&installed).is_file());
    f.shell("index.rebuild", json!({}));
    assert_eq!(f.shell("s.query", collection.clone()), before);
    let outsider = f
        .call(f.second.into(), "s.query", collection, None)
        .unwrap_err();
    assert_eq!(outsider.code, ErrorCode::NotFound);
    let invalid = f
        .call(
            f.owner.into(),
            "s.write",
            json!({"changes":[
                {"op":"change","item":id,"fields":{"archived":true}},
                {"op":"change","item":id,"fields":{"title":"refused"}}
            ]}),
            None,
        )
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::Forbidden);
    assert_eq!(
        f.shell("s.read", json!({"item":id}))["fields"]["archived"],
        false
    );
}

#[test]
fn records_are_attributed_and_replays_do_not_append() {
    use std::io::Write;

    let f = Fixture::new();
    let id = f.add();
    let append =
        json!({"changes":[{"op":"append record","item":id,"record":{"body":"A message"}}]});
    let first = f.plugin(append.clone(), Some("first"));
    let anchor = first["changes"][0]["anchor"].as_str().unwrap();
    assert_eq!(
        first["changes"][0]["link"],
        format!("[Record](<Chats/A%20small%20chat/chat.md#^{anchor}>)")
    );
    assert_eq!(f.plugin(append, Some("first")), first);
    let data = f.shell("s.read", json!({"item":id}));
    assert_eq!(data["records"].as_array().unwrap().len(), 1);
    assert_eq!(data["records"][0]["author"], f.owner.to_string());
    assert_eq!(data["computed"]["unread"], 1);

    let refusal = f
        .call(
            f.second.into(),
            "s.write",
            json!({"changes":[
                {"op":"remove record","item":id,"anchor":first["changes"][0]["anchor"]}
            ]}),
            None,
        )
        .unwrap_err();
    assert_eq!(refusal.code, ErrorCode::NotFound);

    let path = f.space_path().join("Chats/A small chat/chat.md");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("**Ada** · ")
    );
    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    write!(
        file,
        "**{}** · 2026-11-27 14:03 ^0123456789abcdef\n\nPrivate?\n\n",
        f.owner
    )
    .unwrap();
    assert_eq!(
        f.shell("s.read", json!({"item":id}))["records"][1]["author"],
        "outside"
    );
    let error = f
        .call(
            Actor::plugin(f.owner, "calternal-chat"),
            "s.write",
            json!({"changes":[
                {"op":"append record","item":id,"record":{"body":"Later"}}
            ]}),
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[test]
fn restart_keeps_item_and_receipt() {
    let f = Fixture::new();
    let id = f.add();
    let append = json!({"changes":[{"op":"append record","item":id,"record":{"body":"Persist"}}]});
    let first = f.plugin(append.clone(), Some("persist"));
    let Fixture {
        root,
        auth,
        host,
        owner,
        space,
        ..
    } = f;
    drop(host);
    let host = Host::new(root.path().into(), auth, 7).unwrap();
    let response = host
        .call(
            Actor::plugin(owner, "calternal-chat"),
            space,
            Call {
                call: "s.write".into(),
                input: append,
                idempotency_key: Some("persist".into()),
            },
        )
        .unwrap();
    assert_eq!(response, first);
}

#[test]
fn thousand_appends() {
    let f = Fixture::new();
    let id = f.add();
    let one = |n| json!({"changes":[{"op":"append record","item":id,"record":{"body":format!("Message {n}")}}]});
    f.plugin(one(0), Some("warmup"));
    eprintln!("HOST_APPEND_BEGIN");
    for n in 1..=1_000 {
        f.plugin(one(n), Some(&format!("append-{n}")));
    }
    eprintln!("HOST_APPEND_END");
    assert_eq!(
        f.shell("s.read", json!({"item":id}))["records"]
            .as_array()
            .unwrap()
            .len(),
        1_001
    );
}

#[test]
fn unread_position_survives_deleting_the_marked_record() {
    let f = Fixture::new();
    let id = f.add();
    let append =
        |body| json!({"changes":[{"op":"append record","item":id,"record":{"body":body}}]});
    let first = f.plugin(append("First"), None)["changes"][0]["anchor"].clone();
    let second = f.plugin(append("Second"), None)["changes"][0]["anchor"].clone();
    let unread = || f.shell("s.read", json!({"item":id}))["computed"]["unread"].clone();
    assert_eq!(unread(), 2);
    f.shell(
        "s.state",
        json!({"op":"set","item":id,"key":"read through","value":first}),
    );
    assert_eq!(unread(), 1);
    f.shell(
        "s.write",
        json!({"changes":[{"op":"remove record","item":id,"anchor":first}]}),
    );
    assert_eq!(unread(), 1);
    f.shell(
        "s.state",
        json!({"op":"set","item":id,"key":"read through","value":second}),
    );
    assert_eq!(unread(), 0);
}

#[test]
fn named_query_sorts_pages_and_sees_a_coded_rename() {
    let f = Fixture::new();
    let one = f.add();
    let added = f.shell(
        "s.write",
        json!({"changes":[{"op":"add","kind":KIND,"fields":{"title":"B chat"}}]}),
    );
    let two = &added["changes"][0]["id"];
    let first = f.shell(
        "s.query",
        json!({"from":KIND,"where":"not archived","sort":["title desc"],"limit":1}),
    );
    assert_eq!(first["rows"][0]["id"], *two);
    let second = f.shell(
        "s.query",
        json!({"from":KIND,"where":"not archived","sort":["title desc"],"limit":1,"cursor":first["cursor"]}),
    );
    assert_eq!(second["rows"][0]["id"], one);
    assert_eq!(second["cursor"], Value::Null);
    f.plugin(
        json!({"changes":[{"op":"change","item":one,"fields":{"title":"Renamed"}}]}),
        None,
    );
    assert!(f.space_path().join("Chats/Renamed/chat.md").is_file());
    assert!(!f.space_path().join("Chats/A small chat/chat.md").exists());
    assert_eq!(
        f.shell("s.read", json!({"item":one}))["fields"]["title"],
        "Renamed"
    );
}

#[test]
fn code_dispatch_requires_an_isolated_runtime() {
    let f = Fixture::new();
    let error = f
        .call(
            f.owner.into(),
            "calternal-chat.chats.send",
            json!({"chat":f.add(),"text":"Hello"}),
            Some("coded"),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
}
