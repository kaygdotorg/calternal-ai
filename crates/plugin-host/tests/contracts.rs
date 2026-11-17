use calternal_core::*;
use calternal_plugin_host::validate_manifest;
use calternal_plugin_host::*;
use serde_json::{Value, json};

fn manifest() -> ManifestV1 {
    parse_manifest(include_bytes!(
        "../../../plugins/notes-insights/manifest.json"
    ))
    .unwrap()
}
fn authority() -> Authority {
    Authority {
        account_id: AccountId::parse("01932910-0000-7000-8000-000000000001").unwrap(),
        space_id: SpaceId::parse("01932910-0000-7000-8000-000000000002").unwrap(),
        grant: "host-reference".into(),
    }
}
#[derive(Default)]
struct FakeHost {
    revoked: bool,
    calls: usize,
}
impl HostBroker for FakeHost {
    fn authorize(&mut self, _: &Authority, _: ManifestCapability) -> bool {
        !self.revoked
    }
    fn call(&mut self, _: &Authority, method: HostCallKind, _: &Value) -> Result<Value, Error> {
        self.calls += 1;
        Ok(if method == HostCallKind::StartJob {
            json!({"job_id":"job-1"})
        } else if method == HostCallKind::RequestSecretHandle {
            json!({"handle":"opaque-handle-1"})
        } else {
            json!({"markdown":"hello"})
        })
    }
}
fn session() -> (WorkerSession, FakeHost) {
    (
        WorkerSession::new(manifest(), authority().space_id).unwrap(),
        FakeHost::default(),
    )
}
fn invoke(s: &mut WorkerSession, h: &mut FakeHost, id: &str) {
    s.invoke(
        id.into(),
        "notes-insights.summarize".into(),
        json!({"id":"01932910-0000-7000-8000-000000000003"}),
        authority(),
        h,
    )
    .unwrap();
}
fn frame(id: &str, message: Message) -> Frame {
    Frame {
        protocol_version: 2,
        invocation_id: id.into(),
        message,
    }
}
fn call(id: &str, method: HostCallKind) -> Frame {
    frame(
        "inv-1",
        Message::HostCall {
            call_id: id.into(),
            method,
            input: json!({}),
            authority: authority(),
        },
    )
}

#[test]
fn manifest_validation_vectors() {
    let m = manifest();
    let valid = serde_json::to_value(&m).unwrap();
    assert_eq!(
        parse_manifest(&serde_json::to_vec(&valid).unwrap()).unwrap(),
        m
    );
    let bad = [
        ("/api_version", json!(2)),
        ("/id", json!("../escape")),
        ("/id", json!("..")),
        ("/icon", json!("")),
        ("/name", json!("")),
        ("/version", json!("")),
        ("/operations/0/input_schema", json!(null)),
        ("/operations/0/output_schema", json!([])),
        (
            "/operations/0/required_capability",
            json!("arbitrary.admin"),
        ),
        ("/operations/0/mutates", json!(true)),
        ("/operations/0/idempotency", json!("never")),
        ("/operations/0/id", json!("")),
        // A target must name a required string property of the input schema.
        ("/operations/0/target", json!({"kind":"notes","field":"missing"})),
        ("/operations/0/target", json!({"kind":"notes","field":"space_id"})),
        ("/operations/0/target", json!({"kind":"Notes!","field":"id"})),
        ("/operations/0/target", json!({"kind":"notes","field":"id","extra":1})),
        ("/operations/0/target/field", json!("Id")),
        ("/kinds", json!([{"id":"notes","schema":null}])),
        // Non-text kinds must name a safe extension; text kinds are Markdown.
        ("/kinds", json!([{"id":"bin","schema":{},"storage":{"hidden":true},"claimed_text":false}])),
        ("/kinds", json!([{"id":"bin","schema":{},"storage":{"hidden":true},"claimed_text":false,"extension":"../x"}])),
        ("/kinds", json!([{"id":"bin","schema":{},"storage":{"hidden":true},"claimed_text":false,"extension":"PNG"}])),
        ("/kinds", json!([{"id":"t","schema":{},"storage":{"hidden":true},"claimed_text":true,"extension":"txt"}])),
        (
            "/collections",
            json!([{"id":"all","kind":"bad/kind","name":"All"}]),
        ),
        (
            "/views",
            json!([{"id":"v","region":"trusted_login","component":"x"}]),
        ),
        (
            "/views",
            json!([{"id":"v","region":"main","component":"../x"}]),
        ),
        ("/attention", json!([{"id":"a","operation":"missing"}])),
        (
            "/settings",
            json!([{"id":"s","value_type":"boolean","default":"true"}]),
        ),
        (
            "/schedules",
            json!([{"id":"s","operation":"notes-insights.summarize","expression":""}]),
        ),
        ("/webhooks", json!([{"id":"w","operation":"missing"}])),
        (
            "/hooks",
            json!([{"id":"h","mode":"rewrite","patterns":["notes.*"],"operation":"notes-insights.summarize"}]),
        ),
        (
            "/hooks",
            json!([{"id":"h","mode":"watch","patterns":["*"],"operation":"notes-insights.summarize"}]),
        ),
        ("/themes", json!([{"id":"t","stylesheet":"../theme.css"}])),
        ("/secrets", json!([{"name":"secret","value":"forbidden"}])),
        ("/host_calls", json!(["shell"])),
        ("/host_calls", json!(["read_item", "read_item"])),
    ];
    for (path, value) in bad {
        let mut v = valid.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert!(
            parse_manifest(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{path}"
        );
    }
    for key in [
        "kinds",
        "operations",
        "collections",
        "views",
        "attention",
        "settings",
        "schedules",
        "webhooks",
        "secrets",
        "hooks",
        "themes",
    ] {
        let mut v = valid.clone();
        let item = match key {
            "kinds" => {
                json!({"id":"notes","schema":true,"storage":{"hidden":true},"claimed_text":true})
            }
            "collections" => json!({"id":"all","kind":"notes","name":"All"}),
            "views" => json!({"id":"v","region":"main","component":"x"}),
            "attention" | "webhooks" => json!({"id":"a","operation":"notes-insights.summarize"}),
            "settings" => json!({"id":"s","value_type":"boolean","default":true}),
            "schedules" => {
                json!({"id":"s","operation":"notes-insights.summarize","expression":"* * * * *"})
            }
            "hooks" => {
                json!({"id":"h","mode":"block","patterns":["notes.*"],"operation":"notes-insights.summarize"})
            }
            "themes" => json!({"id":"t","stylesheet":"theme.css"}),
            "secrets" => json!("token"),
            _ => valid["operations"][0].clone(),
        };
        v[key] = json!([item.clone()]);
        assert!(
            parse_manifest(&serde_json::to_vec(&v).unwrap()).is_ok(),
            "valid {key}"
        );
        v[key] = json!([item.clone(), item]);
        assert!(
            parse_manifest(&serde_json::to_vec(&v).unwrap()).is_err(),
            "duplicate {key}"
        );
        if key != "secrets" {
            v[key][0]["unknown"] = json!(true);
            assert!(
                parse_manifest(&serde_json::to_vec(&v).unwrap()).is_err(),
                "unknown {key}"
            );
        }
    }
    let mut unknown = valid.clone();
    unknown["unknown"] = json!(true);
    assert!(parse_manifest(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert_eq!(
        parse_manifest(&vec![b' '; MAX_MANIFEST_BYTES + 1]),
        Err(Error::TooLarge)
    );
    assert!(parse_manifest(b"{} {}").is_err());
}
#[test]
fn collisions_are_atomic_and_policy_is_data_only() {
    for kind in ["plugin", "kind", "operation", "collection"] {
        let mut a = manifest();
        a.kinds.push(ManifestKind {
            storage: ManifestPlacement::Hidden { hidden: true },
            claimed_text: true,
            extension: None,
            id: "notes".into(),
            schema: json!(true),
        });
        a.collections.push(ManifestCollection {
            id: "all-notes".into(),
            kind: "notes".into(),
            name: "All".into(),
        });
        let mut b = a.clone();
        if kind != "plugin" {
            b.id = "other".into();
        }
        if kind != "kind" {
            b.kinds.clear();
        }
        if kind != "operation" {
            b.operations.clear();
        }
        if kind != "collection" {
            b.collections.clear();
        }
        let mut registry = Registry::default();
        registry.register(a.clone()).unwrap();
        assert_eq!(registry.register(b), Err(Error::Collision));
        assert_eq!(registry.requests(&a.id), Some(&a));
        assert_eq!(registry.policy_rows().len(), 1);
    }
}
#[test]
fn protocol_roundtrip_callback_refusal_and_live_revocation() {
    let (mut s, mut h) = session();
    invoke(&mut s, &mut h, "inv-1");
    let request = s.next_outgoing(&mut h).unwrap().unwrap();
    assert_eq!(
        decode_frame(&encode_frame(&request).unwrap()).unwrap(),
        request
    );
    let c = call("c1", HostCallKind::ReadItem);
    assert_eq!(decode_frame(&encode_frame(&c).unwrap()).unwrap(), c);
    s.receive(c.clone(), &mut h).unwrap();
    assert!(matches!(
        s.next_outgoing(&mut h).unwrap().unwrap().message,
        Message::HostReply { ok: true, .. }
    ));
    assert_eq!(s.receive(c, &mut h), Err(Error::Refused));
    assert_eq!(
        s.receive(call("c2", HostCallKind::WriteItem), &mut h),
        Err(Error::Refused)
    );
    assert_eq!(h.calls, 1);
    let mut forged = call("c3", HostCallKind::ReadItem);
    if let Message::HostCall { authority, .. } = &mut forged.message {
        authority.grant = "forged".into();
    }
    assert_eq!(s.receive(forged, &mut h), Err(Error::Refused));
    h.revoked = true;
    assert_eq!(
        s.receive(call("c4", HostCallKind::ReadItem), &mut h),
        Err(Error::Refused)
    );
    assert_eq!(
        s.receive(
            frame("inv-1", Message::Complete { data: json!({}) }),
            &mut h
        ),
        Err(Error::Refused)
    );
}
#[test]
fn back_pressure_does_not_execute_or_advance_and_cancel_rejects_late_work() {
    let (mut s, mut h) = session();
    for n in 0..MAX_QUEUE {
        invoke(&mut s, &mut h, &format!("inv-{n}"));
    }
    assert_eq!(
        s.receive(call("blocked", HostCallKind::ReadItem), &mut h),
        Err(Error::BackPressure)
    );
    assert_eq!(h.calls, 0);
    s.cancel("inv-1").unwrap();
    assert_eq!(
        s.receive(call("late", HostCallKind::ReadItem), &mut h),
        Err(Error::Invocation)
    );
    assert_eq!(
        s.receive(
            frame("inv-1", Message::Complete { data: json!(null) }),
            &mut h
        ),
        Err(Error::Invocation)
    );
}
#[test]
fn streams_are_ordered_bounded_and_require_admitted_jobs() {
    let mut m = manifest();
    m.host_calls.push(HostCallKind::StartJob);
    let mut s = WorkerSession::new(m, authority().space_id).unwrap();
    let mut h = FakeHost::default();
    invoke(&mut s, &mut h, "inv-1");
    s.next_outgoing(&mut h).unwrap();
    let chunk = |n| {
        frame(
            "inv-1",
            Message::Chunk {
                job_id: "job-1".into(),
                sequence: n,
                data: json!("x"),
            },
        )
    };
    assert_eq!(s.receive(chunk(0), &mut h), Err(Error::Invalid));
    s.receive(call("job", HostCallKind::StartJob), &mut h)
        .unwrap();
    s.next_outgoing(&mut h).unwrap();
    assert_eq!(s.receive(chunk(1), &mut h), Err(Error::Invalid));
    for n in 0..MAX_QUEUE {
        s.receive(chunk(n as u32), &mut h).unwrap();
    }
    assert_eq!(
        s.receive(chunk(MAX_QUEUE as u32), &mut h),
        Err(Error::BackPressure)
    );
    s.next_delivery(&mut h).unwrap();
    s.receive(chunk(MAX_QUEUE as u32), &mut h).unwrap();
    s.cancel("inv-1").unwrap();
    assert!(s.next_delivery(&mut h).unwrap().is_none());
}
#[test]
fn malformed_wire_and_oversize_are_rejected() {
    let f = frame("inv-1", Message::Failed {});
    let mut v = serde_json::to_value(f).unwrap();
    v["message"]["unknown"] = json!(true);
    assert!(decode_frame(&serde_json::to_vec(&v).unwrap()).is_err());
    assert_eq!(
        decode_frame(&vec![0; MAX_FRAME_BYTES + 1]),
        Err(Error::TooLarge)
    );
    assert!(
        decode_frame(
            b"{\"protocol_version\":1,\"invocation_id\":\"x\",\"message\":{\"type\":\"failed\"}}"
        )
        .is_err()
    );
}

#[test]
fn schemas_and_typed_identity_are_enforced_without_external_retrieval() {
    assert!(AccountId::parse("01932910000070008000000000000001").is_err());
    assert!(AccountId::parse("01932910-0000-4000-8000-000000000001").is_err());
    let mut m = manifest();
    m.operations[0].input_schema = json!({"type":42});
    assert_eq!(validate_manifest(&m), Err(Error::Invalid));
    m.operations[0].input_schema = json!({"$ref":"file:///etc/passwd"});
    assert_eq!(validate_manifest(&m), Err(Error::Invalid));
    m.operations[0].input_schema = json!({"$ref":"https://example.invalid/schema"});
    assert_eq!(validate_manifest(&m), Err(Error::Invalid));
    let (mut s, mut h) = session();
    assert_eq!(
        s.invoke(
            "bad".into(),
            "notes-insights.summarize".into(),
            json!({"extra":true}),
            authority(),
            &mut h
        ),
        Err(Error::Invalid)
    );
    invoke(&mut s, &mut h, "inv-1");
    s.next_outgoing(&mut h).unwrap();
    assert_eq!(
        s.receive(
            frame(
                "inv-1",
                Message::Complete {
                    data: json!({"wrong":true})
                }
            ),
            &mut h
        ),
        Err(Error::Invalid)
    );
    s.receive(
        frame(
            "inv-1",
            Message::Complete {
                data: json!({"stats":{"words":0,"characters":0,"lines":0},"view":{}}),
            },
        ),
        &mut h,
    )
    .unwrap();
    h.revoked = true;
    assert_eq!(s.next_delivery(&mut h), Err(Error::Refused));
    assert!(s.next_delivery(&mut h).unwrap().is_none());
}
#[test]
fn every_message_variant_roundtrips_and_queued_replies_recheck_authority() {
    let variants = [
        Message::Invoke {
            operation: "notes-insights.summarize".into(),
            input: json!({}),
            authority: authority(),
        },
        Message::HostReply {
            call_id: "c".into(),
            ok: false,
            data: Value::Null,
        },
        Message::Chunk {
            job_id: "j".into(),
            sequence: 0,
            data: json!({}),
        },
        Message::Complete { data: json!({}) },
        Message::Failed {},
        Message::Cancel {},
    ];
    for message in variants {
        let f = frame("i", message);
        assert_eq!(decode_frame(&encode_frame(&f).unwrap()).unwrap(), f);
    }
    let (mut s, mut h) = session();
    invoke(&mut s, &mut h, "inv-1");
    s.next_outgoing(&mut h).unwrap();
    s.receive(call("c", HostCallKind::ReadItem), &mut h)
        .unwrap();
    h.revoked = true;
    assert_eq!(s.next_outgoing(&mut h), Err(Error::Refused));
}
#[test]
fn shared_worker_wire_vectors_match_the_sdk() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../packages/plugin-sdk/test/fixtures/worker-v2-cases.json"
    ))
    .unwrap();
    for case in cases {
        assert_eq!(
            decode_frame(&serde_json::to_vec(&case["value"]).unwrap()).is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
}
#[test]
fn storage_placement_names_and_cross_plugin_folder_claims() {
    let mut m = manifest();
    m.kinds.push(ManifestKind {
        id: "notes".into(),
        schema: json!(true),
        storage: ManifestPlacement::Visible {
            folder: "Notes".into(),
        },
        claimed_text: true,
        extension: None,
    });
    assert!(validate_manifest(&m).is_ok());
    for folder in [
        "",
        ".calternal",
        ".private",
        "Trash",
        "TRASH",
        "../Notes",
        "a/b",
        "a\\b",
        "a:b",
        "a\0b",
        " Notes",
        "Notes ",
        "Cafe\u{301}",
    ] {
        let mut bad = m.clone();
        bad.kinds[0].storage = ManifestPlacement::Visible {
            folder: folder.into(),
        };
        assert!(validate_manifest(&bad).is_err(), "{folder:?}");
    }
    let mut hidden = m.clone();
    hidden.kinds[0].storage = ManifestPlacement::Hidden { hidden: false };
    assert!(validate_manifest(&hidden).is_err());
    hidden.kinds[0].storage = ManifestPlacement::Hidden { hidden: true };
    hidden.kinds[0].claimed_text = false;
    // A non-text kind must name its file extension.
    assert!(validate_manifest(&hidden).is_err());
    hidden.kinds[0].extension = Some("png".into());
    assert!(validate_manifest(&hidden).is_ok());
    let mut registry = Registry::default();
    registry.register(m.clone()).unwrap();
    let mut other = m.clone();
    other.id = "other".into();
    other.operations.clear();
    other.kinds[0].id = "tasks".into();
    other.kinds[0].storage = ManifestPlacement::Visible {
        folder: "notes".into(),
    };
    assert_eq!(registry.register(other.clone()), Err(Error::Collision));
    other.kinds[0].storage = ManifestPlacement::Hidden { hidden: true };
    assert!(registry.register(other).is_ok());
    let mut v = serde_json::to_value(m).unwrap();
    v["kinds"][0]["storage"] = json!({"folder":"Notes","hidden":true});
    assert!(parse_manifest(&serde_json::to_vec(&v).unwrap()).is_err());
}

#[test]
fn callbacks_enforce_declared_secret_names_and_operation_mutability() {
    let mut m = manifest();
    m.host_calls
        .extend([HostCallKind::RequestSecretHandle, HostCallKind::WriteItem]);
    m.secrets.push("provider-key".into());
    let mut s = WorkerSession::new(m, authority().space_id).unwrap();
    let mut h = FakeHost::default();
    invoke(&mut s, &mut h, "inv-1");
    s.next_outgoing(&mut h).unwrap();
    assert_eq!(
        s.receive(call("write", HostCallKind::WriteItem), &mut h),
        Err(Error::Refused)
    );
    let secret = |name| {
        frame(
            "inv-1",
            Message::HostCall {
                call_id: "secret".into(),
                method: HostCallKind::RequestSecretHandle,
                input: json!({"name":name}),
                authority: authority(),
            },
        )
    };
    assert_eq!(s.receive(secret("undeclared"), &mut h), Err(Error::Refused));
    assert_eq!(h.calls, 0);
    s.receive(secret("provider-key"), &mut h).unwrap();
    assert_eq!(h.calls, 1);
}

#[test]
fn failed_registration_does_not_reserve_other_names() {
    let mut r = Registry::default();
    r.register(manifest()).unwrap();
    let mut collision = manifest();
    collision.id = "other".into();
    collision.kinds.push(ManifestKind {
        id: "tasks".into(),
        schema: json!(true),
        storage: ManifestPlacement::Visible {
            folder: "Tasks".into(),
        },
        claimed_text: true,
        extension: None,
    });
    assert_eq!(r.register(collision.clone()), Err(Error::Collision));
    collision.operations.clear();
    assert!(r.register(collision).is_ok());
}
#[test]
fn required_capability_is_checked_before_admission() {
    struct Gate;
    impl HostBroker for Gate {
        fn authorize(&mut self, _: &Authority, required: ManifestCapability) -> bool {
            required == ManifestCapability::SpaceRead
        }
        fn call(&mut self, _: &Authority, _: HostCallKind, _: &Value) -> Result<Value, Error> {
            panic!("unauthorized effect")
        }
    }
    let mut m = manifest();
    m.operations[0].required_capability = ManifestCapability::ContentWrite;
    let mut s = WorkerSession::new(m, authority().space_id).unwrap();
    let mut h = Gate;
    assert_eq!(
        s.invoke(
            "denied".into(),
            "notes-insights.summarize".into(),
            json!({"id":"01932910-0000-7000-8000-000000000003"}),
            authority(),
            &mut h
        ),
        Err(Error::Refused)
    );
    assert!(s.next_outgoing(&mut h).unwrap().is_none());
}
#[test]
fn oversized_callback_output_is_redacted_without_repeating_the_effect() {
    struct LargeHost(usize);
    impl HostBroker for LargeHost {
        fn authorize(&mut self, _: &Authority, _: ManifestCapability) -> bool {
            true
        }
        fn call(&mut self, _: &Authority, _: HostCallKind, _: &Value) -> Result<Value, Error> {
            self.0 += 1;
            Ok(json!("x".repeat(MAX_FRAME_BYTES + 1)))
        }
    }
    let mut s = WorkerSession::new(manifest(), authority().space_id).unwrap();
    let mut h = LargeHost(0);
    s.invoke(
        "inv-1".into(),
        "notes-insights.summarize".into(),
        json!({"id":"01932910-0000-7000-8000-000000000003"}),
        authority(),
        &mut h,
    )
    .unwrap();
    s.next_outgoing(&mut h).unwrap();
    s.receive(call("large", HostCallKind::ReadItem), &mut h)
        .unwrap();
    assert!(matches!(
        s.next_outgoing(&mut h).unwrap().unwrap().message,
        Message::HostReply {
            ok: false,
            data: Value::Null,
            ..
        }
    ));
    assert_eq!(
        s.receive(call("large", HostCallKind::ReadItem), &mut h),
        Err(Error::Refused)
    );
    assert_eq!(h.0, 1);
    assert_eq!(
        encode_frame(&frame(
            "inv-1",
            Message::Complete {
                data: json!("x".repeat(MAX_FRAME_BYTES + 1))
            }
        )),
        Err(Error::TooLarge)
    );
}

#[test]
fn a_secret_value_response_is_never_forwarded_as_a_handle() {
    struct WrongSecret;
    impl HostBroker for WrongSecret {
        fn authorize(&mut self, _: &Authority, _: ManifestCapability) -> bool {
            true
        }
        fn call(&mut self, _: &Authority, _: HostCallKind, _: &Value) -> Result<Value, Error> {
            Ok(json!({"value":"never-forward-this"}))
        }
    }
    let mut m = manifest();
    m.host_calls.push(HostCallKind::RequestSecretHandle);
    m.secrets.push("key".into());
    let mut s = WorkerSession::new(m, authority().space_id).unwrap();
    let mut h = WrongSecret;
    s.invoke(
        "inv-1".into(),
        "notes-insights.summarize".into(),
        json!({"id":"01932910-0000-7000-8000-000000000003"}),
        authority(),
        &mut h,
    )
    .unwrap();
    s.next_outgoing(&mut h).unwrap();
    s.receive(
        frame(
            "inv-1",
            Message::HostCall {
                call_id: "secret".into(),
                method: HostCallKind::RequestSecretHandle,
                input: json!({"name":"key"}),
                authority: authority(),
            },
        ),
        &mut h,
    )
    .unwrap();
    assert!(matches!(
        s.next_outgoing(&mut h).unwrap().unwrap().message,
        Message::HostReply {
            ok: false,
            data: Value::Null,
            ..
        }
    ));
}

#[test]
fn a_full_unrelated_output_queue_cannot_keep_cancelled_authority_alive() {
    let (mut s, mut h) = session();
    invoke(&mut s, &mut h, "inv-1");
    invoke(&mut s, &mut h, "inv-2");
    s.next_outgoing(&mut h).unwrap();
    s.next_outgoing(&mut h).unwrap();
    for n in 0..MAX_QUEUE {
        let mut c = call(&format!("c-{n}"), HostCallKind::ReadItem);
        c.invocation_id = "inv-2".into();
        s.receive(c, &mut h).unwrap();
    }
    assert_eq!(s.cancel("inv-1"), Err(Error::BackPressure));
    assert_eq!(
        s.receive(call("late", HostCallKind::ReadItem), &mut h),
        Err(Error::Invocation)
    );
    assert_eq!(h.calls, MAX_QUEUE);
}
