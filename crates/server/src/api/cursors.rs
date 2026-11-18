//! Stateless wire cursors over ordered keys selected by the existing handlers.
use super::{Directory, Identity, Session, Shared, dispatch::run_admitted_operation};
use crate::authz::Policy;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use calternal_core::{JsonObject, canonical_request_input};
use calternal_types::{AccountId, Error};
use ring::hmac;
use serde_json::{Value, json};

/// Scope cursors after admission and seal returned positions after delivery checks.
pub(super) async fn admitted_operation<D: Directory>(
    state: Shared<D>,
    p: Identity,
    session: Session,
    request_id: &str,
    op: &str,
    mut input: JsonObject,
    agent: Option<crate::turns::access::Authority>,
    policy: Policy,
    job_scoped: bool,
) -> Result<Value, Error> {
    let supports_cursor = |schema: &Value| schema["properties"].get("cursor").is_some();
    let paged = (op == "threads.list"
        && state
            .registry
            .owner_of_operation(op)
            .and_then(|plugin| plugin.operation(op))
            .is_some_and(|d| supports_cursor(&d.input_schema)))
        || calternal_core::cached_operation(op).is_some_and(|d| {
            d.input_schema
                .get("properties")
                .is_some_and(|p| p.get("cursor").is_some())
        });
    let scope = paged
        .then(|| CursorScope::open(&state.store.cursor_key, p.id, op, &mut input))
        .transpose()?;
    let account = p.id;
    let mut result = run_admitted_operation(
        state.clone(),
        p,
        session,
        request_id,
        op,
        input,
        agent,
        policy,
        job_scoped,
    )
    .await?;
    if let Some(scope) = scope {
        scope.finish(account, op, &mut result);
    }
    Ok(result)
}

/// One bounded token scope. A cursor carries only a previously delivered key;
/// account, operation and filter values are authenticated without being encoded.
struct CursorScope<'a> {
    key: &'a hmac::Key,
    scope: String,
}

fn invalid() -> Error {
    Error::invalid_input("invalid page cursor")
}

impl<'a> CursorScope<'a> {
    /// Open an authenticated cursor in-place after normal operation admission.
    /// No storage reads or writes, and no issuer detail on invalid input.
    fn open(
        key: &'a hmac::Key,
        account: AccountId,
        op: &str,
        input: &mut JsonObject,
    ) -> Result<Self, Error> {
        let cursor = input.remove("cursor");
        let scope = Self::new(key, account, op, input);
        if let Some(cursor) = cursor {
            let token = cursor.as_str().ok_or_else(invalid)?;
            calternal_core::validate_page_cursor(token)?;
            input.insert("cursor".into(), Value::String(scope.decode(token)?));
        }
        Ok(scope)
    }

    fn new(key: &'a hmac::Key, account: AccountId, op: &str, input: &JsonObject) -> Self {
        let mut filters = input.clone();
        // Page size does not change the ordered result set.
        filters.remove("limit");
        Self {
            key,
            scope: format!("{account}\n{op}\n{}\n", canonical_request_input(&filters)),
        }
    }

    fn mac(&self, position: &[u8]) -> hmac::Tag {
        hmac::sign(self.key, &self.message(position))
    }

    fn message(&self, position: &[u8]) -> Vec<u8> {
        [self.scope.as_bytes(), position].concat()
    }

    fn decode(&self, token: &str) -> Result<String, Error> {
        let encoded = token.strip_prefix('p').ok_or_else(invalid)?;
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
        if bytes.len() <= 32 {
            return Err(invalid());
        }
        let (signature, position) = bytes.split_at(32);
        // The scope is the first part of the signed message. A mismatch reveals
        // neither the issuer nor whether the order key is still present.
        hmac::verify(self.key, &self.message(position), signature).map_err(|_| invalid())?;
        String::from_utf8(position.to_owned()).map_err(|_| invalid())
    }

    fn seal(&self, cursor: &mut Value) {
        if let Some(position) = cursor.as_str() {
            let mut bytes = self.mac(position.as_bytes()).as_ref().to_vec();
            bytes.extend_from_slice(position.as_bytes());
            // A leading letter keeps base64 values unambiguous as CLI arguments.
            *cursor = Value::String(format!("p{}", URL_SAFE_NO_PAD.encode(bytes)));
        }
    }

    /// Seal the new page key and any first-page member keys of returned groups.
    /// Costs one MAC per returned continuation and no storage access.
    fn finish(&self, account: AccountId, op: &str, result: &mut Value) {
        if let Some(cursor) = result.get_mut("next_cursor") {
            self.seal(cursor);
        }
        if op == "groups.list"
            && let Some(groups) = result["groups"].as_array_mut()
        {
            for group in groups {
                if group["members_next_cursor"].is_string() {
                    let input = JsonObject::from([("group_id".into(), json!(group["group_id"]))]);
                    Self::new(self.key, account, op, &input)
                        .seal(&mut group["members_next_cursor"]);
                }
            }
        }
    }
}
