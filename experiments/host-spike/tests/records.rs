//! A chat-like record file end to end: appends by two people, a delete for
//! everyone, and forged or changed bytes written straight to disk.
mod common;

use calternal_types::{AccountId, SpaceId};
use common::{Server, declaration};
use serde_json::{Value, json};
use std::io::Write;

const CHAT: &str = "spike-chat";

fn records(srv: &Server, who: AccountId, file: &str) -> Value {
    srv.ok(who, CHAT, "record.list", json!({ "file": file }))
}

#[test]
fn record_file_append_delete_and_forgery() {
    let dir = tempfile::tempdir().unwrap();
    let (ana, lee, sam) = (AccountId::new(), AccountId::new(), AccountId::new());
    let srv = Server::start(
        dir.path(),
        SpaceId::new(),
        &[
            (ana, "Ana", "owner"),
            (lee, "Lee", "editor"),
            (sam, "Sam", "editor"),
        ],
    );
    srv.ok(
        ana,
        "host",
        "plugins.install",
        json!({ "declaration": declaration("spike-chat.json") }),
    );
    let chat = srv.ok(
        lee,
        CHAT,
        "files.create",
        json!({ "kind": "chats", "fields": { "title": "Lisbon crew" } }),
    );
    let (file, path) = (
        chat["id"].as_str().unwrap().to_owned(),
        chat["path"].as_str().unwrap().to_owned(),
    );
    assert_eq!(path, "Chats/Lisbon crew.md");
    let disk = srv.files().join(&path);

    let a1 = srv.ok(
        lee,
        CHAT,
        "record.append",
        json!({ "file": file, "body": "Can we move the trip to Friday?" }),
    )["anchor"]
        .as_str()
        .unwrap()
        .to_owned();
    let a2 = srv.ok(
        sam,
        CHAT,
        "record.append",
        json!({ "file": file, "body": "Friday works." }),
    )["anchor"]
        .as_str()
        .unwrap()
        .to_owned();
    let a3 = srv.ok(
        lee,
        CHAT,
        "record.append",
        json!({ "file": file, "body": "Great, booking the flat now." }),
    )["anchor"]
        .as_str()
        .unwrap()
        .to_owned();
    let text = std::fs::read_to_string(&disk).unwrap();
    assert!(text.starts_with(&format!("---\nid: {file}\nkind: chats\nformat: \"calternal-chat/v2\"\ntitle: \"Lisbon crew\"\n---\n**Lee** · ")), "{text}");
    assert!(text.contains(&format!(" ^{a2}\n\nFriday works.\n\n")));
    let len_before = text.len();

    // Only the author deletes; the delete is in place, padded to the old length.
    assert_eq!(
        srv.call(
            sam,
            CHAT,
            None,
            "record.delete",
            json!({ "file": file, "anchor": a3 })
        )
        .0,
        403
    );
    let del = srv.ok(
        lee,
        CHAT,
        "record.delete",
        json!({ "file": file, "anchor": a3 }),
    );
    assert_eq!(del["in_place"], true);
    let text = std::fs::read_to_string(&disk).unwrap();
    assert_eq!(text.len(), len_before);
    assert!(text.contains(&format!(" ^{a3}\n\n*deleted*\n")) && !text.contains("booking"));
    // Host state never held the words (hashes only, D5); nothing in its folder has them.
    let state = srv.root.join("state").join(srv.space.to_string());
    for entry in std::fs::read_dir(&state).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!bytes.windows(7).any(|w| w == b"booking"));
    }

    // The ledger holds the stamps: authors and plugin per record, the delete stamped too.
    let db = srv.ledger();
    let authors: Vec<(String, String, String)> = db
        .prepare("SELECT anchor, person, op FROM stamps WHERE anchor IS NOT NULL ORDER BY seq")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let row = |a: &str, p: AccountId, op: &str| (a.to_owned(), p.to_string(), op.to_owned());
    assert_eq!(
        authors,
        [
            row(&a1, lee, "append"),
            row(&a2, sam, "append"),
            row(&a3, lee, "append"),
            row(&a3, lee, "delete")
        ]
    );
    let scrubbed: i64 = db
        .query_row(
            "SELECT count(*) FROM stamps WHERE anchor = ?1 AND hash IS NULL",
            [&a3],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        scrubbed, 1,
        "the deleted record's earlier hash is scrubbed; the delete stamp hashes the padded bytes"
    );

    // A forged record appended straight to disk, claiming to be Lee: accepted as a
    // pure append, but stamped `outside`, and listed as `outside`, never as Lee.
    let forged = "**Lee** · 2026-11-20 17:02 ^zzzzzzzzzzzzzzzz\n\nI quit.\n\n";
    std::fs::OpenOptions::new()
        .append(true)
        .open(&disk)
        .unwrap()
        .write_all(forged.as_bytes())
        .unwrap();
    let list = records(&srv, ana, &file);
    assert_eq!(
        list["checked"]["outside_appends"],
        json!(["zzzzzzzzzzzzzzzz"])
    );
    let last = &list["records"][3];
    assert_eq!(
        (last["author"].as_str(), last["body"].as_str()),
        (Some("outside"), Some("I quit."))
    );

    // A changed name in an existing header: not a pure append, so a conflict copy is
    // written, and the header is put back from its stamp: the file is as it was.
    let clean = std::fs::read_to_string(&disk).unwrap();
    // Same length, same inode: only mtime shows the change, and the kernel's file
    // timestamps are coarse (one tick, a few ms). An edit inside the same tick as the
    // host's own write is invisible to the cheap check (see the report: "racy clean").
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&disk, clean.replacen("**Sam** · ", "**Lee** · ", 1)).unwrap();
    let list = records(&srv, ana, &file);
    assert_eq!(list["checked"]["restored"], true, "{list}");
    assert_eq!(std::fs::read_to_string(&disk).unwrap(), clean);
    let copy = std::fs::read_to_string(
        srv.files()
            .join(list["checked"]["conflict_copy"].as_str().unwrap()),
    )
    .unwrap();
    assert_eq!(
        (
            copy.matches("**Lee** · ").count(),
            clean.matches("**Lee** · ").count()
        ),
        (4, 3)
    );
    assert_eq!(list["records"][1]["author"], "Sam");

    // A changed body cannot be put back (host state has no copy of the text): it is
    // detected, a conflict copy is written, and the record is marked tampered.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&disk, clean.replace("Friday works.", "Monday works.")).unwrap();
    let list = records(&srv, ana, &file);
    assert_eq!(list["checked"]["restored"], false);
    assert_eq!(list["checked"]["tampered"], json!([a2]));
    assert_eq!(
        (
            list["records"][1]["state"].as_str(),
            list["records"][1]["author"].as_str()
        ),
        (Some("tampered"), Some("Sam"))
    );
    // The file still takes appends afterwards, and the next check finds nothing new.
    srv.ok(
        ana,
        CHAT,
        "record.append",
        json!({ "file": file, "body": "Back to Friday, please." }),
    );
    let list = records(&srv, ana, &file);
    assert!(list["checked"].is_null(), "{list}");
    assert_eq!(list["records"].as_array().unwrap().len(), 5);
}
