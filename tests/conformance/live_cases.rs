//! Live journal isolation over TCP with real signed-in account sessions.
use super::*;
pub(super) struct Live {
    pub(super) response: reqwest::Response,
    pub(super) buffer: Vec<u8>,
}
impl Live {
    async fn open(f: &Fixture, who: usize, subscriptions: Value) -> Result<Self, (u16, Value)> {
        let response = f
            .client
            .get(format!("{}/api/v1/live", f.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", &f.cookies[who])
            .query(&[("subscriptions", subscriptions.to_string())])
            .send()
            .await
            .unwrap();
        if response.status() != 200 {
            return Err((response.status().as_u16(), response.json().await.unwrap()));
        }
        Ok(Self {
            response,
            buffer: Vec::new(),
        })
    }
    pub(super) async fn next(&mut self) -> Option<Value> {
        loop {
            if let Some(i) = self.buffer.windows(2).position(|w| w == b"\n\n") {
                let bytes: Vec<_> = self.buffer.drain(..i + 2).collect();
                let text = std::str::from_utf8(&bytes).unwrap();
                if let Some(data) = text.lines().find_map(|s| s.strip_prefix("data: ")) {
                    return Some(serde_json::from_str(data).unwrap());
                }
            }
            match tokio::time::timeout(Duration::from_secs(5), self.response.chunk()).await {
                Ok(Ok(Some(bytes))) => self.buffer.extend_from_slice(&bytes),
                _ => return None,
            }
        }
    }
}
#[tokio::test]
async fn live_hides_foreign_spaces_and_bookmarks_and_requires_session() {
    let f = Fixture::new().await;
    let foreign = Live::open(&f, 2, json!([{"space_id":f.spaces[0],"bookmark":null}]))
        .await
        .err()
        .unwrap();
    let absent = Live::open(
        &f,
        2,
        json!([{"space_id":uuid::Uuid::now_v7().to_string(),"bookmark":null}]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(foreign, absent);
    assert_eq!(foreign.0, 404);
    let response = f
        .client
        .get(format!("{}/api/v1/live", f.base))
        .header("host", "localhost:18080")
        .query(&[(
            "subscriptions",
            json!([{"space_id":f.spaces[0],"bookmark":null}]).to_string(),
        )])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    let mut own = Live::open(&f, 1, json!([{"space_id":f.spaces[0],"bookmark":null}]))
        .await
        .ok()
        .unwrap();
    let ready = own.next().await.unwrap();
    assert_eq!(ready["type"], "ready");
    let mut other = Live::open(
        &f,
        2,
        json!([{"space_id":f.spaces[1],"bookmark":ready["bookmark"]}]),
    )
    .await
    .ok()
    .unwrap();
    let reset = other.next().await.unwrap();
    assert_eq!(reset["type"], "reset");
    assert!(other.next().await.is_none());
}
#[tokio::test]
async fn live_replays_after_server_and_client_restart_and_stops_on_logout() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let mut live = Live::open(&f, 1, json!([{"space_id":space,"bookmark":null}]))
        .await
        .ok()
        .unwrap();
    let ready = live.next().await.unwrap();
    let bookmark = ready["bookmark"].clone();
    drop(live);
    for i in 0..5 {
        f.op(
            1,
            "notes.create",
            json!({"space_id":space,"title":format!("{i}"),"markdown":"backlog"}),
        )
        .await
        .ok();
    }
    f.stop();
    f.start().await;
    let mut live = Live::open(&f, 1, json!([{"space_id":space,"bookmark":bookmark}]))
        .await
        .ok()
        .unwrap();
    let mut seqs = Vec::new();
    loop {
        let frame = live.next().await.unwrap();
        if frame["type"] == "ready" {
            break;
        }
        assert_eq!(frame["type"], "entry");
        seqs.push(frame["entry"]["seq"].as_i64().unwrap());
    }
    assert_eq!(seqs.len(), 5);
    assert!(seqs.windows(2).all(|p| p[1] == p[0] + 1));
    assert_eq!(
        f.auth("logout", Some(&f.cookies[1]), json!({}))
            .await
            .status,
        204
    );
    assert!(
        live.next().await.is_none(),
        "revoked session must close while idle"
    );
}
