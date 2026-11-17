//! Helpers for golden and JSON-shape tests of command output; enabled by the
//! `testing` feature, which only dev-dependencies turn on.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Compare `actual` with the golden file `path`. With `UPDATE_GOLDEN=1` the file
/// is written instead, so a reviewer sees the change in the diff. Panics on a
/// mismatch or a missing file, showing both texts.
pub fn assert_golden(path: &Path, actual: &str) {
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        std::fs::write(path, actual).expect("write golden");
        return;
    }
    let want = std::fs::read_to_string(path).unwrap_or_else(|_| {
        panic!(
            "missing golden {} (run with UPDATE_GOLDEN=1)",
            path.display()
        )
    });
    assert_eq!(
        want,
        actual,
        "output differs from {} (UPDATE_GOLDEN=1 rewrites it)",
        path.display()
    );
}

fn shapes() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/cli-shapes.json");
    serde_json::from_slice(&std::fs::read(&path).expect("docs/cli-shapes.json"))
        .expect("shapes JSON")
}

/// Check that `doc` (a whole `--json` document) matches the shape documented under
/// `name` in `docs/cli-shapes.json`, `name` being `<command>` for success and
/// `error` for failures. Leaves are `"string"`, `"integer"`, `"bool"`, `"any"`,
/// `"string?"` (string or null); objects must have exactly the listed keys (a key
/// ending in `?` is optional); an array lists one element shape that every element
/// must match. Panics with the JSON path of the first difference.
pub fn assert_shape(name: &str, doc: &Value) {
    let all = shapes();
    let ok = doc["ok"].as_bool().expect("document has ok");
    let shape = if ok {
        let mut s = serde_json::json!({"ok":"bool","request_id?":"string"});
        s["data"] = all["commands"][name].clone();
        assert!(!s["data"].is_null(), "no shape documented for {name}");
        s
    } else {
        all["error"].clone()
    };
    check("$", &shape, doc);
}

fn check(at: &str, shape: &Value, v: &Value) {
    match shape {
        Value::String(t) => {
            let good = match t.as_str() {
                "string" => v.is_string(),
                "string?" => v.is_string() || v.is_null(),
                "integer" => v.is_i64() || v.is_u64(),
                "bool" => v.is_boolean(),
                "any" => true,
                other => panic!("unknown shape leaf {other} at {at}"),
            };
            assert!(good, "{at}: expected {t}, got {v}");
        }
        Value::Array(items) => {
            let arr = v
                .as_array()
                .unwrap_or_else(|| panic!("{at}: expected array, got {v}"));
            for (i, e) in arr.iter().enumerate() {
                check(&format!("{at}[{i}]"), &items[0], e);
            }
        }
        Value::Object(fields) => {
            let obj = v
                .as_object()
                .unwrap_or_else(|| panic!("{at}: expected object, got {v}"));
            for (k, s) in fields {
                let (name, optional) = k
                    .strip_suffix('?')
                    .map_or((k.as_str(), false), |n| (n, true));
                match obj.get(name) {
                    Some(x) => check(&format!("{at}.{name}"), s, x),
                    None => assert!(optional, "{at}: missing {name}"),
                }
            }
            for k in obj.keys() {
                assert!(
                    fields.contains_key(k) || fields.contains_key(&format!("{k}?")),
                    "{at}: undocumented field {k}"
                );
            }
        }
        _ => panic!("bad shape at {at}"),
    }
}

/// One text snapshot of a finished process: exit status, stdout and stderr, so a
/// golden file shows everything a user or script would see.
pub fn transcript(exit: Option<i32>, stdout: &str, stderr: &str) -> String {
    format!(
        "exit: {}\n--- stdout ---\n{stdout}--- stderr ---\n{stderr}",
        exit.map_or("signal".to_owned(), |c| c.to_string())
    )
}
