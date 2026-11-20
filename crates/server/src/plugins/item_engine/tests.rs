use crate::plugins::item_engine::{
    AttemptText, CapturedInput, ItemAccess, ItemAuthority, ItemCheck, ItemEngine, ItemStages,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::Content;
use calternal_plugin_host::ManifestV2;
use calternal_roots::Roots;
use calternal_types::{AccountId, ErrorCode, ItemRef, RequestId, RevisionRef, SpaceId};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    content: Arc<Content>,
    space: SpaceId,
    package: Arc<ManifestV2>,
    account: AccountId,
    authority: ItemAuthority,
    deny_delivery: Arc<AtomicBool>,
    staging: Arc<ItemStages>,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let content = Arc::new(
            Content::open(
                Roots {
                    appdata: root.join("appdata"),
                    userdata_ssd: root.join("userdata-ssd"),
                    userdata_hdd: root.join("userdata-hdd"),
                },
                root.join("secrets"),
                Duration::from_secs(30),
                calternal_content::DEFAULT_RECEIPT_RETENTION,
                |_| {},
            )
            .unwrap(),
        );
        let space = SpaceId::new();
        let manifest: Value = serde_json::from_str(include_str!(
            "../../../../../contract/fixtures/description-v2/manifests/reading-list-manifest-v2-app.json"
        ))
        .unwrap();
        let mut kind = manifest["kinds"][0].clone();
        kind["append_file"] = json!(true);
        kind["attempt_discovery"] = json!("item_policy");
        kind["viewer_fields"] = json!({
            "last_read": {"type":"Timestamp"},
            "pinned": {"type":"Boolean"},
            "unread": {"type":"Number","source":"entry_unread"}
        });
        let mut operation = manifest["operations"][0].clone();
        operation["methods"] = json!([
            "item_create",
            "item_head",
            "item_range",
            "item_spans",
            "item_save",
            "item_append",
            "item_save_begin",
            "item_save_chunk",
            "item_save_seal",
            "viewer_state_read",
            "viewer_state_write"
        ]);
        operation["write_origins"] = json!({"host_input":"title","attempt_output":"text"});
        let mut declaration = manifest;
        declaration["kinds"][0] = kind;
        declaration["operations"] = json!([operation]);
        // Keep only the declarations this single-operation fixture uses.
        for unused in ["schedule_eligibility", "jobs", "wait_webhooks", "alarms"] {
            declaration.as_object_mut().unwrap().remove(unused);
        }
        let package =
            Arc::new(ManifestV2::parse(&serde_json::to_vec(&declaration).unwrap()).unwrap());
        for (kind, placement, format, live) in super::kind_registrations(&package).unwrap() {
            content
                .register_kind(space, &kind, placement, format)
                .unwrap();
            assert!(!live);
        }
        let deny_delivery = Arc::new(AtomicBool::new(false));
        let denied = deny_delivery.clone();
        let authority: ItemAuthority = Arc::new(move |access: ItemAccess| {
            if denied.load(Ordering::Acquire) && access.check == ItemCheck::Deliver {
                Err(calternal_types::Error::forbidden("delivery revoked"))
            } else {
                Ok(())
            }
        });
        Self {
            _temp: temp,
            content,
            space,
            package,
            account: AccountId::new(),
            authority,
            deny_delivery,
            staging: Arc::new(ItemStages::new(space)),
        }
    }

    fn engine(&self, invocation: &str) -> ItemEngine {
        self.engine_for(self.account, invocation)
    }

    fn engine_for(&self, account: AccountId, invocation: &str) -> ItemEngine {
        ItemEngine::new(
            self.content.clone(),
            self.space,
            account,
            self.package.clone(),
            "reading.add",
            "reading-item",
            RequestId(invocation.to_owned()),
            self.authority.clone(),
        )
        .unwrap()
        .with_stages(self.staging.clone())
        .unwrap()
    }

    fn discoverable(&self, item: Uuid) -> bool {
        self.content
            .creator_discoverable(self.space, item, "reading-item", self.account)
            .unwrap()
    }
}

fn reply(text: &str) -> super::AttemptOutputs {
    let text = text.to_owned();
    Arc::new(move |seq| {
        assert_eq!(seq, 1);
        Ok(AttemptText {
            text: text.clone(),
            attempt: "attempt-1".into(),
        })
    })
}

#[test]
fn plugin_item_exchanges_replay_and_preserve_the_prior_range() {
    let fixture = Fixture::new();
    let (content, space, account) = (&fixture.content, fixture.space, fixture.account);
    let mut engine = fixture
        .engine("invocation")
        .with_captured_input(CapturedInput {
            text: "Hello".into(),
            target: None,
        })
        .with_attempt_outputs(reply("!"));
    let create = json!({
        "request_id":"new", "kind":"reading-item", "folders":["Example"], "name":"Conversation.md",
        "metadata":{"title":"Conversation"}, "attempt_discovery":"creator_only",
        "attested":{"source":{"kind":"input"},"prefix":"# ","suffix":"\n","entry_anchor":"m-1"}
    });
    let created = engine.call("item_create", &create).unwrap();
    assert_eq!(engine.call("item_create", &create).unwrap(), created);
    let item = created["item"].clone();
    assert_ne!(item, created["locator"]["id"]);
    let local = Uuid::parse_str(created["locator"]["id"].as_str().unwrap()).unwrap();
    assert!(fixture.discoverable(local));
    assert!(
        !content
            .creator_discoverable(space, local, "reading-item", AccountId::new())
            .unwrap()
    );
    assert_eq!(
        engine.created(&json!({"created":item})).unwrap(),
        Some(ItemRef(item.as_str().unwrap().to_owned()))
    );
    fixture.deny_delivery.store(true, Ordering::Release);
    assert!(engine.created(&json!({"created":item})).is_err());
    fixture.deny_delivery.store(false, Ordering::Release);
    assert!(
        fixture
            .engine("different invocation")
            .created(&json!({"created":item}))
            .is_err()
    );
    let head = engine.call("item_head", &json!({"item":item})).unwrap();
    assert_eq!(head["bytes"], 8);
    let before = head["revision"].clone();
    let append = json!({
        "request_id":"reply", "item":item, "expected_revision":before,
        "attested":{"source":{"kind":"output","seq":1},"prefix":"> ","entry_anchor":"m-2"}
    });
    let added = engine.call("item_append", &append).unwrap();
    assert_eq!(added["end_offset"], 11);
    assert!(added["span"].is_string());
    assert_eq!(engine.call("item_append", &append).unwrap(), added);
    assert!(fixture.discoverable(local));
    assert_eq!(
        engine
            .call(
                "item_range",
                &json!({"item":item,"revision":before,"offset":0,"length":64})
            )
            .unwrap(),
        json!({"bytes":STANDARD.encode("# Hello\n")})
    );
    let spans = engine
        .call(
            "item_spans",
            &json!({"item":item,"revision":added["revision"]}),
        )
        .unwrap();
    assert_eq!(
        spans,
        json!({
            "item": item, "revision": added["revision"], "body_provenance": "attested",
            "spans": [
                {"span": created["span"], "offset": 0, "length": 8, "middle_offset": 2,
                 "middle_length": 5, "attribution": {"kind":"person","actor":account}},
                {"span": added["span"], "offset": 8, "length": 3, "middle_offset": 10,
                 "middle_length": 1, "attribution": {"kind":"attempt","attempt":"attempt-1"}},
            ]
        })
    );
    let old = engine
        .call("item_spans", &json!({"item":item,"revision":before}))
        .unwrap();
    assert_eq!(old["spans"].as_array().unwrap().len(), 1);
    assert_eq!(
        engine
            .call(
                "item_spans",
                &json!({"item":item,"revision":before,"cursor":"0.00"})
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert!(
        engine
            .call("item_head", &json!({"item":created["locator"]["id"]}))
            .is_err()
    );
    // A reused request with changed material conflicts before any effect.
    assert!(
        engine
            .call(
                "item_append",
                &json!({
                    "request_id":"reply", "item":item, "expected_revision":before, "bytes":"Pw=="
                })
            )
            .is_err()
    );
    let mut other_reply = fixture
        .engine("invocation")
        .with_attempt_outputs(reply("?"));
    assert_eq!(
        other_reply.call("item_append", &append).unwrap_err().code,
        ErrorCode::Conflict
    );
    // A reused anchor fails before effects.
    let duplicate = json!({
        "request_id":"again", "item":item, "expected_revision":added["revision"],
        "attested":{"source":{"kind":"output","seq":1},"entry_anchor":"m-2"}
    });
    assert_eq!(
        engine.call("item_append", &duplicate).unwrap_err().code,
        ErrorCode::Conflict
    );

    let mut discovery = fixture.engine("invocation").for_attempt_discovery();
    let reference = ItemRef(item.as_str().unwrap().to_owned());
    let prior = RevisionRef(before.as_str().unwrap().to_owned());
    discovery.pin_revision(&reference, prior.clone()).unwrap();
    discovery
        .authorize_range_delivery(&reference, &prior)
        .unwrap();

    let expected_bytes = b"\"\\\t\n".repeat(2 * 1024 * 1024 / 4);
    let begin = json!({
        "request_id":"stage", "item":item, "expected":added["revision"], "length":expected_bytes.len(),
        "sha256":crate::store::sha256_hex(&expected_bytes)
    });
    let handle = engine.call("item_save_begin", &begin).unwrap()["handle"].clone();
    assert_eq!(
        engine.call("item_save_begin", &begin).unwrap(),
        json!({"handle":handle})
    );
    for (index, bytes) in expected_bytes.chunks(65536).enumerate() {
        let offset = index * 65536;
        let chunk = json!({"handle":handle,"offset":offset,"bytes":STANDARD.encode(bytes)});
        assert_eq!(
            engine.call("item_save_chunk", &chunk).unwrap(),
            json!({"next_offset":offset + bytes.len()})
        );
        if index == 0 {
            assert_eq!(
                engine.call("item_save_chunk", &chunk).unwrap(),
                json!({"next_offset":65536})
            );
        }
    }
    assert!(
        engine
            .call(
                "item_save_chunk",
                &json!({"handle":handle,"offset":0,"bytes":"IQ=="})
            )
            .is_err()
    );
    let staged = engine
        .call("item_save_seal", &json!({"handle":handle}))
        .unwrap()["staged"]
        .clone();
    let save =
        json!({"request_id":"stage","item":item,"expected":added["revision"],"staged":staged});
    let saved = engine.call("item_save", &save).unwrap();
    assert_eq!(engine.call("item_save", &save).unwrap(), saved);
    assert_eq!(engine.call("item_save_begin", &begin).unwrap(), saved);
    let mut changed_save = save;
    changed_save["patch"] = json!({"title":"changed"});
    assert!(engine.call("item_save", &changed_save).is_err());
    let mut changed = begin;
    changed["length"] = json!(5);
    assert!(engine.call("item_save_begin", &changed).is_err());
    // A staged plugin-built body is opaque: protected, no live spans, and
    // the formerly attested revisions keep their own recorded spans.
    assert!(!fixture.discoverable(local));
    assert!(
        discovery
            .authorize_range_delivery(&reference, &prior)
            .is_err()
    );
    assert!(discovery.call("item_head", &json!({"item": item})).is_err());
    let after = engine
        .call(
            "item_spans",
            &json!({"item":item,"revision":saved["revision"]}),
        )
        .unwrap();
    assert_eq!(after["body_provenance"], "unproven");
    assert_eq!(after["spans"], json!([]));
    assert_eq!(
        engine
            .call(
                "item_spans",
                &json!({"item":item,"revision":added["revision"]}),
            )
            .unwrap(),
        spans
    );
    let mut rebuilt = Vec::new();
    let mut offset = 0;
    loop {
        let page = engine
            .call(
                "item_range",
                &json!({
                    "item":item,"revision":saved["revision"],"offset":offset,"length":65536
                }),
            )
            .unwrap();
        rebuilt.extend(STANDARD.decode(page["bytes"].as_str().unwrap()).unwrap());
        let Some(next) = page["next_offset"].as_u64() else {
            break;
        };
        offset = next;
    }
    assert_eq!(rebuilt, expected_bytes);
}

#[test]
fn attested_writes_refuse_unproven_sources_and_raw_writes_protect() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine("invocation");
    let attested = json!({
        "request_id":"a", "kind":"reading-item", "name":"A.md", "metadata":{"title":"A"},
        "attested":{"source":{"kind":"input"}}
    });
    let refused = engine.call("item_create", &attested).unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert_eq!(refused.details, Some(json!({"reason":"ineligible_source"})));
    let mut quoting = attested.clone();
    quoting["attested"]["quote"] = json!({"item":"x","revision":"y","span":"z"});
    assert!(engine.call("item_create", &quoting).is_err());
    let raw = engine
        .call(
            "item_create",
            &json!({
                "request_id":"b", "kind":"reading-item", "name":"B.md", "metadata":{"title":"B"},
                "body":"SGVsbG8=", "attempt_discovery":"creator_only"
            }),
        )
        .unwrap();
    let local = Uuid::parse_str(raw["locator"]["id"].as_str().unwrap()).unwrap();
    assert!(!fixture.discoverable(local));
    assert!(raw.get("span").is_none());
    let spans = engine
        .call(
            "item_spans",
            &json!({"item":raw["item"],"revision":raw["revision"]}),
        )
        .unwrap();
    assert_eq!(spans["body_provenance"], "unproven");
    // A capture for another item cannot attest this one.
    let mut scoped = fixture.engine("scoped").with_captured_input(CapturedInput {
        text: "Hi".into(),
        target: Some(ItemRef("someone-else".into())),
    });
    let append = json!({
        "request_id":"c", "item":raw["item"], "expected_revision":raw["revision"],
        "attested":{"source":{"kind":"input"}}
    });
    assert_eq!(
        scoped.call("item_append", &append).unwrap_err().code,
        ErrorCode::Forbidden
    );
}

#[test]
fn viewer_state_is_private_per_account_and_compare_and_set() {
    let fixture = Fixture::new();
    let mut owner = fixture.engine("owner");
    let created = owner
        .call(
            "item_create",
            &json!({"request_id":"v","kind":"reading-item","name":"V.md","metadata":{"title":"V"}}),
        )
        .unwrap();
    let item = created["item"].clone();
    let read = json!({"item":item});
    assert_eq!(
        owner.call("viewer_state_read", &read).unwrap(),
        json!({"version":"0","fields":{}})
    );
    let write = json!({
        "item":item, "expected":"0", "request_id":"pin",
        "patch":{"pinned":true,"last_read":"2026-11-20T10:00:00Z"}
    });
    let written = owner.call("viewer_state_write", &write).unwrap();
    assert_eq!(
        written,
        json!({"version":"1","fields":{"pinned":true,"last_read":"2026-11-20T10:00:00Z"}})
    );
    assert_eq!(owner.call("viewer_state_write", &write).unwrap(), written);
    let mut stale = write.clone();
    stale["request_id"] = json!("stale");
    assert_eq!(
        owner.call("viewer_state_write", &stale).unwrap_err().code,
        ErrorCode::Conflict
    );
    let mut other = fixture.engine_for(AccountId::new(), "other");
    assert_eq!(
        other.call("viewer_state_read", &read).unwrap(),
        json!({"version":"0","fields":{}})
    );
    for patch in [
        json!({"unread":3}),
        json!({"pinned":"yes"}),
        json!({"shared":true}),
        json!({"last_read":"2026-11-20T10:00:60Z"}),
    ] {
        let refused = json!({"item":item,"expected":"1","request_id":"bad","patch":patch});
        assert_eq!(
            owner.call("viewer_state_write", &refused).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
    let clear = json!({"item":item,"expected":"1","request_id":"clear","patch":{"pinned":null}});
    assert_eq!(
        owner.call("viewer_state_write", &clear).unwrap(),
        json!({"version":"2","fields":{"last_read":"2026-11-20T10:00:00Z"}})
    );
}
