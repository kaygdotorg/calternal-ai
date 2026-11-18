use super::*;

fn materialized_path(f: &Fixture, space: &str, id: &str) -> std::path::PathBuf {
    let app = f.appdata().join("spaces").join(space);
    let db = Connection::open(app.join("content.db")).unwrap();
    let path: String = db
        .query_row("SELECT path FROM items WHERE id=?1", [id], |row| row.get(0))
        .unwrap();
    drop(db);
    f.dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join(path)
}

fn invalidate(f: &Fixture, space: &str) {
    let app = f.appdata().join("spaces").join(space);
    Connection::open(app.join("search-v2.db"))
        .unwrap()
        .execute(
            "UPDATE search_configuration SET generation='older-release'",
            [],
        )
        .unwrap();
}

#[tokio::test]
async fn rebuild_uses_the_staged_body_when_materialized_bytes_disagree() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    let saved = note(&f, 1, &space, "Integrity probe", "guardedstaged").await;
    let query = json!({"space_id":space,"query":"guardedstaged"});
    let before = ready_search(&f, 2, "search.query", query.clone()).await;
    assert_eq!(
        before["hits"][0]["address"],
        format!("/notes/{}", saved["id"].as_str().unwrap())
    );

    let path = materialized_path(&f, &space, saved["id"].as_str().unwrap());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(&path).is_ok_and(|text| text.contains("guardedstaged")) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    f.stop();

    let prior = std::fs::read_to_string(&path).unwrap();
    let altered = prior.replace("guardedstaged", "guardedforged");
    assert_ne!(altered, prior);
    assert_eq!(altered.len(), prior.len());
    std::fs::write(&path, &altered).unwrap();
    invalidate(&f, &space);

    f.start().await;
    assert_eq!(ready_search(&f, 2, "search.query", query).await, before);
    let forged = json!({"space_id":space,"query":"guardedforged"});
    assert_eq!(
        ready_search(&f, 2, "search.query", forged).await["hits"],
        json!([])
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), altered);
}

#[tokio::test]
async fn rebuild_can_fall_back_to_an_empty_staged_body() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    let saved = note(&f, 1, &space, "Empty fallback sentinel", "").await;
    let id = saved["id"].as_str().unwrap();
    let path = materialized_path(&f, &space, id);
    let query = json!({"space_id":space,"query":"empty fallback sentinel"});
    let before = ready_search(&f, 2, "search.query", query.clone()).await;
    assert_eq!(before["hits"][0]["address"], format!("/notes/{id}"));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    f.stop();

    std::fs::remove_file(path).unwrap();
    invalidate(&f, &space);
    f.start().await;
    assert_eq!(ready_search(&f, 2, "search.query", query).await, before);
}
