//! Compile the reachable sealed wire definitions using the plugin-host validator.
use calternal_core::ManifestV1;
use calternal_plugin_host::CompiledSchemas;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) struct Wire {
    schemas: BTreeMap<&'static str, CompiledSchemas>,
}

const TYPES: &[(&str, &str)] = &[
    ("view_open", "ViewOpenRequest"),
    ("view", "ViewOpenResult"),
    ("window_request", "WindowRequest"),
    ("window", "WindowReply"),
    ("selection", "SettledSelectionRequest"),
    ("selection_context", "SelectionContextResult"),
    ("selection_set", "SelectionSetRequest"),
    ("selection_set_result", "SelectionSetResult"),
    ("choice_request", "ChoiceOptionsRequest"),
    ("choices", "ChoiceOptions"),
    ("resolve", "ResolveActivation"),
    ("resolution", "ActivationResolution"),
    ("confirm", "ConfirmActivation"),
    ("dispatch", "DispatchActivation"),
    ("outcome", "ActionOutcome"),
    ("undo_request", "UndoRequest"),
    ("renew_request", "LeaseRenewRequest"),
    ("renew", "LeaseRenewResult"),
];

fn reachable(schema: &Value, catalogue: &Map<String, Value>, found: &mut Map<String, Value>) {
    match schema {
        Value::Object(fields) => {
            if let Some(name) = fields
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/$defs/"))
            {
                if !found.contains_key(name) {
                    let child = catalogue.get(name).expect("reachable sealed definition");
                    found.insert(name.into(), child.clone());
                    reachable(child, catalogue, found);
                }
            }
            for sub in fields.values() {
                reachable(sub, catalogue, found);
            }
        }
        Value::Array(fields) => {
            for sub in fields {
                reachable(sub, catalogue, found);
            }
        }
        _ => {}
    }
}

impl Wire {
    pub(super) fn new() -> Self {
        let src: Value =
            serde_json::from_str(include_str!("rev13/host-catalogue-v2.json")).unwrap();
        let defs = src["$defs"].as_object().unwrap();
        let schemas = TYPES.iter().map(|&(name,ty)| {
            let reference=format!("#/$defs/{ty}");
            let mut found=Map::new();
            reachable(&json!({"$ref":reference}),defs,&mut found);
            let operation = json!({
                "id":"spikewire.check",
                "input_schema":{"type":"object","$defs":found,"$ref":reference},
                "output_schema":{"type":"object"},
                "required_capability":"space.read","mutates":false,"idempotency":"read_only","risk":"read"
            });
            let manifest: ManifestV1 = serde_json::from_value(json!({
                "id":"spikewire","name":"sealed wire fixture","icon":"puzzle","version":"0.1.0","api_version":1,
                "kinds":[],"operations":[operation],"collections":[],"views":[],"attention":[],
                "settings":[],"schedules":[],"webhooks":[],"hooks":[],"themes":[],"secrets":[],"host_calls":[]
            })).unwrap();
            let checked=CompiledSchemas::compile(&manifest).unwrap_or_else(|e|panic!("wire schema {ty}: {e:?}"));
            (name,checked)
        }).collect();
        Self { schemas }
    }

    pub(super) fn valid(&self, name: &str, value: &Value) -> bool {
        self.schemas[name]
            .validate_input("spikewire.check", value)
            .is_ok()
    }
}
