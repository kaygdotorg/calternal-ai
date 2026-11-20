use super::*;
use calternal_plugin_host::space_channel::SpaceMethod;
use serde_json::json;

#[test]
fn item_head_scope_cannot_be_widened_by_callback_input() {
    let id = Uuid::new_v4();
    assert!(item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":id})
    ));
    assert!(!item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":Uuid::new_v4()})
    ));
    assert!(!item_target_matches(
        "notes",
        id,
        &json!({"kind":"notes","item_id":id,"space_id":SpaceId::new()})
    ));
}

use calternal_auth::{
    db::GrantRevision,
    token::{hash32, normalize_setup_token},
};
use calternal_content::{Content, FileFormat, Item, Placement, Request};
use calternal_plugin_host::space_channel::{MAX_ACTIVE_TICKETS, SpaceChannel};
use tokio::net::{TcpListener, TcpStream};

#[cfg(target_os = "linux")]
#[path = "product_fixture.rs"]
mod product_fixture;

const CHILD: &str = r#"
setInterval(() => {}, 1000);
const lines = require('node:readline').createInterface({input:process.stdin});
const call = require('node:crypto').randomUUID();
const waiting = [];
let firstBurst = true;
const send = (invocation, message) => process.stdout.write(JSON.stringify({
  protocol_version:2, invocation_id:invocation, message
}) + '\n');
lines.on('line', line => {
  const f = JSON.parse(line), m = f.message;
  if ('ticket' in f || 'epoch' in f || 'space_id' in (m.input || {})) process.exit(8);
  if (m.type === 'invoke') {
    const callback = () => send(f.invocation_id, {
      type:'host_call', call_id:call, method:'item_head', input:m.input
    });
    if (m.operation === 'fixture.other' || !firstBurst) callback();
    else {
      waiting.push(callback);
      if (waiting.length === Number("$MAX_ACTIVE_TICKETS")) {
        firstBurst = false;
        waiting.splice(0).forEach(run => run());
      }
    }
  } else if (m.type === 'host_reply' && m.call_id === call && m.result.Ok) {
    send(f.invocation_id, {type:'complete',data:m.result.Ok});
  }
});
"#;

fn child_source() -> String {
    CHILD.replace("$MAX_ACTIVE_TICKETS", &MAX_ACTIVE_TICKETS.to_string())
}

struct Fixture {
    content: Arc<Content>,
    auth: Spaces,
    grants: GrantRevision,
    space: SpaceId,
    actor: AccountId,
    pin: PackagePin,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        let auth = Spaces::open(&path.join("auth.db")).unwrap();
        let ids = auth.seed_development().unwrap();
        let machines = auth.machines();
        let (code, _) = machines.issue_code(None, Duration::from_secs(30)).unwrap();
        let machine = machines
            .redeem_code(
                hash32(&normalize_setup_token(&code).unwrap()),
                vec![42; 32],
                "item head fixture".into(),
                "linux".into(),
                "x86_64".into(),
                false,
            )
            .unwrap()
            .unwrap();
        let space = ids.personal;
        let grants = auth.grant_revision(space, machine.id).unwrap();
        let content = Content::open(
            calternal_roots::Roots {
                appdata: path.join("appdata"),
                userdata_ssd: path.join("userdata-ssd"),
                userdata_hdd: path.join("userdata-hdd"),
            },
            path.join("secrets"),
            Duration::from_secs(30),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap();
        content
            .register_kind(
                space,
                "notes",
                Placement::Visible {
                    folder: "Notes".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        Self {
            _root: root,
            content: Arc::new(content),
            auth,
            grants,
            space,
            actor: ids.owner,
            pin: PackagePin {
                package: "calternal-head-fixture".into(),
                digest: sha256_hex(child_source().as_bytes()),
            },
        }
    }

    fn write(&self, id: Uuid) -> Item {
        self.content
            .create_assigned(
                self.space,
                id,
                "notes",
                json!({"title":"Fixture"}),
                b"# Retained body\n",
                Request {
                    account: self.actor,
                    via: None,
                    id: "head-fixture-item",
                    operation: "fixture.put",
                    fingerprint: "same checked payload",
                },
                |item| json!({"id": item.id, "revision": item.revision}),
            )
            .unwrap()
    }

    async fn carrier(&self) -> (Connected, SpaceChannel) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (connected, accepted) = tokio::join!(
            TcpStream::connect(listener.local_addr().unwrap()),
            listener.accept()
        );
        let assignment = Uuid::new_v4();
        let (host, runtime) = tokio::join!(
            SpaceChannel::host(
                accepted.unwrap().0,
                self.space,
                assignment,
                vec![self.pin.clone()]
            ),
            SpaceChannel::runtime(connected.unwrap(), self.space, assignment),
        );
        (
            Connected {
                space: self.space,
                channel: host.unwrap(),
                grants: self.grants.clone(),
                restore_consumed: tokio::sync::watch::channel(true).1,
            },
            runtime.unwrap(),
        )
    }

    fn scope(&self, channel: &SpaceChannel, item: Uuid) -> Scope {
        Scope {
            actor: self.actor,
            installation: None,
            package: self.pin.clone(),
            epoch: channel.epoch(),
            operation: "fixture.head".into(),
            kind: "notes".into(),
            item,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_an_old_ticket_and_admits_a_fresh_read_after_reconnect() {
    let fixture = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let (old, peer) = fixture.carrier().await;
    let target = Uuid::now_v7();
    let old_scope = fixture.scope(&old.channel, target);
    let (old_ticket, item) = tokio::task::block_in_place(|| {
        let admission = fixture.auth.invocation_reader(&old.grants).unwrap();
        let ticket = Ticket::mint(&admission, old_scope).unwrap();
        let rights = fixture
            .auth
            .capabilities(fixture.actor, fixture.space)
            .unwrap()
            .unwrap();
        assert!(rights.contains(Capability::ContentWrite));
        (ticket, fixture.write(target))
    });
    let old_epoch = old.channel.epoch();
    let prior_callback = calternal_plugin_host::space_channel::SpaceFrame {
        v: 1,
        epoch: old_epoch,
        seq: 1,
        package: Some(fixture.pin.package.clone()),
        digest: Some(fixture.pin.digest.clone()),
        ticket: Some(old_ticket.id()),
        message: SpaceMessage::Call {
            callback_id: Uuid::new_v4(),
            method: SpaceMethod::new("item_head").unwrap(),
            input: json!({"kind":"notes","item_id":item.id}),
        },
    };
    old_ticket
        .authorize_at(&prior_callback, Instant::now())
        .unwrap();
    drop((old, peer));

    let (fresh, mut runtime) = fixture.carrier().await;
    assert_ne!(fresh.channel.epoch(), old_epoch);
    assert!(
        old_ticket
            .authorize_at(
                &calternal_plugin_host::space_channel::SpaceFrame {
                    epoch: fresh.channel.epoch(),
                    ..prior_callback
                },
                Instant::now()
            )
            .is_err()
    );
    let scope = fixture.scope(&fresh.channel, item.id);
    let old_pin = fixture.pin.clone();
    let forged = tokio::spawn(async move {
        runtime.ready(&old_pin).unwrap();
        let admitted = runtime.receive().await.unwrap().frame().ticket.unwrap();
        assert_ne!(admitted, old_ticket.id());
        let callback = Uuid::new_v4();
        let input = json!({"kind":"notes","item_id":item.id});
        runtime
            .sender()
            .send_wait(
                &old_pin,
                old_ticket.id(),
                SpaceMessage::Call {
                    callback_id: callback,
                    method: SpaceMethod::new("item_head").unwrap(),
                    input: input.clone(),
                },
            )
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), runtime.receive())
                .await
                .is_err()
        );
        runtime
            .sender()
            .send_wait(
                &old_pin,
                admitted,
                SpaceMessage::Call {
                    callback_id: callback,
                    method: SpaceMethod::new("item_head").unwrap(),
                    input,
                },
            )
            .await
            .unwrap();
        let reply = runtime.receive().await.unwrap();
        let SpaceMessage::Reply { result, .. } = &reply.frame().message else {
            panic!("expected admitted reply")
        };
        assert_eq!(reply.frame().ticket, Some(admitted));
        runtime
            .sender()
            .send_wait(
                &old_pin,
                admitted,
                SpaceMessage::Complete {
                    result: result.clone(),
                },
            )
            .await
            .unwrap();
        assert!(runtime.receive().await.is_none());
    });
    let reader = Arc::new(
        tokio::task::block_in_place(|| fixture.content.item_head_reader(fixture.space)).unwrap(),
    );
    exchange(fresh, fixture.auth.clone(), reader.clone(), scope)
        .await
        .unwrap();
    forged.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ticket_in_flight_keeps_the_carrier_awake_past_idle() {
    let fixture = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let (connected, mut runtime) = fixture.carrier().await;
    let item = tokio::task::block_in_place(|| fixture.write(Uuid::now_v7()));
    let scope = fixture.scope(&connected.channel, item.id);
    let reader =
        tokio::task::block_in_place(|| fixture.content.item_head_reader(fixture.space)).unwrap();
    runtime.ready(&fixture.pin).unwrap();
    let idle = Duration::from_millis(150);
    let awake = Awake::attach(
        connected,
        fixture.auth.clone(),
        Arc::new(reader),
        &fixture.pin,
        None,
        idle,
    )
    .await
    .unwrap();
    let invoke = awake.clone();
    let caller = tokio::spawn(async move { invoke.invoke(scope).await });
    let ticket = runtime.receive().await.unwrap().frame().ticket.unwrap();
    tokio::time::sleep(idle * 2).await;
    assert!(awake.sender.is_live());
    let callback_id = Uuid::new_v4();
    runtime
        .sender()
        .send_wait(
            &fixture.pin,
            ticket,
            SpaceMessage::Call {
                callback_id,
                method: SpaceMethod::new("item_head").unwrap(),
                input: json!({"kind":"notes","item_id":item.id}),
            },
        )
        .await
        .unwrap();
    let reply = runtime.receive().await.unwrap();
    let SpaceMessage::Reply { result, .. } = &reply.frame().message else {
        panic!("expected reply")
    };
    runtime
        .sender()
        .send_wait(
            &fixture.pin,
            ticket,
            SpaceMessage::Complete {
                result: result.clone(),
            },
        )
        .await
        .unwrap();
    assert!(caller.await.unwrap().is_ok());
    tokio::time::timeout(Duration::from_secs(1), runtime.receive())
        .await
        .unwrap();
    assert!(!awake.sender.is_live());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_carrier_refuses_with_busy_before_checking_authority() {
    let fixture = tokio::task::spawn_blocking(Fixture::new).await.unwrap();
    let (connected, mut runtime) = fixture.carrier().await;
    let item = tokio::task::block_in_place(|| fixture.write(Uuid::now_v7()));
    let scope = fixture.scope(&connected.channel, item.id);
    let reader =
        tokio::task::block_in_place(|| fixture.content.item_head_reader(fixture.space)).unwrap();
    runtime.ready(&fixture.pin).unwrap();
    let awake = Awake::attach(
        connected,
        fixture.auth.clone(),
        Arc::new(reader),
        &fixture.pin,
        None,
        awake::IDLE,
    )
    .await
    .unwrap();
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..MAX_ACTIVE_TICKETS {
        let awake = awake.clone();
        let scope = scope.clone();
        calls.spawn(async move { awake.invoke(scope).await });
    }
    for _ in 0..MAX_ACTIVE_TICKETS {
        let incoming = tokio::time::timeout(Duration::from_secs(2), runtime.receive())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            incoming.frame().message,
            SpaceMessage::Invoke { .. }
        ));
    }
    let denied_actor = Scope {
        actor: AccountId::new(),
        ..scope
    };
    let busy = awake.invoke(denied_actor.clone()).await.unwrap_err();
    assert_eq!(busy.code, ErrorCode::Unavailable);
    assert_eq!(
        busy.details,
        Some(json!({"reason":"space_ticket_capacity"}))
    );
    calls.abort_all();
    while calls.join_next().await.is_some() {}
    assert_eq!(
        awake.invoke(denied_actor).await.unwrap_err().code,
        ErrorCode::Forbidden
    );
}
