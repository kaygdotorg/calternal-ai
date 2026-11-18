use super::*;

#[tokio::test]
async fn thread_search_opens_exact_message_and_hides_unreadable_spaces() {
    let f = Fixture::new().await;
    let shared = &f.spaces[2];
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
    let (id, message) = send(shared).await;
    let input = json!({"query":"\"canvas nebula\"","kinds":["threads"],"limit":1});
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    let hit = &page["hits"][0];
    assert_eq!(hit["address"], format!("/threads/{}", id.as_str().unwrap()));
    assert_eq!(hit["space_id"], *shared);
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

    let (second, second_message) = send(shared).await;
    let page = ready_search(&f, 2, "search.query", input.clone()).await;
    assert_eq!(page["hits"][0]["message_id"], second_message["id"]);
    let mut next = input.clone();
    next["cursor"] = page["next_cursor"].clone();
    let page = ready_search(&f, 2, "search.query", next).await;
    assert_eq!(page["hits"][0]["message_id"], message["id"]);

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
