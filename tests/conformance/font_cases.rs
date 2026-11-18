//! Account appearance settings and original-format fonts through the real HTTP API.

use super::*;

/// Account-scoped appearance and font operations covered by these fixtures.
pub(super) const OPERATIONS: &[&str] = &[
    "appearance.get",
    "appearance.patch",
    "fonts.catalogue",
    "fonts.install",
    "fonts.status",
    "fonts.faces",
    "fonts.list",
    "fonts.remove",
];

#[tokio::test]
async fn appearance_patch_is_private_and_survives_restart() {
    let mut f = Fixture::new().await;
    let initial = f.op(0, "appearance.get", json!({})).await;
    initial.ok();
    for (key, device_value) in [
        ("scheme", json!("dark")),
        ("uiSize", json!(120)),
        ("appOrder", json!(["notes"])),
        ("chatWidth", json!("centered")),
        ("clarity", json!(0.75)),
    ] {
        assert!(
            initial.body["data"].get(key).is_none(),
            "{key} is device-local"
        );
        let patch = Value::Object([(key.to_owned(), device_value)].into_iter().collect());
        assert_eq!(f.op(0, "appearance.patch", patch).await.status, 400);
    }
    let updated = f
        .op(
            0,
            "appearance.patch",
            json!({
                "brand": "मेरे Fonts", "motion": false,
                "materials": {"blur": false},
                "fonts": {"terminal": {"family": "System", "source": "system"}}
            }),
        )
        .await;
    updated.ok();
    assert_eq!(
        updated.body["data"]["materials"],
        json!({"blur":false, "dim":true, "tint":true})
    );
    assert_eq!(
        f.op(1, "appearance.get", json!({})).await.body["data"],
        initial.body["data"]
    );
    let cleared = f
        .op(
            0,
            "appearance.patch",
            json!({"wallpaperId":null,"themeId":null}),
        )
        .await;
    cleared.ok();
    assert!(cleared.body["data"]["wallpaperId"].is_null());
    assert_eq!(cleared.body["data"]["brand"], "मेरे Fonts");
    f.stop();
    f.start().await;
    assert_eq!(
        f.op(0, "appearance.get", json!({})).await.body["data"],
        cleared.body["data"]
    );
}

fn ttf_multipart() -> (Vec<u8>, Vec<u8>) {
    let mut ttf = vec![0; 12];
    ttf[..4].copy_from_slice(b"\x00\x01\x00\x00");
    let mut body = b"--test-font\r\nContent-Disposition: form-data; name=\"family\"\r\n\r\nPersonal Test\r\n--test-font\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bold-italic.ttf\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
    body.extend(&ttf);
    body.extend(b"\r\n--test-font--\r\n");
    (ttf, body)
}

#[tokio::test]
async fn uploaded_font_keeps_its_format_and_owner_boundary() {
    let f = Fixture::new().await;
    let (ttf, body) = ttf_multipart();
    let reply = f
        .client
        .post(format!("{}/api/v1/fonts/upload", f.base))
        .header("host", "localhost:18080")
        .header("cookie", &f.cookies[0])
        .header("content-type", "multipart/form-data; boundary=test-font")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status(), 200, "{}", reply.text().await.unwrap());
    let list = f.op(0, "fonts.list", json!({})).await;
    list.ok();
    let row = &list.body["data"]["families"][0];
    assert_eq!(row["family"], "Personal Test");
    let id = row["id"].as_str().unwrap();
    let faces = f.op(0, "fonts.faces", json!({"id":id})).await;
    faces.ok();
    let css = faces.body["data"]["css"].as_str().unwrap();
    assert!(css.contains("font-style:italic"));
    assert!(css.contains("font-weight:700"));
    let path = css
        .split_once("url(\"")
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .0;
    let request = |person: usize| {
        f.client
            .get(format!("{}{path}", f.base))
            .header("host", "localhost:18080")
            .header("cookie", &f.cookies[person])
    };
    let own = request(0).send().await.unwrap();
    assert_eq!(own.status(), 200);
    assert_eq!(own.headers()["content-type"], "font/ttf");
    assert_eq!(own.headers()["cache-control"], "no-store");
    assert_eq!(own.bytes().await.unwrap().as_ref(), &ttf);
    assert_eq!(
        request(1)
            .header("if-none-match", "*")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    f.op(
        0,
        "appearance.patch",
        json!({"fonts":{"chat":{"family":"Personal Test","source":"upload","id":id}}}),
    )
    .await
    .ok();
    f.op(0, "fonts.remove", json!({"id":id})).await.ok();
    assert_eq!(request(0).send().await.unwrap().status(), 404);
    assert_eq!(
        f.op(0, "appearance.get", json!({})).await.body["data"]["fonts"]["chat"]["family"],
        "Google Sans Flex"
    );
}

#[tokio::test]
async fn disabled_font_downloads_leave_account_settings_and_local_status_available() {
    let mut f = Fixture::new().await;
    f.stop();
    std::fs::write(f.dir.path().join("fonts-off"), b"").unwrap();
    f.start().await;
    let page = f.op(0, "fonts.catalogue", json!({})).await;
    page.ok();
    assert_eq!(
        page.body["data"],
        json!({"families":[],"next_cursor":null,"partial":true})
    );
    let id = format!("google-{}", "0".repeat(64));
    let info = f.op(0, "fonts.status", json!({"id":id})).await;
    info.ok();
    assert_eq!(
        info.body["data"],
        json!({"state":"missing","downloads_enabled":false})
    );
    assert_eq!(f.op(0, "fonts.install", json!({"id":id})).await.status, 503);
    f.op(0, "appearance.patch", json!({"motion":false}))
        .await
        .ok();
}

#[tokio::test]
async fn font_upload_waiting_for_its_body_cannot_write_after_sign_out() {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpStream,
    };

    let mut f = Fixture::new().await;
    let (_, body) = ttf_multipart();
    let mut stream = BufReader::new(
        TcpStream::connect(f.base.strip_prefix("http://").unwrap())
            .await
            .unwrap(),
    );
    let headers = format!(
        "POST /api/v1/fonts/upload HTTP/1.1\r\nHost: localhost:18080\r\nCookie: {}\r\n\
         Expect: 100-continue\r\nContent-Type: multipart/form-data; boundary=test-font\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        f.cookies[0],
        body.len(),
    );
    stream
        .get_mut()
        .write_all(headers.as_bytes())
        .await
        .unwrap();
    let mut interim = String::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_line(&mut interim))
        .await
        .unwrap()
        .unwrap();
    assert!(interim.starts_with("HTTP/1.1 100 "), "{interim:?}");
    let revoked = f.auth("logout", Some(&f.cookies[0]), json!({})).await;
    assert_eq!(revoked.status, 204);
    stream.get_mut().write_all(&body).await.unwrap();
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(response.contains("HTTP/1.1 401 "), "{response}");
    let session = f.login(0).await;
    session.ok();
    f.cookies[0] = session.cookie();
    let rows = f.op(0, "fonts.list", json!({})).await;
    rows.ok();
    assert_eq!(rows.body["data"]["families"], json!([]));
}
