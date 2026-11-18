//! Expiry checks for operator-issued first-owner codes.
use super::*;
use crate::test_support::TempDir;

#[tokio::test]
async fn expired_first_owner_candidate_is_replaced_with_the_exact_new_expiry() {
    let dir = TempDir::new();
    let auth = crate::Auth::open(
        dir.path(),
        crate::AuthConfig::new("http://localhost:18080").unwrap(),
    )
    .unwrap();
    let original = auth.first_owner_code(None).await.unwrap().unwrap();
    auth.inner
        .db
        .run(|c| {
            c.execute("UPDATE setup_tokens SET expires_at=0", [])
                .map_err(sql)?;
            Ok(())
        })
        .await
        .unwrap();
    let replacement = auth
        .first_owner_code(Some(original.code.clone()))
        .await
        .unwrap()
        .unwrap();
    assert!(replacement.code != original.code);
    assert!(replacement.expires_ms > now_ms());
    let stored = auth
        .inner
        .db
        .run(|c| {
            c.query_row("SELECT expires_at FROM setup_tokens", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(sql)
        })
        .await
        .unwrap();
    assert_eq!(replacement.expires_ms, stored);
}
