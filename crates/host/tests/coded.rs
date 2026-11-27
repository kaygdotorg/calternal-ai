//! Exercise the real storage route and revision 9 Chat bundle in the OS sandbox.
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Call, Host, WorkerRuntime};
use calternal_types::{AccountId, Actor, ErrorCode, SpaceId};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Fixture {
    _root: tempfile::TempDir,
    host: Arc<Host>,
    owner: AccountId,
    space: SpaceId,
}

fn checkout() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let auth_dir = root.path().join("auth");
        let auth = tokio::task::spawn_blocking(move || {
            Auth::open(&auth_dir, AuthConfig::new("http://localhost").unwrap())
        })
        .await
        .unwrap()
        .unwrap();
        let owner = auth.create("Ada").await.unwrap().id;
        let directory = auth.spaces();
        let space = tokio::task::spawn_blocking(move || directory.list(owner))
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .find(|s| s.personal)
            .unwrap()
            .id;
        let program = |name: &str| {
            PathBuf::from(
                std::env::var_os(name).expect("set sandbox and Node paths for the coded host gate"),
            )
        };
        let runtime = WorkerRuntime::new(
            program("CALTERNAL_TEST_BWRAP"),
            program("CALTERNAL_TEST_NODE"),
            checkout().join("packages/plugin-sdk"),
        )
        .unwrap();
        let host = Arc::new(
            Host::new(root.path().into(), auth, 7)
                .unwrap()
                .with_worker_runtime(runtime),
        );
        Self {
            _root: root,
            host,
            owner,
            space,
        }
    }

    async fn install(&self, path: PathBuf) -> calternal_types::Result<()> {
        let (host, owner, space) = (self.host.clone(), self.owner, self.space);
        tokio::task::spawn_blocking(move || host.install_local(owner, space, &path, true))
            .await
            .unwrap()
    }

    async fn call(
        &self,
        actor: Actor,
        call: &str,
        input: Value,
        key: Option<&str>,
    ) -> calternal_types::Result<Value> {
        self.host
            .clone()
            .trusted_call(
                actor,
                self.space,
                Call {
                    call: call.into(),
                    input,
                    idempotency_key: key.map(str::to_owned),
                },
            )
            .await
    }

    async fn chat(&self) -> Value {
        self.call(self.owner.into(), "s.write", json!({"changes":[{"op":"add","kind":"calternal-chat.chats","fields":{"title":"A small chat"}}]}), None)
            .await.unwrap()["changes"][0]["id"].clone()
    }
}

#[tokio::test]
#[ignore = "requires an explicit Bubblewrap/Node launcher and built SDK"]
async fn real_chat_send_rename_and_replay() {
    let f = Fixture::new().await;
    f.install(checkout().join("plugins-v2/calternal-chat"))
        .await
        .unwrap();
    let item = f.chat().await;
    let owner = Actor::account(f.owner);
    f.call(
        owner.clone(),
        "calternal-chat.chats.send",
        json!({"chat":item,"text":"Prime the append cache"}),
        Some("prime"),
    )
    .await
    .unwrap();
    let input = json!({"chat":item,"text":"Hello"});
    eprintln!("CODED_APPEND_BEGIN");
    let first = f
        .call(
            owner.clone(),
            "calternal-chat.chats.send",
            input.clone(),
            Some("gesture"),
        )
        .await
        .unwrap();
    eprintln!("CODED_APPEND_END");
    assert_eq!(
        f.call(
            owner.clone(),
            "calternal-chat.chats.send",
            input,
            Some("gesture")
        )
        .await
        .unwrap(),
        first
    );
    let version = f
        .call(owner.clone(), "s.read", json!({"item":item}), None)
        .await
        .unwrap();
    assert_eq!(version["records"].as_array().unwrap().len(), 2);
    let anchor = first["changes"][0]["anchor"].as_str().unwrap();
    assert_eq!(
        first["changes"][0]["link"],
        format!("[Record](<Chats/A%20small%20chat/chat.md#^{anchor}>)")
    );
    assert_eq!(version["records"][1]["author"], json!(f.owner));
    assert_eq!(version["records"][1]["body"], "Hello");
    let error = f
        .call(
            owner.clone(),
            "calternal-chat.chats.send",
            json!({"chat":item,"text":"Changed input"}),
            Some("gesture"),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    f.call(
        owner.clone(),
        "calternal-chat.chats.rename",
        json!({"chat":item,"title":"Renamed"}),
        None,
    )
    .await
    .unwrap();
    let read = f
        .call(owner.clone(), "s.read", json!({"item":item}), None)
        .await
        .unwrap();
    assert_eq!(read["fields"]["title"], "Renamed");
    let denied = f
        .call(
            owner,
            "calternal-chat.chats.undeclared",
            json!({"chat":item}),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(denied.code, ErrorCode::Forbidden);
}

fn observer(root: &Path) {
    std::fs::create_dir(root.join("kinds")).unwrap();
    std::fs::write(root.join("plugin.md"), "---\nplugin: observer\nname: Observer\nversion: 1.0.0\nformat: 1\nreads: calternal-chat.chats\ncalls: calternal-chat.chats.rename\n---\n").unwrap();
    std::fs::write(root.join("kinds/observations.md"), "---\nkind: observations\nname: Observation\nformat: 1\nfolder: Observations\neditable: operations\nfields:\n  title: text, required\n  weight: number\nstate:\n  seen: yes/no\noperations:\n  add: title\n  relay: code\n---\nRecords observations of committed changes.\n").unwrap();
    std::fs::write(root.join("operations.ts"), r#"
        import { on } from "@calternal/plugin-sdk";
        export const observe = on.change("collection:calternal-chat.chats", async (_item, s) => {
            const page = await s.query({ from: "calternal-chat.chats" });
            const item = await s.read(s.event.item!);
            const receipt = await s.write([{ add: { kind: "observer.observations", fields: {
                title: [s.me.id, s.space.id, item.records.length, page.rows.length].join("/"), weight: 0.25
            } } }]);
            const created = receipt.changes[0].id;
            await s.state.set(created, "seen", true);
            if (await s.state.get(created, "seen") !== true) throw Error("state not visible");
        });
        export const relay = on.call("observations.relay", async ({ chat }: { chat: string }, s) =>
            s.call("calternal-chat.chats.rename", { chat, title: "Relay" }));
    "#).unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit Bubblewrap/Node launcher and built SDK"]
async fn install_probe_registers_a_change_on_the_real_route() {
    let f = Fixture::new().await;
    f.install(checkout().join("plugins-v2/calternal-chat"))
        .await
        .unwrap();
    let chat = f.chat().await;
    let bundle = tempfile::tempdir().unwrap();
    observer(bundle.path());
    f.install(bundle.path().to_owned()).await.unwrap();
    f.call(
        f.owner.into(),
        "calternal-chat.chats.send",
        json!({"chat":chat,"text":"Hello"}),
        Some("observed"),
    )
    .await
    .unwrap();
    let rows = f
        .call(
            f.owner.into(),
            "s.query",
            json!({"from":"observer.observations"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(rows["rows"].as_array().unwrap().len(), 1);
    let id = &rows["rows"][0]["id"];
    assert_eq!(rows["rows"][0]["fields"]["weight"], json!(0.25));
    assert_eq!(
        rows["rows"][0]["fields"]["title"],
        format!("{}/{}/1/1", f.owner, f.space)
    );
    let state = f
        .call(
            f.owner.into(),
            "s.state",
            json!({"op":"get","item":id,"key":"seen"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(state["value"], true);
    f.call(
        f.owner.into(),
        "observer.observations.relay",
        json!({"chat":chat}),
        Some("relay"),
    )
    .await
    .unwrap();
    let item = f
        .call(f.owner.into(), "s.read", json!({"item":chat}), None)
        .await
        .unwrap();
    assert_eq!(item["fields"]["title"], "Relay");
    let rows = f
        .call(
            f.owner.into(),
            "s.query",
            json!({"from":"observer.observations"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        rows["rows"].as_array().unwrap().len(),
        1,
        "a reaction cannot see its own call-chain changes"
    );
}

#[tokio::test]
#[ignore = "requires an explicit Bubblewrap/Node launcher and built SDK"]
async fn child_exit_cannot_leave_half_of_a_real_write() {
    let f = Fixture::new().await;
    let bundle = tempfile::tempdir().unwrap();
    fn copy(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    copy(&checkout().join("plugins-v2/calternal-chat"), bundle.path());
    let kind = bundle.path().join("kinds/chats.md");
    let kind_text = std::fs::read_to_string(&kind).unwrap();
    std::fs::write(
        kind,
        kind_text.replace("  rename: code", "  crash: code\n  rename: code"),
    )
    .unwrap();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(bundle.path().join("operations.ts"))
        .unwrap()
        .write_all(
            br#"
            export const crash = on.call("chats.crash", async ({ chat }: {chat: ItemId}, s) => {
                await Promise.all([
                    s.write([
                        { change: { item: chat, fields: { archived: true } } },
                        { "append record": { item: chat, record: { body: "All or nothing" } } }
                    ]),
                    new Promise(() => setTimeout(() => process.exit(23), 20))
                ]);
            });
        "#,
        )
        .unwrap();
    f.install(bundle.path().to_owned()).await.unwrap();
    let item = f.chat().await;
    let error = f
        .call(
            f.owner.into(),
            "calternal-chat.chats.crash",
            json!({"chat":item}),
            Some("crash"),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
    let data = f
        .call(f.owner.into(), "s.read", json!({"item":item}), None)
        .await
        .unwrap();
    assert_eq!(
        data["records"].as_array().unwrap().len(),
        usize::from(data["fields"]["archived"] == true)
    );
    f.call(
        f.owner.into(),
        "calternal-chat.chats.rename",
        json!({"chat":item,"title":"Recovered"}),
        None,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit Bubblewrap/Node launcher and built SDK"]
async fn chat_harness_refusal_reuses_the_record_receipt() {
    let f = Fixture::new().await;
    f.install(checkout().join("plugins-v2/calternal-chat"))
        .await
        .unwrap();
    let item = f.chat().await;
    for _ in 0..2 {
        let error = f.call(f.owner.into(), "calternal-chat.chats.send",
            json!({"chat":item,"text":"For the harness","harness":"calternal-chat-claude-code.runs.start"}),
            Some("send-start")).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Forbidden);
    }
    let data = f
        .call(f.owner.into(), "s.read", json!({"item":item}), None)
        .await
        .unwrap();
    assert_eq!(data["records"].as_array().unwrap().len(), 1);
}
