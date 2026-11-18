//! Scoped settings through an ordinary installed SDK worker and the real journal.
use super::*;
use crate::live_cases::Live;

pub(super) const OPERATIONS: &[&str] = &[
    "plugins.settings.get",
    "plugins.settings.set",
    "plugins.personal_settings.get",
    "plugins.personal_settings.set",
];
const PLUGIN: &str = "composer-settings";
const VIEW: &str = "composer-settings.label";

fn address(space: &str) -> Value {
    json!({"space_id":space,"plugin_id":PLUGIN,"key":"label"})
}

fn edit(space: &str, base: Value, value: Value) -> Value {
    let mut input = address(space);
    input["base"] = base;
    input["value"] = value;
    input
}

fn path(f: &Fixture, space: &str) -> PathBuf {
    f.dir
        .path()
        .join("userdata-ssd/spaces")
        .join(space)
        .join(".calternal/settings.json")
}

fn data(response: Answer) -> Value {
    response.ok();
    response.body["data"].clone()
}

async fn badge(f: &Fixture, who: usize, space: &str, text: &str) {
    let result = data(f.op(who, VIEW, json!({"space_id":space})).await);
    assert_eq!(result["view"]["source"], format!("Badge({})", json!(text)));
}

async fn install(f: &Fixture, space: &str) {
    crate::owned_slot_cases::install(
        f,
        &source_root().join("tests/plugins/composer-settings"),
        space,
    );
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":space,"plugin_id":PLUGIN}),
    )
    .await
    .ok();
}

#[tokio::test]
async fn composer_setting_is_live_and_scoped_with_recoverable_offline_edits() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone(); // A Reader belongs to this space.
    install(&f, &space).await;
    let foreign_space = f.spaces[1].clone();
    badge(&f, 2, &space, "Quick note").await;
    let denied = f.op(2, "plugins.settings.get", address(&space)).await;
    assert_eq!(denied.status, 403, "{}", denied.body);
    let denied = f
        .op(
            2,
            "plugins.settings.set",
            edit(&space, Value::Null, json!("blocked")),
        )
        .await;
    assert_eq!(denied.status, 403, "{}", denied.body);
    for (op, input) in [
        ("plugins.settings.get", address(&space)),
        (
            "plugins.settings.set",
            edit(&space, Value::Null, json!("blocked")),
        ),
        (VIEW, json!({"space_id":space})),
    ] {
        assert_eq!(f.op(3, op, input).await.status, 404);
    }
    let foreign = data(
        f.op(2, "plugins.list", json!({"space_id":foreign_space}))
            .await,
    );
    assert!(!foreign["slots"].to_string().contains(PLUGIN));
    let listed = data(f.op(2, "plugins.list", json!({"space_id":space})).await);
    assert!(listed["slots"].to_string().contains("composer_row"));
    let refused = f
        .op(1, VIEW, json!({"space_id":space,"key":"other/private"}))
        .await;
    assert_ne!(refused.status, 200);
    assert!(!refused.body.to_string().contains("private"));
    assert_eq!(
        f.op(
            1,
            "plugins.settings.set",
            edit(&space, Value::Null, json!(17))
        )
        .await
        .status,
        400
    );

    let mut live = Live::open(&f, 2, json!([{"space_id":space,"bookmark":null}]))
        .await
        .ok()
        .unwrap();
    assert_eq!(live.next().await.unwrap()["type"], "ready");
    let first = edit(&space, Value::Null, json!("Project notes"));
    let result = data(
        f.op_id(1, "plugins.settings.set", first.clone(), "setting-save")
            .await,
    );
    assert_eq!(result["conflicted"], false);
    loop {
        let frame = live.next().await.expect("setting invalidation");
        if frame["entry"]["kind"] == "plugins.settings.set" {
            assert_eq!(frame["entry"]["data"]["plugin_id"], PLUGIN);
            assert_eq!(frame["entry"]["data"]["key"], "label");
            assert!(!frame.to_string().contains("Project notes"));
            break;
        }
    }
    badge(&f, 2, &space, "Project notes").await;
    let conflict = edit(&space, Value::Null, json!("Offline notes"));
    assert_eq!(
        data(f.op(1, "plugins.settings.set", conflict).await)["conflicted"],
        true
    );
    let current = data(f.op(1, "plugins.settings.get", address(&space)).await);
    assert_eq!(current["alternatives"], json!(["Offline notes"]));
    assert_eq!(current["effective"], "Project notes");
    f.stop();
    f.start().await;
    badge(&f, 2, &space, "Project notes").await;
    assert_eq!(
        data(
            f.op_id(1, "plugins.settings.set", first, "setting-save")
                .await
        ),
        result
    );
    let mut reset = edit(&space, json!("Project notes"), Value::Null);
    reset["resolve"] = json!(["Offline notes"]);
    data(f.op(1, "plugins.settings.set", reset).await);
    badge(&f, 2, &space, "Quick note").await;
    data(
        f.op(
            1,
            "plugins.disable",
            json!({"space_id":space,"plugin_id":PLUGIN}),
        )
        .await,
    );
    let after = data(f.op(2, "plugins.list", json!({"space_id":space})).await);
    assert!(!after["slots"].to_string().contains(PLUGIN));
    assert_ne!(f.op(2, VIEW, json!({"space_id":space})).await.status, 200);
}

#[tokio::test]
async fn a_damaged_file_holds_the_pending_edit_without_pausing_the_space() {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    install(&f, &space).await;
    data(
        f.op(
            1,
            "plugins.settings.set",
            edit(&space, Value::Null, json!("Before")),
        )
        .await,
    );
    let file = path(&f, &space);
    let prior = std::fs::read(&file).unwrap();
    f.stop();
    let fault = f.dir.path().join("content-fault");
    std::fs::write(&fault, "after_commit").unwrap();
    f.start().await;
    let args = edit(&space, json!("Before"), json!("After"));
    let lost = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("origin", ORIGIN)
        .header("cookie", &f.cookies[1])
        .json(
            &json!({"operation":"plugins.settings.set","request_id":"setting-crash","input":args}),
        )
        .send()
        .await;
    assert!(lost.is_err());
    f.stop();
    std::fs::remove_file(&fault).unwrap();
    let damaged = "{\n  broken\n}";
    std::fs::write(&file, damaged).unwrap();
    f.start().await;
    let read = data(f.op(1, "plugins.settings.get", address(&space)).await);
    assert_eq!(read["writable"], false);
    assert!(
        read["repair"]
            .as_str()
            .unwrap()
            .contains(".calternal/settings.json at line 2"),
        "{read}"
    );
    assert_eq!(read["effective"], "Quick note");
    let failed = f
        .op_id(1, "plugins.settings.set", args.clone(), "setting-crash")
        .await;
    assert_eq!(failed.status, 409, "{}", failed.body);
    assert_eq!(failed.body["error"]["details"]["line"], 2);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), damaged);
    f.note(1, &space).await;
    let mut repaired: Value = serde_json::from_slice(&prior).unwrap();
    repaired["future"] = json!({"manual": true});
    std::fs::write(&file, repaired.to_string()).unwrap();
    badge(&f, 1, &space, "After").await;
    data(
        f.op_id(1, "plugins.settings.set", args, "setting-crash")
            .await,
    );
    let persisted: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(persisted["format"], "calternal-plugin-settings/v1");
    assert_eq!(persisted["settings"]["composer-settings/label"], "After");
    assert_eq!(persisted["future"], json!({"manual": true}));
}

#[tokio::test]
async fn personal_plugin_settings_follow_only_the_signed_in_account() {
    let mut f = Fixture::new().await;
    let space = f.spaces[2].clone();
    install(&f, &space).await;
    let address = json!({"space_id":space,"plugin_id":PLUGIN,"key":"personal-label"});
    let read = "plugins.personal_settings.get";
    let write = "plugins.personal_settings.set";
    let mut edit = address.clone();
    edit["base"] = Value::Null;
    edit["value"] = json!("My private label");

    assert_eq!(
        data(f.op(2, read, address.clone()).await)["effective"],
        "My note"
    );
    let first = data(f.op_id(2, write, edit.clone(), "personal-edit").await);
    assert_eq!(first["effective"], "My private label");
    assert_eq!(
        data(f.op(1, read, address.clone()).await)["effective"],
        "My note"
    );
    assert_eq!(f.op(3, read, address.clone()).await.status, 404);
    assert_eq!(
        data(
            f.op(2, VIEW, json!({"space_id":space,"key":"personal-label"}))
                .await
        )["view"]["source"],
        r#"Badge("My private label")"#
    );
    assert_eq!(
        f.op(1, "plugins.settings.get", address.clone())
            .await
            .status,
        404
    );

    f.stop();
    f.start().await;
    assert_eq!(
        data(f.op(2, read, address).await)["effective"],
        "My private label"
    );
    assert_eq!(data(f.op_id(2, write, edit, "personal-edit").await), first);
}
