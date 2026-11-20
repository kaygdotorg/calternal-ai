use super::*;
use serde_json::json;

#[test]
fn contribution_preflight_checks_encoded_bytes_including_escape_and_lf() {
    let declarations = json!({
        "input_schema":{
            "type":"object",
            "additionalProperties":false,
            "required":["text"],
            "properties":{"text":{"type":"string","minLength":1}}
        },
        "output_schema":{}
    });
    let interface = ContributionInterface::check(&declarations, &declarations).unwrap();
    let harness = HarnessRef("harness".into());
    let reserved = "\u{0000}".repeat(128);
    let empty = serde_json::to_vec(&json!({
        "request_id":reserved,
        "execution":reserved,
        "contribution":"harness",
        "input":{"text":""}
    }))
    .unwrap();
    let text_budget = 16_384 - empty.len() - 1;
    let input = format!(
        "{}{}",
        "\t".repeat(text_budget / 2),
        "x".repeat(text_budget % 2)
    );
    interface
        .preflight(&harness, &json!({"text":input}))
        .unwrap();

    let error = interface
        .preflight(&harness, &json!({"text":format!("{input}x")}))
        .unwrap_err();
    assert_eq!(error.code.http_status(), 413);
    assert_eq!(error.details.unwrap()["reason"], "contribution_frame_limit");
    assert!(interface.preflight(&harness, &json!({"text":""})).is_err());
}

#[test]
fn contribution_launch_reuses_the_admitted_input() {
    let schema = json!({"input_schema":{"type":"string"}, "output_schema":{}});
    let interface = ContributionInterface::check(&schema, &schema).unwrap();
    let request = RequestId("request".into());
    let execution = ExecutionRef("attempt".into());
    let harness = HarnessRef("checked-harness".into());
    let admitted = json!("approved");
    let bytes = interface
        .encode_entry(&request, &execution, &harness, "call", &admitted, &admitted)
        .unwrap();
    assert!(bytes.ends_with(b"\n"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"call":"call","input":"approved"})
    );
    let changed = interface
        .encode_entry(
            &request,
            &execution,
            &harness,
            "call",
            &admitted,
            &json!("changed"),
        )
        .unwrap_err();
    assert_eq!(changed, Error::request_reused());
}
