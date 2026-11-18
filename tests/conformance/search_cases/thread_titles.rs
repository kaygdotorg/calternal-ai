use super::*;

#[tokio::test]
async fn thread_title_picker_uses_current_heads_and_hides_other_spaces() {
    let f = Fixture::new().await;
    let private = &f.spaces[0];
    let shared = &f.spaces[2];
    let create = async |space: &str, title: &str| {
        data(
            f.op(1, "threads.create", json!({"space_id":space,"title":title}))
                .await,
        )["thread"]["id"]
            .clone()
    };
    create(private, "Canvas A private").await;
    let one = create(shared, "Canvas B visible").await;
    let two = create(shared, "Canvas C visible").await;
    let first = data(
        f.op(2, "threads.lookup", json!({"prefix":"cANvas","limit":1}))
            .await,
    );
    assert_eq!(first["partial"], false);
    assert_eq!(
        first["hits"][0]["address"],
        format!("/threads/{}", one.as_str().unwrap())
    );
    let next = data(
        f.op(
            2,
            "threads.lookup",
            json!({"prefix":"cANvas","limit":1,"cursor":first["next_cursor"]}),
        )
        .await,
    );
    assert_eq!(
        next["hits"][0]["address"],
        format!("/threads/{}", two.as_str().unwrap())
    );

    f.op(1, "threads.rename", json!({"id":one,"title":"Boards"}))
        .await
        .ok();
    f.op(1, "threads.delete", json!({"id":two})).await.ok();
    assert_eq!(
        data(f.op(2, "threads.lookup", json!({"prefix":"Canvas"})).await)["hits"],
        json!([])
    );
    let renamed = data(f.op(2, "threads.lookup", json!({"prefix":"boards"})).await);
    assert_eq!(
        renamed["hits"][0]["address"],
        format!("/threads/{}", one.as_str().unwrap())
    );
    f.op(
        1,
        "plugins.disable",
        json!({"space_id":shared,"plugin_id":"calternal-orchestrator"}),
    )
    .await
    .ok();
    let disabled = data(f.op(2, "threads.lookup", json!({"prefix":"boards"})).await);
    assert_eq!(disabled, json!({"hits":[],"partial":false}));
}
