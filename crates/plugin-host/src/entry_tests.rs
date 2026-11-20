use crate::space_channel::{PackagePin, SpaceChannel, SpaceMessage};
use crate::{Error, entry};
use calternal_types::SpaceId;
use serde_json::json;
use uuid::Uuid;

#[test]
fn invoke_counts_encoded_bytes_and_line_feed() {
    let empty = json!({"call": "one", "input": ""});
    let overhead = entry::encode_invoke(&empty).unwrap().len();
    let exact =
        json!({"call": "one", "input": "x".repeat(entry::MAX_ENTRY_FRAME_BYTES - overhead)});
    let encoded = entry::encode_invoke(&exact).unwrap();
    assert_eq!(encoded.len(), entry::MAX_ENTRY_FRAME_BYTES);
    assert_eq!(encoded.last(), Some(&b'\n'));
    let oversize = json!({"call": "one", "input": "\0".repeat(entry::MAX_ENTRY_FRAME_BYTES / 6)});
    assert_eq!(entry::encode_invoke(&oversize), Err(Error::TooLarge));
    assert_eq!(
        entry::encode_invoke(&json!({"call": "one", "input": null, "ticket": "secret"})),
        Err(Error::Invalid)
    );
}

#[test]
fn decode_rejects_invalid_shape_and_raw_encoding() {
    let valid = b"{\"seq\":1,\"kind\":\"output\",\"data\":{\"text\":\"hello\"}}\n";
    assert_eq!(entry::decode_frame(valid).unwrap()["data"]["text"], "hello");
    for invalid in [
        &valid[..valid.len() - 1],
        b"{\"seq\":0,\"kind\":\"terminal\",\"state\":\"ok\"}\n",
        b"{\"seq\":1,\"kind\":\"terminal\",\"state\":\"ok\",\"data\":null}\n",
        b"{\"seq\":1,\"kind\":\"output\",\"data\":{\"a\":1,\"a\":2}}\n",
        b"{\"seq\":1,\"kind\":\"output\",\"data\":\"\xff\"}\n",
    ] {
        assert_eq!(entry::decode_frame(invalid), Err(Error::Invalid));
    }
    let oversized = vec![b'x'; entry::MAX_ENTRY_FRAME_BYTES + 1];
    assert_eq!(entry::decode_frame(&oversized), Err(Error::TooLarge));
}

#[tokio::test]
async fn only_the_host_can_dispatch_an_entry_on_its_ticket() {
    let (host_io, runtime_io) = tokio::io::duplex(4096);
    let space = SpaceId::new();
    let assignment = Uuid::new_v4();
    let pin = PackagePin {
        package: "fixture".into(),
        digest: "a".repeat(64),
    };
    let (host, runtime) = tokio::join!(
        SpaceChannel::host(host_io, space, assignment, vec![pin.clone()]),
        SpaceChannel::runtime(runtime_io, space, assignment),
    );
    let host = host.unwrap();
    let mut runtime = runtime.unwrap();
    let ticket = Uuid::new_v4();
    let message = || SpaceMessage::StartEntry {
        program: entry::EntryProgram {
            image_source: "registry.example/fixture".into(),
            image_digest: format!("sha256:{}", "b".repeat(64)),
            entry_sha256: "c".repeat(64),
            entry_base64: "YQ==".into(),
            invocation: json!({"call": "call", "input": {}}),
            methods: vec![],
            login: None,
            network: entry::EntryNetwork::None,
            run_limit_secs: 60,
        },
    };
    assert!(
        runtime
            .sender()
            .send_wait(&pin, ticket, message())
            .await
            .is_err()
    );
    host.sender()
        .send_checked_wait(&pin, ticket, message(), || true)
        .await
        .unwrap();
    let received = runtime.receive().await.unwrap();
    assert_eq!(received.frame().ticket, Some(ticket));
    assert!(
        matches!(&received.frame().message, SpaceMessage::StartEntry { program } if program.invocation["call"] == "call")
    );
}

#[test]
fn broker_calls_carry_only_the_checked_call() {
    let call = entry::decode_broker_call(
        b"{\"type\":\"call\",\"callback_id\":\"c1\",\"method\":\"item_head\",\"input\":{\"a\":1}}\n",
    )
    .unwrap();
    assert_eq!(call.callback_id, "c1");
    assert_eq!(call.method.as_str(), "item_head");
    assert_eq!(call.input, json!({"a": 1}));
    for invalid in [
        &b"{\"type\":\"call\",\"callback_id\":\"c1\",\"method\":\"item_head\",\"input\":{},\"ticket\":\"t\"}\n"[..],
        b"{\"type\":\"call\",\"callback_id\":\"c1\",\"method\":\"not_a_method\",\"input\":{}}\n",
        b"{\"type\":\"call\",\"callback_id\":\"\",\"method\":\"item_head\",\"input\":{}}\n",
        b"{\"type\":\"call\",\"callback_id\":\"c1\",\"callback_id\":\"c2\",\"method\":\"item_head\",\"input\":{}}\n",
        b"{\"type\":\"call\",\"callback_id\":\"c1\",\"method\":\"item_head\",\"input\":{}}",
    ] {
        assert!(entry::decode_broker_call(invalid).is_err());
    }
    let mut oversize =
        br#"{"type":"call","callback_id":"c","method":"item_head","input":""#.to_vec();
    oversize.extend(std::iter::repeat_n(b'x', entry::MAX_BROKER_FRAME_BYTES));
    oversize.extend(b"\"}\n");
    assert_eq!(entry::decode_broker_call(&oversize), Err(Error::TooLarge));
}

#[test]
fn broker_replies_are_one_bounded_line() {
    let ok = entry::encode_broker_reply("c1", &Ok(json!({"x": 1})));
    assert_eq!(ok.iter().filter(|&&b| b == b'\n').count(), 1);
    assert!(ok.ends_with(b"\n"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&ok).unwrap(),
        json!({"type": "reply", "callback_id": "c1", "ok": true, "data": {"x": 1}})
    );
    let failed: serde_json::Value = serde_json::from_slice(&entry::encode_broker_reply(
        "c1",
        &Err(calternal_types::ErrorCode::Forbidden),
    ))
    .unwrap();
    assert_eq!(failed["error"], json!({"status": 403, "code": "forbidden"}));
    let huge =
        entry::encode_broker_reply("c1", &Ok(json!("x".repeat(entry::MAX_BROKER_FRAME_BYTES))));
    let huge: serde_json::Value = serde_json::from_slice(&huge).unwrap();
    assert_eq!(huge["error"]["code"], "too_large");
}

#[test]
fn program_check_refuses_overlapping_logins_and_bad_hosts() {
    let program = |directories: &[&str], network| entry::EntryProgram {
        image_source: "registry.example/fixture".into(),
        image_digest: format!("sha256:{}", "b".repeat(64)),
        entry_sha256: "c".repeat(64),
        entry_base64: "YQ==".into(),
        invocation: json!({"call": "call", "input": {}}),
        methods: vec![],
        login: Some(entry::EntryLogin {
            account: calternal_types::AccountId::new(),
            package: "fixture".into(),
            contribution: "claude-code".into(),
            directories: directories.iter().map(|d| d.to_string()).collect(),
        }),
        network,
        run_limit_secs: 60,
    };
    let sites = |host: &str| entry::EntryNetwork::Sites {
        hosts: vec![host.into()],
    };
    assert!(
        program(&[".claude", ".config/tool"], sites("api.anthropic.com"))
            .check()
            .is_ok()
    );
    for (dirs, network) in [
        (&[".claude", ".claude/sub"][..], entry::EntryNetwork::None),
        (&[".claude", ".claude"][..], entry::EntryNetwork::None),
        (&["../escape"][..], entry::EntryNetwork::None),
        (&["/absolute"][..], entry::EntryNetwork::None),
        (&["a//b"][..], entry::EntryNetwork::None),
        (&[".claude"][..], sites("127.0.0.1")),
        (&[".claude"][..], sites("API.example.com")),
        (&[".claude"][..], sites("localhost")),
    ] {
        assert!(program(dirs, network).check().is_err(), "{dirs:?}");
    }
    let mut long = program(&[".claude"], entry::EntryNetwork::None);
    long.run_limit_secs = entry::MAX_RUN_LIMIT_SECS + 1;
    assert!(long.check().is_err());
}
