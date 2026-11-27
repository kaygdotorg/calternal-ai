//! Exercise crash recovery at the boundary between the file and ledger flush.
use super::*;
use crate::Call;
use calternal_types::{AccountId, SpaceId};
use std::io::Write;
use std::path::{Path, PathBuf};

const REQUEST: &[u8] = b"fixture canonical write request";
const GESTURE: &str = "one-gesture";

struct Fixture {
    root: tempfile::TempDir,
    id: SpaceId,
    actor: Actor,
    space: Space,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let id = SpaceId::new();
        let mut space = Space::open(root.path(), id, 7 * 86_400_000).unwrap();
        let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins-v2/calternal-chat");
        space.install(&plugin, true).unwrap();
        let actor = Actor::plugin(AccountId::new(), "calternal-chat");
        Self {
            root,
            id,
            actor,
            space,
        }
    }

    fn add(&mut self, title: &str) -> Head {
        let value = self.space.call(&self.actor, Call {
            call: "s.write".to_owned(),
            input: json!({"changes":[{"op":"add","kind":"calternal-chat.chats","fields":{"title":title}}]}),
            idempotency_key: None,
        }).unwrap();
        let item = serde_json::from_value(value["changes"][0]["id"].clone()).unwrap();
        self.space.row(item).unwrap().head.clone()
    }

    fn scope(&self) -> String {
        format!("write:{}", serde_json::to_string(&self.actor).unwrap())
    }

    fn tail(&self, head: &Head) -> (Value, Vec<u8>) {
        let receipt = self
            .space
            .receipt(&self.scope(), Some(GESTURE), REQUEST)
            .unwrap()
            .unwrap();
        let input = Input {
            fields: Fields::new(),
            body: Some("Message left by the interrupted request".to_owned()),
        };
        let (next, record, bytes, _) = make_append(
            &self.space,
            head,
            &self.actor,
            input,
            calternal_types::now_ms(),
            Some(&receipt),
        )
        .unwrap();
        (append_result(&next, &record), bytes)
    }

    fn path(&self, head: &Head) -> PathBuf {
        self.root
            .path()
            .join("spaces")
            .join(self.id.to_string())
            .join(&head.path)
    }

    fn append_uncommitted(&self, target: &Head, tail: &[u8]) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(self.path(target))
            .unwrap();
        file.write_all(tail).unwrap();
        file.sync_data().unwrap();
    }

    fn reopen(self) -> Self {
        let Self {
            root,
            id,
            actor,
            space,
        } = self;
        drop(space);
        let space = Space::open(root.path(), id, 7 * 86_400_000).unwrap();
        Self {
            root,
            id,
            actor,
            space,
        }
    }

    fn verify_outside(&self, head: &Head) {
        assert_eq!(self.space.row(head.id).unwrap().head.version, head.version);
        assert!(self.space.records(head.id).unwrap().is_empty());
        assert!(
            self.space
                .receipt(&self.scope(), Some(GESTURE), REQUEST)
                .unwrap()
                .unwrap()
                .replay
                .is_none()
        );
        let bytes = std::fs::read(self.path(head)).unwrap();
        assert!(bytes.len() as u64 > head.length);
        let kind = self.space.kind(&head.kind).unwrap();
        assert_eq!(
            self.space.record_views(head, &bytes, &[], kind).unwrap()[0]["author"],
            "outside"
        );
    }
}

#[test]
fn recovers_a_complete_append_and_its_retry_receipt() {
    let mut f = Fixture::new();
    let head = f.add("First");
    let (expected, tail) = f.tail(&head);
    f.append_uncommitted(&head, &tail);
    f = f.reopen();
    let rows = f.space.records(head.id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].author, f.actor.account.to_string());
    assert_eq!(f.space.row(head.id).unwrap().head.version, head.version + 1);
    let receipt = f
        .space
        .receipt(&f.scope(), Some(GESTURE), REQUEST)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.replay, Some(expected));
    assert!(
        f.space
            .receipt(&f.scope(), Some(GESTURE), b"different request")
            .is_err()
    );
    f = f.reopen();
    assert_eq!(f.space.records(head.id).unwrap().len(), 1);
}

#[test]
fn a_torn_append_stays_outside() {
    let mut f = Fixture::new();
    let head = f.add("First");
    let (_, tail) = f.tail(&head);
    f.append_uncommitted(&head, &tail[..tail.len() - 4]);
    f = f.reopen();
    f.verify_outside(&head);
}

#[test]
fn a_valid_append_copied_to_another_item_stays_outside() {
    let mut f = Fixture::new();
    let original = f.add("First");
    let another = f.add("Other");
    assert_eq!(original.length, another.length);
    assert_eq!(original.version, another.version);
    let (_, tail) = f.tail(&original);
    f.append_uncommitted(&another, &tail);
    f = f.reopen();
    f.verify_outside(&another);
}

#[test]
fn a_valid_append_at_another_offset_stays_outside() {
    let mut f = Fixture::new();
    let original = f.add("First");
    let mut other_placement = original.clone();
    other_placement.length += 1;
    let (_, tail) = f.tail(&other_placement);
    f.append_uncommitted(&original, &tail);
    f = f.reopen();
    f.verify_outside(&original);
}

#[test]
fn a_complete_append_with_changed_content_stays_outside() {
    let mut f = Fixture::new();
    let head = f.add("First");
    let (_, mut tail) = f.tail(&head);
    let body = tail.iter().position(|b| *b == b'\n').unwrap() + 2;
    tail[body] = b'X';
    f.append_uncommitted(&head, &tail);
    f = f.reopen();
    f.verify_outside(&head);
}
