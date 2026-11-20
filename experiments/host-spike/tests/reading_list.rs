//! Reading List end to end: installed as data, three links added through `/call`,
//! one marked read, "Unread" answered from the index and a scan.
mod common;

use calternal_types::{AccountId, SpaceId};
use common::{Server, declaration};
use serde_json::json;

const RL: &str = "calternal-reading-list";

#[test]
fn reading_list_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let (ana, sam, guest) = (AccountId::new(), AccountId::new(), AccountId::new());
    let mut srv = Server::start(
        dir.path(),
        SpaceId::new(),
        &[
            (ana, "Ana", "owner"),
            (sam, "Sam", "editor"),
            (guest, "Gus", "reader"),
        ],
    );

    // Install: only a manager, only from the host's own screen.
    let decl = declaration("calternal-reading-list.json");
    assert_eq!(
        srv.call(
            sam,
            "host",
            None,
            "plugins.install",
            json!({ "declaration": decl })
        )
        .0,
        403
    );
    assert_eq!(
        srv.call(
            ana,
            RL,
            None,
            "plugins.install",
            json!({ "declaration": decl })
        )
        .0,
        403
    );
    let installed = srv.ok(
        ana,
        "host",
        "plugins.install",
        json!({ "declaration": decl }),
    );
    assert_eq!(installed["sidebar"], json!(["Unread", "All"]));
    // The one validator refuses a broken declaration, and a second owner of `reading`.
    let mut broken = decl.clone();
    broken["collections"]["Unread"]["where"] = json!({ "url": "x" });
    assert_eq!(
        srv.call(
            ana,
            "host",
            None,
            "plugins.install",
            json!({ "declaration": broken })
        )
        .0,
        400
    );
    let mut thief = decl.clone();
    thief["id"] = json!("someone-else");
    assert_eq!(
        srv.call(
            ana,
            "host",
            None,
            "plugins.install",
            json!({ "declaration": thief })
        )
        .0,
        409
    );
    srv.ok(
        ana,
        "host",
        "plugins.install",
        json!({ "declaration": declaration("spike-chat.json") }),
    );

    // Add three links as Sam, through Reading List's kind grant (no code runs).
    let links = [
        ("https://doc.rust-lang.org/book/", "The Rust Book"),
        ("https://sqlite.org/wal.html", "Write-Ahead Logging"),
        ("https://www.markdownguide.org/", "Markdown Guide"),
    ];
    let mut ids = Vec::new();
    for (i, (url, title)) in links.iter().enumerate() {
        let attempt = format!("add-{i}");
        let input = json!({ "kind": "reading", "fields": { "url": url, "title": title } });
        let (status, first) = srv.call(sam, RL, Some(&attempt), "files.create", input.clone());
        assert_eq!(status, 200, "{first}");
        // A retry of the same gesture is answered from its receipt: no second file.
        let (_, again) = srv.call(sam, RL, Some(&attempt), "files.create", input);
        assert_eq!(first["result"], again["result"]);
        ids.push(first["result"]["id"].as_str().unwrap().to_owned());
    }
    // Refusals: a reader, a plugin without the grant, a field the kind lacks, a bad URL.
    let add = json!({ "kind": "reading", "fields": { "url": "https://example.org/" } });
    assert_eq!(
        srv.call(guest, RL, None, "files.create", add.clone()).0,
        403
    );
    assert_eq!(
        srv.call(sam, "spike-chat", None, "files.create", add.clone())
            .0,
        403
    );
    assert_eq!(
        srv.call(
            sam,
            RL,
            None,
            "files.create",
            json!({ "kind": "reading", "fields": { "url": "https://e.org/", "x": 1 } })
        )
        .0,
        400
    );
    assert_eq!(
        srv.call(
            sam,
            RL,
            None,
            "files.create",
            json!({ "kind": "reading", "fields": { "url": "ftp://e.org/" } })
        )
        .0,
        400
    );

    // Mark the second one read (the "Mark read" swipe is one files.set).
    srv.ok(
        sam,
        RL,
        "files.set",
        json!({ "file": ids[1], "fields": { "read": true } }),
    );
    assert_eq!(
        srv.call(
            sam,
            RL,
            None,
            "files.set",
            json!({ "file": ids[1], "fields": { "read": "yes" } })
        )
        .0,
        400
    );

    // "Unread", from the index and from a scan: the same two items, newest first.
    let unread = srv.ok(sam, RL, "query", json!({ "collection": "Unread" }));
    let scanned = srv.ok(
        sam,
        RL,
        "query",
        json!({ "collection": "Unread", "from": "scan" }),
    );
    assert_eq!(unread["count"], 2, "{unread}");
    assert_eq!(unread["items"], scanned["items"]);
    let titles: Vec<&str> = unread["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["fields"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Markdown Guide", "The Rust Book"]);
    assert_eq!(
        srv.ok(guest, RL, "query", json!({ "collection": "All" }))["count"],
        3
    );
    assert_eq!(
        srv.call(
            sam,
            RL,
            None,
            "query",
            json!({ "kind": "reading", "where": { "url": "x" } })
        )
        .0,
        400
    );

    // The files on disk are readable Markdown with the fields in frontmatter.
    let folder = srv.files().join("Reading");
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Markdown Guide.md",
            "The Rust Book.md",
            "Write-Ahead Logging.md"
        ]
    );
    let wal = std::fs::read_to_string(folder.join("Write-Ahead Logging.md")).unwrap();
    assert_eq!(
        wal,
        format!(
            "---\nid: {}\nkind: reading\nread: true\ntitle: \"Write-Ahead Logging\"\nurl: \"https://sqlite.org/wal.html\"\n---\n",
            ids[1]
        )
    );

    // Host state holds the stamps: three creates and one set, all Sam through Reading List.
    let db = srv.ledger();
    let stamps: Vec<(String, String, String)> = db
        .prepare("SELECT op, person, plugin FROM stamps ORDER BY seq")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let sam_rl = (sam.to_string(), RL.to_owned());
    assert_eq!(stamps.len(), 4);
    assert!(
        stamps
            .iter()
            .all(|(_, p, g)| (p.clone(), g.clone()) == sam_rl)
    );
    assert_eq!(
        stamps.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
        ["create", "create", "create", "set"]
    );
    // Every call passed through authorize and left an audit row, refusals included.
    let (allowed, denied): (i64, i64) = db
        .query_row(
            "SELECT sum(allowed), sum(1 - allowed) FROM audit",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(denied, 4, "denied calls audited");
    assert!(allowed >= 12);

    // An edit on disk that also changes `kind:` is stamped `outside`; host state's kind wins.
    let book = folder.join("The Rust Book.md");
    let edited = std::fs::read_to_string(&book)
        .unwrap()
        .replace("kind: reading", "kind: notes")
        .replace("The Rust Book", "The Book");
    std::fs::write(&book, edited).unwrap();
    srv.ok(
        sam,
        RL,
        "files.set",
        json!({ "file": ids[0], "fields": { "read": true } }),
    );
    let after = std::fs::read_to_string(&book).unwrap();
    assert!(
        after.contains("kind: reading")
            && after.contains("title: \"The Book\"")
            && after.contains("read: true"),
        "{after}"
    );
    let outside: i64 = db
        .query_row(
            "SELECT count(*) FROM stamps WHERE person = 'outside'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outside, 1);
    let unread = srv.ok(sam, RL, "query", json!({ "collection": "Unread" }));
    assert_eq!(unread["count"], 1);

    // The index is appdata: delete it, restart, and it is rebuilt to the same answer
    // from the files and host state. Installed plugins come back from host state.
    srv.restart();
    std::fs::remove_file(
        srv.root
            .join("appdata")
            .join(srv.space.to_string())
            .join("index.db"),
    )
    .unwrap();
    srv.restart();
    let rebuilt = srv.ok(sam, RL, "query", json!({ "collection": "Unread" }));
    assert_eq!(rebuilt["items"], unread["items"]);
    assert_eq!(
        srv.ok(
            sam,
            RL,
            "query",
            json!({ "collection": "All", "from": "scan" })
        )["count"],
        3
    );
}
