use super::*;

#[tokio::test]
async fn thread_search_opens_exact_message_and_hides_unreadable_spaces() {
    let mut f = Fixture::new().await;
    let shared = f.spaces[2].clone();
    let private = &f.spaces[0];
    let send = async |space: &str| {
        let created = data(
            f.op(
                1,
                "threads.create",
                json!({"space_id":space,"title":"Project"}),
            )
            .await,
        );
        let id = created["thread"]["id"].clone();
        let sent = data(
            f.op(
                1,
                "threads.send",
                json!({"id":id,"message":"canvas nebula"}),
            )
            .await,
        );
        (id, sent["thread"]["messages"][0].clone())
    };
    let (hidden, _) = send(private).await;
    let (id, message) = send(&shared).await;
    let input = json!({"query":"\"canvas nebula\"","kinds":["threads"],"limit":1});
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    let hit = &page["hits"][0];
    assert_eq!(hit["address"], format!("/threads/{}", id.as_str().unwrap()));
    assert_eq!(hit["space_id"], shared);
    assert_eq!(hit["message_id"], message["id"]);
    assert_eq!(hit["message_at"], message["at"]);
    assert!(
        hit["excerpt"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["matched"] == true)
    );
    let opened = data(
        f.op(
            2,
            "threads.get",
            json!({"id":id,"message_id":message["id"]}),
        )
        .await,
    );
    assert_eq!(opened["thread"]["messages"], json!([message]));
    let hidden = f
        .op(
            2,
            "threads.get",
            json!({"id":hidden,"message_id":message["id"]}),
        )
        .await;
    assert_eq!(hidden.status, 404);

    let (second, second_message) = send(&shared).await;
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    assert_eq!(page["hits"][0]["message_id"], second_message["id"]);
    let mut next = input.clone();
    next["cursor"] = page["next_cursor"].clone();
    let page = ready_search(&f, 2, "search.query", next).await;
    assert_eq!(page["hits"][0]["message_id"], message["id"]);

    f.stop();
    let index_path = f
        .appdata()
        .join("spaces")
        .join(&shared)
        .join("search-messages-v1.all.db");
    let old = Connection::open(&index_path).unwrap();
    old.execute_batch(
        "UPDATE build_state SET generation='1';
         CREATE INDEX IF NOT EXISTS locations_thread ON locations(thread,position);
         DELETE FROM postings;
         BEGIN IMMEDIATE;",
    )
    .unwrap();
    f.start().await;
    let pending = data(f.op(2, "search.query", input.clone()).await);
    assert_eq!(pending, json!({"hits":[],"partial":true}));
    old.execute_batch("ROLLBACK").unwrap();
    drop(old);
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    assert_eq!(page["hits"][0]["message_id"], second_message["id"]);
    let read = Connection::open(&index_path).unwrap();
    assert!(
        !read
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='locations_thread')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
    );
    drop(read);

    f.op(
        1,
        "threads.rename",
        json!({"id":second,"title":"A longer saved thread title"}),
    )
    .await
    .ok();
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    assert_eq!(page["hits"][0]["title"], "A longer saved thread title");
    let opened = data(
        f.op(
            2,
            "threads.get",
            json!({"id":second,"message_id":second_message["id"]}),
        )
        .await,
    );
    assert_eq!(opened["thread"]["messages"], json!([second_message]));
    f.op(1, "threads.stop", json!({"id":second})).await.ok();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let deleted = f.op(1, "threads.delete", json!({"id":second})).await;
            if deleted.status == 200 {
                break;
            }
            assert_eq!(deleted.status, 409, "{}", deleted.body);
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("test turn stopped");
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    assert_eq!(page["hits"][0]["message_id"], message["id"]);
    assert!(page.get("next_cursor").is_none());
    let deleted = f
        .op(
            2,
            "threads.get",
            json!({"id":second,"message_id":second_message["id"]}),
        )
        .await;
    assert_eq!(deleted.status, 404);

    f.op(
        1,
        "plugins.disable",
        json!({"space_id":shared,"plugin_id":"calternal-orchestrator"}),
    )
    .await
    .ok();
    assert_eq!(
        data(f.op(2, "search.query", input).await),
        json!({"hits":[],"partial":false})
    );
}
