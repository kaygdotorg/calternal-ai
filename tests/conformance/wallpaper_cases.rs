//! Installation catalog and same-origin image admission, using real passkeys.

use super::*;

#[tokio::test]
async fn wallpaper_catalog_and_renditions_enforce_the_public_boundary() {
    const RID: &str = "/api/v1/wallpapers";
    const DEFAULT: &str = "EX_STserBZM";
    const JPEG: &[u8] = b"\xff\xd8\xffstub\xff\xd9";
    let mut f = Fixture::new().await;
    let anon = f
        .req(
            "POST",
            "/api/v1/operations",
            None,
            json!({"request_id":"anon","operation":"wallpapers.list","input":{}}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(anon.status, 401);
    let catalog = f.op(0, "wallpapers.list", json!({})).await;
    catalog.ok();
    assert_eq!(catalog.body["data"]["default_id"], DEFAULT);
    let photos = catalog.body["data"]["wallpapers"].as_array().unwrap();
    assert_eq!(photos.len(), 8);
    assert_eq!(
        catalog.body["data"],
        f.op(1, "wallpapers.list", json!({})).await.body["data"]
    );
    for photo in photos {
        assert!(
            photo["author_url"]
                .as_str()
                .unwrap()
                .starts_with("https://unsplash.com/@")
        );
        assert!(
            photo["link"]
                .as_str()
                .unwrap()
                .contains("utm_source=calternal&utm_medium=referral")
        );
    }
    assert_eq!(
        f.op(0, "wallpapers.list", json!({"query":"no search yet"}))
            .await
            .status,
        400
    );
    let blur = format!("{RID}/{DEFAULT}/blur");
    let full = format!("{RID}/{DEFAULT}/full");
    let absent = f.req("GET", &blur, None, json!({}), Some(ORIGIN)).await;
    assert_eq!(absent.status, 404);
    assert_eq!(
        absent.body["error"]["message"],
        "Unsplash wallpapers are not configured"
    );
    assert_eq!(absent.headers["cache-control"], "no-store");
    assert_eq!(
        f.req("GET", &full, None, json!({}), Some(ORIGIN))
            .await
            .status,
        401
    );
    let absent = f
        .req("GET", &full, Some(&f.cookies[0]), json!({}), Some(ORIGIN))
        .await;
    assert_eq!(
        absent.body["error"]["message"],
        "Unsplash wallpapers are not configured"
    );

    // Seed only synthetic cached bytes. The real server exercises the key-enabled
    // HTTP surface without a DNS lookup or sending credentials outside the fixture.
    f.stop();
    std::fs::write(f.dir.path().join("wallpaper-cache-case"), b"").unwrap();
    let cache = f.appdata().join("wallpapers");
    std::fs::create_dir(&cache).unwrap();
    for name in [
        "blur",
        "blur-2x",
        "blur-3x",
        "blur-light",
        "blur-light-2x",
        "blur-light-3x",
        "full",
    ] {
        std::fs::write(cache.join(format!("{DEFAULT}.{name}")), JPEG).unwrap();
    }
    f.start().await;
    for name in [
        "blur",
        "blur-2x",
        "blur-3x",
        "blur-light",
        "blur-light-2x",
        "blur-light-3x",
    ] {
        let path = format!("{RID}/{DEFAULT}/{name}");
        let response = f.req("GET", &path, None, json!({}), None).await;
        assert_eq!(response.status, 200);
        assert_eq!(response.headers["content-type"], "image/jpeg");
        assert_eq!(response.headers["cache-control"], "public, max-age=2592000");
        assert!(response.headers["content-security-policy"].contains("img-src 'self'"));
        assert_eq!(response.headers["x-content-type-options"], "nosniff");
        let revalidated = f
            .client
            .get(format!("{}{path}", f.base))
            .header("host", "localhost:18080")
            .header("if-none-match", &response.headers["etag"])
            .send()
            .await
            .unwrap();
        assert_eq!(revalidated.status(), 304);
    }
    for path in [
        &full,
        &format!("{RID}/fbV03MdVtJA/blur"),
        &format!("{RID}/{DEFAULT}/thumb"),
    ] {
        assert_eq!(f.req("GET", path, None, json!({}), None).await.status, 401);
    }
    let signed_in = f
        .req("GET", &full, Some(&f.cookies[0]), json!({}), Some(ORIGIN))
        .await;
    assert_eq!(signed_in.status, 200);
    assert_eq!(
        signed_in.headers["cache-control"],
        "private, max-age=2592000"
    );
    assert_eq!(signed_in.headers["vary"], "Cookie, Authorization");
    for (cookie, expected) in [(None, 401), (Some(&f.cookies[0]), 304)] {
        let mut request = f
            .client
            .get(format!("{}{full}", f.base))
            .header("host", "localhost:18080")
            .header("if-none-match", &signed_in.headers["etag"]);
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        assert_eq!(request.send().await.unwrap().status(), expected);
    }
    assert_eq!(
        f.req(
            "GET",
            &blur,
            None,
            json!({}),
            Some("https://foreign.invalid")
        )
        .await
        .status,
        401
    );
    for path in [
        format!("{RID}/unknown/full"),
        format!("{RID}/{DEFAULT}/orig"),
        format!("{RID}/fbV03MdVtJA/blur"),
    ] {
        assert_eq!(
            f.req("GET", &path, Some(&f.cookies[0]), json!({}), Some(ORIGIN))
                .await
                .status,
            404
        );
    }
}
