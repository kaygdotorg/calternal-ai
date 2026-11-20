use crate::plugins::item_engine::{ItemAccess, ItemCheck, ItemEngine, ItemStages};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::Content;
use calternal_plugin_host::ManifestV2;
use calternal_roots::Roots;
use calternal_types::{AccountId, ItemRef, RequestId, RevisionRef, SpaceId};
use serde_json::{Value, json};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[test]
fn plugin_item_exchanges_replay_and_preserve_the_prior_range() {
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
    let mut operation = manifest["operations"][0].clone();
    operation["methods"] = json!([
        "item_create",
        "item_head",
        "item_range",
        "item_save",
        "item_append",
        "item_save_begin",
        "item_save_chunk",
        "item_save_seal"
    ]);
    operation["output_schema"] = json!({
        "type":"object", "additionalProperties":false, "required":["created"],
        "properties":{"created":{"$ref":"host-catalogue-v2.json#/$defs/ItemRef"}}
    });
    let mut declaration = manifest;
    declaration["kinds"][0] = kind;
    declaration["operations"] = json!([operation]);
    declaration
        .as_object_mut()
        .unwrap()
        .remove("schedule_eligibility");
    let package = ManifestV2::parse(&serde_json::to_vec(&declaration).unwrap()).unwrap();
    let account = AccountId::new();
    let invocation = RequestId("invocation".to_owned());
    let deny_delivery = Cell::new(false);
    let authority = |access: ItemAccess| {
        if deny_delivery.get() && access.check == ItemCheck::Deliver {
            Err(calternal_types::Error::forbidden("delivery revoked"))
        } else {
            Ok(())
        }
    };
    let staging = ItemStages::new(space);
    let mut engine = ItemEngine::new(
        &content,
        space,
        account,
        &package,
        "reading.add",
        "reading-item",
        &invocation,
        &authority,
    )
    .unwrap()
    .with_stages(&staging)
    .unwrap();
    engine.provision_kind().unwrap();
    let create = json!({
        "request_id":"new", "kind":"reading-item", "folders":["Example"], "name":"Conversation.md",
        "metadata":{"title":"Conversation"}, "body":"SGVsbG8=", "attempt_discovery":"creator_only"
    });
    let created = engine.call("item_create", &create).unwrap();
    assert_eq!(engine.call("item_create", &create).unwrap(), created);
    let item = created["item"].clone();
    assert_ne!(item, created["locator"]["id"]);
    let local = Uuid::parse_str(created["locator"]["id"].as_str().unwrap()).unwrap();
    assert!(
        content
            .creator_discoverable(space, local, "reading-item", account)
            .unwrap()
    );
    assert!(
        !content
            .creator_discoverable(space, local, "reading-item", AccountId::new())
            .unwrap()
    );
    assert_eq!(
        engine.created(&json!({"created":item})).unwrap(),
        Some(ItemRef(item.as_str().unwrap().to_owned()))
    );
    deny_delivery.set(true);
    assert!(engine.created(&json!({"created":item})).is_err());
    deny_delivery.set(false);
    let unaffiliated = RequestId("different invocation".to_owned());
    let other = ItemEngine::new(
        &content,
        space,
        account,
        &package,
        "reading.add",
        "reading-item",
        &unaffiliated,
        &authority,
    )
    .unwrap();
    assert!(other.created(&json!({"created":item})).is_err());
    let head = engine.call("item_head", &json!({"item":item})).unwrap();
    assert_eq!(head["bytes"], 5);
    let before = head["revision"].clone();
    let append = json!({
        "request_id":"reply", "item":item, "expected_revision":before, "bytes":"IQ=="
    });
    let added = engine.call("item_append", &append).unwrap();
    assert_eq!(added["end_offset"], 6);
    assert_eq!(engine.call("item_append", &append).unwrap(), added);
    assert!(
        content
            .creator_discoverable(space, local, "reading-item", account)
            .unwrap()
    );
    assert_eq!(
        engine
            .call(
                "item_range",
                &json!({"item":item,"revision":before,"offset":0,"length":64})
            )
            .unwrap(),
        json!({"bytes":"SGVsbG8="})
    );
    assert!(
        engine
            .call("item_head", &json!({"item":created["locator"]["id"]}))
            .is_err()
    );
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
    let mut discovery = ItemEngine::new(
        &content,
        space,
        account,
        &package,
        "reading.add",
        "reading-item",
        &invocation,
        &authority,
    )
    .unwrap()
    .for_attempt_discovery();
    let reference = ItemRef(item.as_str().unwrap().to_owned());
    let prior = RevisionRef(before.as_str().unwrap().to_owned());
    discovery.pin_revision(&reference, prior.clone()).unwrap();
    discovery
        .authorize_range_delivery(&reference, &prior)
        .unwrap();
    let current = saved["revision"].as_str().unwrap();
    content
        .write(
            space,
            Some(local),
            "reading-item",
            Some(current),
            json!({"title":"Imported"}),
            b"unproven",
            calternal_content::Request {
                account,
                via: None,
                id: "external",
                operation: "test.import",
                fingerprint: "external",
            },
            |_| json!({}),
        )
        .unwrap();
    assert!(
        !content
            .creator_discoverable(space, local, "reading-item", account)
            .unwrap()
    );
    assert!(
        discovery
            .authorize_range_delivery(&reference, &prior)
            .is_err()
    );
    assert!(discovery.call("item_head", &json!({"item": item})).is_err());
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
