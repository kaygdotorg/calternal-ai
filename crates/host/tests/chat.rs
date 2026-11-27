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
                slot_selection: None,
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
                slot_selection: None,
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
            json!({"item":f.add(),"text":"Hello"}),
            Some("coded"),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
}

#[test]
fn declares_chat_surfaces_and_evaluates_viewer_state_before_delivery() {
    let f = Fixture::new();
    let catalog = f.shell("s.declarations", json!({}));
    let chat = &catalog["plugins"][0];
    assert_eq!(catalog["space"], f.space.to_string());
    assert_eq!(
        chat["kinds"]["chats"]["operations"]["send"]["inputs"]["text"]["type"],
        "long text"
    );
    assert_eq!(
        chat["sidebars"]["chat"]["entries"][0]["collection"],
        "calternal-chat.pinned"
    );
    assert_eq!(chat["slots"]["composer"]["fillers"][0]["target"], "send");
    let id = f.add();
    let read =
        || f.shell("s.query", json!({"collection":"calternal-chat.chats"}))["rows"][0].clone();
    let before = read();
    assert_eq!(before["state"]["pinned"], false);
    assert_eq!(before["state"]["draft"], Value::Null);
    assert_eq!(before["when"]["pin"], true);
    assert_eq!(before["when"]["unpin"], false);
    f.shell(
        "s.state",
        json!({"op":"set","item":id,"key":"pinned","value":true}),
    );
    let after = read();
    assert_eq!(after["state"]["pinned"], true);
    assert_eq!(after["when"]["pin"], false);
    assert_eq!(after["when"]["unpin"], true);
    // Live refresh calls the same dispatcher. A different person's identity
    // cannot receive this personal row or any of its records.
    for (method, input) in [
        ("s.query", json!({"from":KIND})),
        ("s.read", json!({"item":id})),
        ("s.declarations", json!({})),
    ] {
        assert_eq!(
            f.call(f.second.into(), method, input, None)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }
}

#[test]
fn installing_another_plugin_refreshes_the_browser_catalog() {
    let f = Fixture::new();
    let initial = f.shell("s.declarations", json!({}));
    assert_eq!(initial["plugins"].as_array().unwrap().len(), 1);
    let extra = tempfile::tempdir().unwrap();
    std::fs::write(
        extra.path().join("plugin.md"),
        "---\nplugin: extra\nname: Extra\nversion: 1.0.0\nformat: 1\n---\n",
    )
    .unwrap();
    f.host
        .install_local(f.owner, f.space, extra.path(), false)
        .unwrap();
    let catalog = f.shell("s.declarations", json!({}));
    assert_eq!(catalog["plugins"][1]["id"], "extra");
}

#[test]
fn an_existing_personal_ledger_keeps_its_viewer_state() {
    let f = Fixture::new();
    let item = f.add();
    f.shell(
        "s.state",
        json!({"op":"set","item":item,"key":"draft","value":"Still here"}),
    );
    let Fixture {
        root,
        auth,
        host,
        owner,
        space,
        ..
    } = f;
    drop(host);
    let db = rusqlite::Connection::open(
        root.path()
            .join("state")
            .join(space.to_string())
            .join("ledger.sqlite3"),
    )
    .unwrap();
    db.execute_batch(
        "ALTER TABLE viewer_state RENAME TO scoped_viewer_state;
         CREATE TABLE viewer_state (
           plugin TEXT NOT NULL, item TEXT NOT NULL, name TEXT NOT NULL,
           value TEXT NOT NULL, ordinal INTEGER, PRIMARY KEY(plugin,item,name)
         );
         INSERT INTO viewer_state SELECT plugin,item,name,value,ordinal FROM scoped_viewer_state;
         DROP TABLE scoped_viewer_state;",
    )
    .unwrap();
    drop(db);

    let reopened = Host::new(root.path().into(), auth, 7).unwrap();
    let row = reopened
        .call(
            owner.into(),
            space,
            Call {
                call: "s.read".into(),
                input: json!({"item":item}),
                idempotency_key: None,
                slot_selection: None,
            },
        )
        .unwrap();
    assert_eq!(row["state"]["draft"], "Still here");
}

#[test]
fn batch_destinations_are_unique_and_case_only_rename_keeps_directory() {
    let f = Fixture::new();
    let add = || json!({"op":"add","kind":KIND,"fields":{"title":"Conversation"}});
    let added = f.shell("s.write", json!({"changes":[add(),add()]}));
    let first = &added["changes"][0]["id"];
    assert!(f.space_path().join("Chats/Conversation/chat.md").is_file());
    assert!(
        f.space_path()
            .join("Chats/Conversation 2/chat.md")
            .is_file()
    );
    f.plugin(
        json!({"changes":[{"op":"change","item":first,"fields":{"title":"CONVERSATION"}}]}),
        None,
    );
    let path = f.space_path().join("Chats/Conversation/chat.md");
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("CONVERSATION")
    );
    assert!(!f.space_path().join("Chats/CONVERSATION 3").exists());
}

#[test]
fn save_checks_cover_shell_and_worker_and_refuse_unwired_context() {
    let f = Fixture::new();
    let bundle = tempfile::tempdir().unwrap();
    std::fs::create_dir(bundle.path().join("kinds")).unwrap();
    std::fs::write(bundle.path().join("plugin.md"),
        "---\nplugin: review-tasks\nname: Review Tasks\nversion: 1.0.0\nformat: 1\nsends out: nothing\n---\nReview task estimates.\n").unwrap();
    let kind = |check: &str| {
        format!(
            "---\nkind: tasks\nname: Task\nformat: 1\nfolder: Tasks\neditable: outside\nfields:\n  estimate: number, required\nstate:\n  allowed: yes/no\nchecks:\n  {check}\n---\nTasks.\n"
        )
    };
    let path = bundle.path().join("kinds/tasks.md");
    std::fs::write(&path, kind("state.allowed")).unwrap();
    assert_eq!(
        f.host
            .install_local(f.owner, f.space, bundle.path(), false)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    std::fs::write(path, kind("estimate > 0, Use a positive estimate")).unwrap();
    f.host
        .install_local(f.owner, f.space, bundle.path(), false)
        .unwrap();
    let add = |n| json!({"op":"add","kind":"review-tasks.tasks","fields":{"estimate":n}});
    let item = f.shell("s.write", json!({"changes":[add(2)]}))["changes"][0]["id"].clone();
    for actor in [
        Actor::account(f.owner),
        Actor::plugin(f.owner, "review-tasks"),
    ] {
        for change in [
            add(-1),
            json!({"op":"change","item":item,"fields":{"estimate":-1}}),
        ] {
            let error = f
                .call(actor.clone(), "s.write", json!({"changes":[change]}), None)
                .unwrap_err();
            assert!(
                error.message.contains("Use a positive estimate"),
                "{error:?}"
            );
        }
    }
}
