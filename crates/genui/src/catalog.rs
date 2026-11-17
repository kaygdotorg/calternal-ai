//! One data catalog is consumed by both languages; no platform components or effects are linked.
use serde_json::Value;
use std::sync::OnceLock;
static CATALOG: OnceLock<Value> = OnceLock::new();
/// Return the immutable native catalog, actions and pure-function declarations.
/// The first call parses the embedded JSON in O(catalog bytes), allocating its tree under
/// OnceLock initialization; later calls are O(1) atomic reads. No I/O, fsyncs or round trips.
pub fn catalog() -> &'static Value {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../packages/genui/src/catalog-data.json"
        ))
        .expect("embedded catalog is checked by conformance tests")
    })
}
pub(crate) fn component(name: &str) -> Option<&'static Value> {
    catalog()["components"].get(name)
}
pub(crate) fn action(name: &str) -> Option<&'static Value> {
    catalog()["actions"].get(name)
}
pub(crate) fn constant(name: &str) -> Option<f64> {
    catalog()["constants"].get(name).and_then(Value::as_f64)
}
pub(crate) fn function_arity(name: &str) -> Option<(usize, isize)> {
    Some(match name {
        "round" | "fmt" => (1, 2),
        "min" | "max" => (1, -1),
        "pow" | "atan2" => (2, 2),
        "clamp" | "lerp" => (3, 3),
        "range" => (2, 3),
        "sum" | "abs" | "floor" | "ceil" | "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos"
        | "atan" | "random" | "keys" | "values" | "entries" => (1, 1),
        _ => return None,
    })
}
pub(crate) fn method_arity(name: &str) -> Option<(usize, isize)> {
    Some(match name {
        "map" | "filter" | "find" | "some" | "every" | "includes" | "indexOf" | "split"
        | "startsWith" | "endsWith" => (1, 1),
        "join" | "toFixed" => (0, 1),
        "slice" => (0, 2),
        "trim" | "toUpperCase" | "toLowerCase" => (0, 0),
        _ => return None,
    })
}
pub(crate) fn lambda_method(name: &str) -> bool {
    matches!(name, "map" | "filter" | "find" | "some" | "every")
}
pub(crate) fn params(spec: &Value) -> &[Value] {
    spec["params"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub(crate) fn required(spec: &Value) -> Vec<&str> {
    spec["accessibility"]["required"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
