use calternal_core::*;
use serde_json::{Value, json};
fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../contract/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
#[test]
fn request_fixture_corpus() {
    let cases: Vec<Value> = serde_json::from_str(&fixture("request-cases.json")).unwrap();
    for case in cases {
        let result = parse_request(&serde_json::to_vec(&case["value"]).unwrap());
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}: {result:?}",
            case["name"]
        );
    }
}
#[test]
fn genui_fixture_corpus() {
    let cases: Vec<Value> = serde_json::from_str(&fixture("genui-cases.json")).unwrap();
    for case in cases {
        let result = serde_json::from_value::<GenUiView>(case["value"].clone())
            .map_err(|e| e.to_string())
            .and_then(|v| validate_genui(&v).map_err(|e| e.to_string()));
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}: {result:?}",
            case["name"]
        );
    }
}
#[test]
fn schema_and_catalog_snapshots_do_not_drift() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../contract/v0.schema.json")).unwrap();
    assert_eq!(
        schema,
        serde_json::to_value(schemars::schema_for!(Contract)).unwrap()
    );
    let catalog: Value =
        serde_json::from_str(include_str!("../../../contract/operations.json")).unwrap();
    assert_eq!(catalog, serde_json::to_value(operation_catalog()).unwrap());
}
#[test]
fn unknown_outcome_keeps_id_and_input_and_checks_revision() {
    let request: OperationRequest =
        serde_json::from_str(&fixture("operation-request.json")).unwrap();
    let pending = PendingRequest::with_id(
        &request.request_id,
        &request.operation,
        request.input.clone(),
    )
    .unwrap();
    let retry = pending.clone();
    assert_eq!(retry.request(), &request);
    assert_eq!(
        retry.check_base_revision("newer").unwrap_err().code,
        ErrorCode::Conflict
    );
    retry.check_base_revision("opaque-1").unwrap();
    let original = json!({"z":{"b":2,"a":1},"a":0});
    let reordered = json!({"a":0,"z":{"a":1,"b":2}});
    assert_eq!(
        canonical_request_input(&serde_json::from_value(original).unwrap()),
        canonical_request_input(&serde_json::from_value(reordered).unwrap())
    );
}
#[test]
fn hostile_tree_limits_and_utf8_bytes() {
    let mut node = GenUiNode::Text {
        id: "leaf".into(),
        text: "text".into(),
    };
    for i in 0..8 {
        node = GenUiNode::Stack {
            id: format!("depth-{i}"),
            children: vec![node],
        };
    }
    assert!(
        validate_genui(&GenUiView {
            genui_version: 1,
            root: node
        })
        .is_err()
    );
    let node = GenUiNode::Stack {
        id: "root".into(),
        children: (0..128)
            .map(|i| GenUiNode::Text {
                id: format!("node-{i}"),
                text: String::new(),
            })
            .collect(),
    };
    assert!(
        validate_genui(&GenUiView {
            genui_version: 1,
            root: node
        })
        .is_err()
    );
    assert!(
        validate_genui(&GenUiView {
            genui_version: 1,
            root: GenUiNode::Text {
                id: "utf8".into(),
                text: "🦀".repeat(2049)
            }
        })
        .is_err()
    );
    let node = GenUiNode::Stack {
        id: "root".into(),
        children: (0..5)
            .map(|i| GenUiNode::Text {
                id: format!("node-{i}"),
                text: "a".repeat(8000),
            })
            .collect(),
    };
    assert!(
        validate_genui(&GenUiView {
            genui_version: 1,
            root: node
        })
        .is_err()
    );
}
#[test]
fn strict_envelope_and_response_discriminants() {
    let success = OperationResponse::success("id", json!({"note":null}));
    assert_eq!(
        serde_json::to_value(success).unwrap(),
        json!({"request_id":"id","ok":true,"data":{"note":null}})
    );
    for invalid in [
        json!({"request_id":"id","ok":false,"data":{}}),
        json!({"request_id":"id","ok":true,"error":{"code":"internal","message":"x"}}),
        json!({"request_id":"id","ok":true,"data":{},"error":{"code":"internal","message":"x"}}),
    ] {
        assert!(serde_json::from_value::<OperationResponse<Value>>(invalid).is_err());
    }
    assert!(
        serde_json::from_value::<OperationRequest>(
            json!({"request_id":"id","operation":"notes.list","input":[]})
        )
        .is_err()
    );
}
#[test]
fn input_limits_and_errors() {
    assert_eq!(
        validate_markdown(&"x".repeat(MAX_DOCUMENT_BYTES + 1))
            .unwrap_err()
            .code,
        ErrorCode::TooLarge
    );
    assert!(validate_title("\nname").is_err());
    assert!(validate_title(&"🦀".repeat(256)).is_ok());
    assert!(validate_title(&"🦀".repeat(257)).is_err());
    assert_eq!(ErrorCode::Conflict.http_status(), 409);
    assert_eq!(ErrorCode::TooLarge.http_status(), 413);
    let req = OperationRequest {
        request_id: "id".into(),
        operation: "future.plugin".into(),
        input: JsonObject::new(),
    };
    assert!(validate_client_request(&req).is_ok());
    assert_eq!(
        validate_request(&req).unwrap_err().code,
        ErrorCode::NotFound
    );
}
#[test]
fn extension_cannot_lie_about_mutating_host_action() {
    let view:GenUiView=serde_json::from_value(json!({"genui_version":1,"root":{"type":"button","id":"bad","label":"Run","action":{"operation":"notes.create","input":{"space_id":"personal","title":"Bad","markdown":"bad"}}}})).unwrap();
    let manifest = PluginManifest {
        id: "notes-insights".into(),
        version: "0.1.0".into(),
        api_version: 1,
        name: "Insights".into(),
        description: "Test".into(),
        dependencies: vec![],
        capabilities: vec![],
        operations: vec![PluginOperation {
            id: "notes.create".into(),
            description: "Lies about mutation".into(),
            mutates: False,
        }],
        views: vec![],
    };
    assert!(validate_genui(&view).is_ok());
    assert!(validate_extension_view(&view, &manifest).is_err());
}
#[test]
fn required_nullable_response_fields_are_not_optional() {
    assert!(
        serde_json::from_value::<Failure>(
            json!({"ok":false,"error":{"code":"invalid_input","message":"bad"}})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<NotesHistoryResult>(json!({"versions":[]})).is_err());
    assert!(
        serde_json::from_value::<NotesHistoryResult>(json!({"versions":[],"next_cursor":null}))
            .is_ok()
    );
    assert!(
        serde_json::from_value::<ApiError>(
            json!({"code":"invalid_input","message":"bad","details":null})
        )
        .is_err()
    );
}
