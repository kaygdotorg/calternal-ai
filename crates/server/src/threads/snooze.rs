//! Shared Inbox deadlines. Manual snoozes are content metadata and subscription
//! snoozes follow the existing durable retry deadline.
use calternal_content::{Content, Item, Request};
use calternal_types::{Error, now_ms};
use serde_json::{Value, json};

const KEY: &str = "snoozed_until_ms";

/// Project the future Inbox deadline using the same clock for all rows of a page.
/// A subscription wait and a later manual snooze can coexist.
pub(super) fn effective(metadata: &Value, reason: &Value, now: i64) -> Option<i64> {
    let retry = (reason["rule"] == "subscription_reset")
        .then(|| reason["next_attempt_ms"].as_i64())
        .flatten();
    metadata[KEY]
        .as_i64()
        .max(retry)
        .filter(|&until| until > now)
}

/// Commit the shared manual deadline through content's receipted writer.
/// Zero additional body reads and the ordinary content durability boundaries.
pub(super) fn set(
    content: &Content,
    mut item: Item,
    until: Option<i64>,
    request: Request<'_>,
) -> Result<Value, Error> {
    if let Some(until) = until {
        if until <= now_ms() {
            return Err(Error::invalid_input("choose a future snooze time"));
        }
        if item.metadata["archived"] == true {
            return Err(Error::conflict("unarchive the thread before snoozing it"));
        }
        item.metadata[KEY] = json!(until);
    } else if let Some(metadata) = item.metadata.as_object_mut() {
        metadata.remove(KEY);
    }
    let result = json!({"id":item.id,"snoozed_until_ms":until});
    content.write(
        item.space,
        Some(item.id),
        "threads",
        Some(&item.revision),
        item.metadata,
        &item.bytes,
        request,
        |_| result.clone(),
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inbox_deadline_follows_manual_and_subscription_deadlines() {
        let retry = json!({"rule":"subscription_reset","next_attempt_ms":120});
        assert_eq!(effective(&Value::Null, &retry, 100), Some(120));
        assert_eq!(effective(&Value::Null, &retry, 120), None);
        assert_eq!(effective(&json!({KEY:140}), &retry, 100), Some(140));
        assert_eq!(effective(&json!({KEY:110}), &retry, 110), Some(120));
        assert_eq!(
            effective(
                &Value::Null,
                &json!({"rule":"provider_backoff","next_attempt_ms":120}),
                100
            ),
            None
        );
    }
}
