//! How `cl` shows each operation's result and each failure. Known operations get
//! tables and labelled blocks; any other operation (via `cl invoke`) prints its
//! data as indented JSON, the same text `--json` carries under `data`.
//! Server data is read as untyped JSON on purpose: a newer server's extra fields
//! must not break an older `cl`.

use calternal_cli::output::{CliError, Output, Reply, json_text};
use calternal_core::{ApiError, DescribeResult, ErrorCode};
use serde_json::Value;

/// A string field of `v`, empty when absent.
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn note_fields(out: &Output, note: &Value) -> Vec<(String, String)> {
    vec![
        ("id".into(), s(note, "id").into()),
        ("space".into(), s(note, "space_id").into()),
        ("title".into(), s(note, "title").into()),
        ("revision".into(), s(note, "revision").into()),
        ("updated".into(), out.local_rfc3339(s(note, "updated_at"))),
    ]
}

fn plugin_fields(plugin: &Value) -> Vec<(String, String)> {
    let m = &plugin["manifest"];
    vec![
        ("id".into(), s(m, "id").into()),
        ("name".into(), s(m, "name").into()),
        ("version".into(), s(m, "version").into()),
        ("enabled".into(), yes(plugin["enabled"].as_bool()).into()),
        ("generation".into(), plugin["generation"].to_string()),
    ]
}

fn yes(b: Option<bool>) -> &'static str {
    if b == Some(true) { "yes" } else { "no" }
}

fn rows(v: &Value, key: &str) -> Vec<Value> {
    v.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The reply for `operation`'s successful `data`; `--json` carries `data` unchanged.
pub fn operation(out: &Output, operation: &str, request_id: &str, data: Value) -> Reply {
    if operation.starts_with("requests.") {
        return calternal_cli::requests::reply(data, request_id);
    }
    if calternal_core::is_account_input_operation(operation) {
        return calternal_cli::account_input::render(out, operation, request_id, data);
    }
    if let Some(reply) = calternal_cli::people::render(out, operation, request_id, &data) {
        return reply;
    }
    if operation != "notes.list" && calternal_core::note_operation(operation).is_some() {
        return calternal_cli::notes::reply(out, operation, data).request_id(request_id);
    }
    if let Some(op) = calternal_core::SearchOperation::from_id(operation) {
        return calternal_cli::search::reply(op, data).request_id(request_id);
    }
    let reply = Reply::new(data.clone()).request_id(request_id);
    match operation {
        "notes.list" => {
            let notes = rows(&data, "notes");
            let hint = if notes.is_empty() {
                "Next: run `cl notes create --help`."
            } else {
                "Next: run `cl notes get ID`."
            };
            reply
                .table(
                    &["ID", "TITLE", "UPDATED", "REVISION"],
                    notes
                        .iter()
                        .map(|n| {
                            vec![
                                s(n, "id").into(),
                                s(n, "title").into(),
                                out.local_rfc3339(s(n, "updated_at")),
                                s(n, "revision").into(),
                            ]
                        })
                        .collect(),
                    "This space has no notes yet; run `cl notes create` to add one.",
                )
                .hint(hint)
                .page_cursor()
        }
        "notes.get" | "notes.create" | "notes.update" => {
            let note = &data["note"];
            let hint = match operation {
                "notes.create" => "Next: run `cl notes get ID` to open the note.",
                "notes.update" => "Next: run `cl notes get ID` to check the saved version.",
                _ => "Next: run `cl notes update ID` to edit this note.",
            };
            reply
                .fields(note_fields(out, note))
                .blank()
                .text(s(note, "markdown"))
                .hint(hint)
        }
        "notes.history" => {
            let versions = rows(&data, "versions");
            let mut r = reply.table(
                &["REVISION", "TITLE", "UPDATED"],
                versions
                    .iter()
                    .map(|v| {
                        vec![
                            s(v, "revision").into(),
                            s(v, "title").into(),
                            out.local_rfc3339(s(v, "updated_at")),
                        ]
                    })
                    .collect(),
                "No earlier versions were found. Run `cl notes get ID` to view the current version.",
            );
            if let Some(c) = data.get("next_cursor").and_then(Value::as_str) {
                r = r
                    .blank()
                    .line(format!("More versions: repeat with --cursor {c}"));
            }
            r
        }
        "plugins.list" => {
            let plugins = rows(&data, "plugins");
            let hint = if plugins.is_empty() {
                "Next: run `cl describe` to see what this server offers."
            } else {
                "Next: run `cl plugins preview ID`."
            };
            reply
                .table(
                    &["ID", "NAME", "VERSION", "ENABLED", "GENERATION"],
                    plugins
                        .iter()
                        .map(|p| {
                            let m = &p["manifest"];
                            vec![
                                s(m, "id").into(),
                                s(m, "name").into(),
                                s(m, "version").into(),
                                yes(p["enabled"].as_bool()).into(),
                                p["generation"].to_string(),
                            ]
                        })
                        .collect(),
                    "No plugins are installed. Ask a space owner to install one.",
                )
                .hint(hint)
        }
        "plugins.enable" | "plugins.disable" => reply.fields(plugin_fields(&data["plugin"])),
        "plugins.preview" => reply
            .line("Preview of the plugin's GenUI view (nothing was changed):")
            .text(json_text(&data["view"]))
            .hint("Next: run `cl plugins enable ID` to turn it on."),
        _ => reply.text(json_text(&data)),
    }
}

/// The reply for `cl describe`; `--json` carries the whole describe result.
pub fn describe(result: &DescribeResult) -> Reply {
    let data = serde_json::to_value(result).expect("describe result serializes");
    let spaces = result
        .principal
        .spaces
        .iter()
        .map(|g| {
            let role = serde_json::to_value(&g.role).ok();
            vec![
                g.id.clone(),
                role.as_ref().and_then(Value::as_str).unwrap_or("").into(),
            ]
        })
        .collect();
    let ops = result
        .operations
        .iter()
        .map(|o| {
            vec![
                o.id.clone(),
                if o.mutates { "writes" } else { "reads" }.into(),
                o.description.clone(),
            ]
        })
        .collect();
    Reply::new(data)
        .fields([
            ("protocol", result.protocol_version.to_string()),
            ("genui", result.genui_version.to_string()),
            ("principal", result.principal.id.clone()),
        ])
        .blank()
        .table(
            &["SPACE", "ROLE"],
            spaces,
            "No spaces are available to this account yet. Ask a space owner to invite you.",
        )
        .blank()
        .table(
            &["OPERATION", "ACCESS", "DESCRIPTION"],
            ops,
            "This server offers no operations yet. Ask its owner to enable a plugin.",
        )
        .hint("Next: run `cl notes list` to see notes in your space.")
}

/// The failure for a server or client error: its stable code, the message, and a
/// next step chosen from the code.
pub fn api_error(error: ApiError, request_id: Option<String>) -> CliError {
    let code = serde_json::to_value(error.code)
        .ok()
        .and_then(|c| c.as_str().map(str::to_owned))
        .unwrap_or_else(|| "internal".into());
    let hint = match error.code {
        ErrorCode::Unauthorized => {
            "Check the credential in CALTERNAL_TOKEN or the file named by --token-file, and that it is for this server."
        }
        ErrorCode::PasskeyReverificationRequired => {
            "Verify with your passkey in the app, then retry there."
        }
        ErrorCode::Forbidden => "Ask a space owner to grant your account the access this needs.",
        ErrorCode::NotFound => "Check the id and the --space; `cl notes list` shows what exists.",
        ErrorCode::Conflict => {
            "Fetch the current version (`cl notes get`), then retry with its revision."
        }
        ErrorCode::InvalidInput => {
            "Fix the input named in the message and run the command again; `--help` shows the options."
        }
        ErrorCode::TooLarge => "Shorten the input, then run the command again.",
        ErrorCode::Unavailable => "Check that the server is running at --base-url, then retry.",
        ErrorCode::Internal => "Retry; if it keeps failing, look at the server's log.",
    };
    let human_hint = match error.code {
        ErrorCode::Unauthorized => {
            "Check `CALTERNAL_TOKEN` or `--token-file`, and make sure the credential belongs to this server."
        }
        ErrorCode::PasskeyReverificationRequired => {
            "Verify with your passkey in the app, then retry there."
        }
        ErrorCode::Forbidden => "Ask a space owner to grant your account the needed access.",
        ErrorCode::NotFound => {
            "Check the ID and space; `cl notes list` shows the notes you can access."
        }
        ErrorCode::Conflict => "Run `cl notes get ID` for the latest revision, then try again.",
        ErrorCode::InvalidInput => {
            "Fix the input named in the message, then run the command again; use `--help` for examples."
        }
        ErrorCode::TooLarge => "Shorten the input, then try again.",
        ErrorCode::Unavailable => "Check `--base-url` and the server's status, then try again.",
        ErrorCode::Internal => {
            "Retry once; if it keeps failing, ask the server owner to check the log."
        }
    };
    let human = match error.code {
        ErrorCode::Unauthorized => format!("Sign-in was not accepted: {}", error.message),
        ErrorCode::PasskeyReverificationRequired => error.message.clone(),
        ErrorCode::Forbidden => format!("Your account does not have access: {}", error.message),
        ErrorCode::NotFound => format!("We couldn't find that item: {}", error.message),
        ErrorCode::Conflict => format!("The saved version has changed: {}", error.message),
        ErrorCode::InvalidInput => format!("The server couldn't use that input: {}", error.message),
        ErrorCode::TooLarge => format!("That input is too large: {}", error.message),
        ErrorCode::Unavailable => format!("The server is unavailable: {}", error.message),
        ErrorCode::Internal => format!("The server hit an unexpected problem: {}", error.message),
    };
    let mut e = CliError::new(code, error.message)
        .hint(hint)
        .human_message(human)
        .human_hint(human_hint);
    if let Some(id) = request_id {
        e = e.request_id(id);
    }
    if let Some(d) = error.details {
        e = e.details(serde_json::to_value(d).unwrap_or(Value::Null));
    }
    e
}

/// The failure for a transport error after the request was sent: the outcome is
/// unknown, so the hint explains the safe retry.
pub fn transport_unknown(error: ApiError, request_id: String) -> CliError {
    let mut e = api_error(error, Some(request_id.clone())).hint(format!(
        "The server may or may not have applied this. Repeat the identical command with --request-id {request_id} to retry safely."
    ));
    e = e.details(serde_json::json!({"outcome":"unknown","retry":"reuse request_id with the identical operation and input"}));
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_cli::testing::{assert_golden, assert_shape};
    use serde_json::json;
    use std::path::PathBuf;
    use time::UtcOffset;

    fn golden(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name)
    }
    fn tz() -> UtcOffset {
        UtcOffset::from_hms(1, 0, 0).unwrap()
    }
    fn note() -> Value {
        json!({"id":"11111111-1111-4111-8111-111111111111","space_id":"22222222-2222-4222-8222-222222222222","title":"Groceries","markdown":"- milk\n- eggs\n","revision":"r-7","updated_at":"2026-11-17T06:12:03Z"})
    }
    fn cases() -> Vec<(&'static str, &'static str, Value)> {
        let version = json!({"title":"Groceries","markdown":"- milk\n","revision":"r-6","updated_at":"2026-11-16T20:00:00Z"});
        let plugin = json!({"manifest":{"id":"word-count","name":"Word count","version":"0.1.0","api_version":1,"description":"d","dependencies":[],"capabilities":[],"operations":[],"views":[]},"enabled":true,"generation":3});
        vec![
            (
                "notes.list",
                "notes-list",
                json!({"notes":[{"id":"11111111-1111-4111-8111-111111111111","space_id":"s","title":"Groceries","revision":"r-7","updated_at":"2026-11-17T06:12:03Z"}]}),
            ),
            ("notes.list", "notes-list-empty", json!({"notes":[]})),
            ("notes.get", "notes-get", json!({"note":note()})),
            ("notes.create", "notes-create", json!({"note":note()})),
            ("notes.update", "notes-update", json!({"note":note()})),
            (
                "notes.history",
                "notes-history",
                json!({"versions":[version],"next_cursor":"c-2"}),
            ),
            (
                "notes.history",
                "notes-history-empty",
                json!({"versions": []}),
            ),
            (
                "plugins.list",
                "plugins-list",
                json!({"plugins":[plugin.clone()]}),
            ),
            ("plugins.list", "plugins-list-empty", json!({"plugins": []})),
            (
                "plugins.enable",
                "plugins-enable",
                json!({"plugin":plugin.clone()}),
            ),
            (
                "plugins.disable",
                "plugins-disable",
                json!({"plugin":plugin}),
            ),
            (
                "plugins.preview",
                "plugins-preview",
                json!({"view":{"genui_version":2,"source":"Text(\"hello\")"},"preview":true}),
            ),
            ("future.read", "invoke", json!({"anything":[1,2]})),
        ]
    }
    #[test]
    fn human_and_json_golden() {
        for (op, name, data) in cases() {
            let human = Output::fixed(false, None, tz()).render(&operation(
                &Output::fixed(false, None, tz()),
                op,
                "req-1",
                data.clone(),
            ));
            assert_golden(&golden(&format!("{name}.txt")), &human);
            let out = Output::fixed(true, None, tz());
            let json = out.render(&operation(&out, op, "req-1", data));
            assert_golden(&golden(&format!("{name}.json")), &json);
            let doc: Value = serde_json::from_str(&json).unwrap();
            assert_shape(
                if name.starts_with("notes-list") {
                    "cl notes list"
                } else {
                    op_key(op)
                },
                &doc,
            );
        }
    }
    fn op_key(op: &str) -> &'static str {
        match op {
            "notes.get" | "notes.create" | "notes.update" => "cl notes get",
            "notes.history" => "cl notes history",
            "plugins.list" => "cl plugins list",
            "plugins.enable" | "plugins.disable" => "cl plugins enable",
            "plugins.preview" => "cl plugins preview",
            _ => "cl invoke",
        }
    }
    #[test]
    fn narrow_width_shortens_the_widest_column_only() {
        let data = json!({"notes":[{"id":"i","space_id":"s","title":"A very long note title that will not fit","revision":"r","updated_at":"2026-11-17T06:12:03Z"}]});
        let out = Output::fixed(false, Some(60), tz());
        let text = out.render(&operation(&out, "notes.list", "r", data));
        assert_golden(&golden("notes-list-narrow.txt"), &text);
        assert!(text.lines().all(|l| l.chars().count() <= 60));
    }
    #[test]
    fn list_goldens_cover_120_80_and_40_column_terminals() {
        let note = json!({
            "id":"12345678-1234-4abc-8def-123456789012",
            "space_id":"22222222-2222-4222-8222-222222222222",
            "title":"東京 groceries and a very long shopping list",
            "revision":"revision-2026-11-17",
            "updated_at":"2026-11-17T06:12:03Z"
        });
        let version = json!({
            "title":"東京 groceries and a very long shopping list",
            "markdown":"- milk\n",
            "revision":"revision-2026-11-17",
            "updated_at":"2026-11-17T06:12:03Z"
        });
        let plugin = json!({
            "manifest":{"id":"calternal-example-plugin-with-a-long-id","name":"Example plugin with a long name","version":"0.1.0","api_version":1,"description":"example","dependencies":[],"capabilities":[],"operations":[],"views":[]},
            "enabled":true,
            "generation":123456
        });
        let lists = [
            ("notes.list", "notes-list-width", json!({"notes":[note]})),
            (
                "notes.history",
                "notes-history-width",
                json!({"versions":[version]}),
            ),
            (
                "plugins.list",
                "plugins-list-width",
                json!({"plugins":[plugin]}),
            ),
        ];
        for width in [120, 80, 40] {
            for (op, name, data) in lists.clone() {
                let out = Output::fixed_terminal(false, width, tz(), true);
                let text = out.render(&operation(&out, op, "req-1", data));
                assert_golden(&golden(&format!("{name}-{width}.txt")), &text);
            }
            let result: DescribeResult = serde_json::from_value(json!({
                "protocol_version":1,
                "genui_version":2,
                "principal":{"id":"12345678-1234-4abc-8def-123456789012","spaces":[{"id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","role":"owner"}]},
                "operations":[{"id":"notes.list","description":"List notes in the selected space","mutates":false,"risk":"read","input_schema":{},"output_schema":{}}]
            })).unwrap();
            let out = Output::fixed_terminal(false, width, tz(), true);
            assert_golden(
                &golden(&format!("describe-width-{width}.txt")),
                &out.render(&describe(&result)),
            );
        }
    }
    #[test]
    fn list_hints_can_be_disabled_without_changing_the_result() {
        let data = json!({"notes": []});
        let with_hint = Output::fixed_terminal(false, 80, tz(), true);
        let without_hint = Output::fixed_terminal(false, 80, tz(), false);
        let reply = operation(&with_hint, "notes.list", "req-1", data);
        assert_golden(&golden("notes-list-hint.txt"), &with_hint.render(&reply));
        assert_golden(
            &golden("notes-list-no-hint.txt"),
            &without_hint.render(&reply),
        );
    }
    #[test]
    fn describe_golden() {
        let result: DescribeResult = serde_json::from_value(json!({
            "protocol_version":1,"genui_version":2,
            "principal":{"id":"acct-1","spaces":[{"id":"space-1","role":"owner"}]},
            "operations":[{"id":"notes.list","description":"List notes","risk":"read","mutates":false,"input_schema":{},"output_schema":{}}]
        })).unwrap();
        let human = Output::fixed(false, None, tz()).render(&describe(&result));
        assert_golden(&golden("describe.txt"), &human);
        let out = Output::fixed(true, None, tz());
        let json = out.render(&describe(&result));
        assert_golden(&golden("describe.json"), &json);
        assert_shape("cl describe", &serde_json::from_str(&json).unwrap());
    }
    #[test]
    fn errors_golden() {
        let api = ApiError {
            code: ErrorCode::Conflict,
            message: "revision is stale".into(),
            details: None,
        };
        let e = api_error(api.clone(), Some("req-1".into()));
        let (_, human) = Output::fixed(false, None, tz()).render_error(&e);
        assert_golden(&golden("error-conflict.txt"), &human);
        let (json, _) = Output::fixed(true, None, tz()).render_error(&e);
        assert_golden(&golden("error-conflict.json"), &json);
        assert_shape("error", &serde_json::from_str(&json).unwrap());
        let t = transport_unknown(
            ApiError {
                code: ErrorCode::Unavailable,
                message: "transport failed; server outcome may be unknown".into(),
                details: None,
            },
            "req-2".into(),
        );
        let (json, human) = Output::fixed(true, None, tz()).render_error(&t);
        assert!(human.is_empty());
        assert_golden(&golden("error-transport.json"), &json);
        assert_shape("error", &serde_json::from_str(&json).unwrap());
    }
}
