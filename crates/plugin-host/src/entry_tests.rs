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
