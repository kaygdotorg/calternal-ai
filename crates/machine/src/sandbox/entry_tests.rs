use super::{Exec, OutputEvent, entry::EntryOutput};
use crate::protocol::{ExitInfo, Stream};
use serde_json::json;
use tokio::sync::mpsc;

fn events(chunks: &[&[u8]]) -> EntryOutput {
    let (tx, events) = mpsc::channel(chunks.len() + 2);
    for chunk in chunks {
        tx.try_send(OutputEvent::Data(Stream::Stdout, chunk.to_vec()))
            .unwrap();
    }
    tx.try_send(OutputEvent::Data(Stream::Stderr, b"private".to_vec()))
        .unwrap();
    tx.try_send(OutputEvent::Exit(ExitInfo {
        code: Some(0),
        ..ExitInfo::default()
    }))
    .unwrap();
    EntryOutput::new(Exec {
        input: None,
        events,
    })
}

#[tokio::test]
async fn entry_stream_accepts_partial_and_coalesced_lines() {
    let mut output = events(&[
        b"{\"seq\":1,",
        b"\"kind\":\"output\",\"data\":1}\n{\"seq\":2,\"kind\":\"terminal\",\"state\":\"ok\"}\n",
    ]);
    assert_eq!(
        output.next().await.unwrap(),
        Some(json!({"seq": 1, "kind": "output", "data": 1}))
    );
    assert_eq!(
        output.next().await.unwrap(),
        Some(json!({"seq": 2, "kind": "terminal", "state": "ok"}))
    );
    assert_eq!(output.next().await.unwrap(), None);
}

#[tokio::test]
async fn entry_stream_refuses_invalid_end_or_sequence() {
    let terminal = b"{\"seq\":1,\"kind\":\"terminal\",\"state\":\"ok\"}\n";
    let oversized = vec![b' '; calternal_plugin_host::entry::MAX_ENTRY_FRAME_BYTES + 1];
    for chunks in [
        &[][..],
        &[&terminal[..terminal.len() - 1]][..],
        &[b"{\"seq\":2,\"kind\":\"terminal\",\"state\":\"ok\"}\n" as &[u8]][..],
        &[terminal.as_slice(), terminal.as_slice()][..],
        &[oversized.as_slice()][..],
    ] {
        assert!(events(chunks).next().await.is_err());
    }
}
