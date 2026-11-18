//! Account-scoped browser push operations and the encrypted payload shape.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Register or renew the signed-in installation's browser subscription.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PushSubscribeInput {
    /// Browser PushSubscription endpoint, restricted to supported HTTPS providers.
    pub endpoint: String,
    /// Unpadded base64url-encoded uncompressed P-256 public key.
    pub p256dh: String,
    /// Unpadded base64url-encoded 16-byte browser authentication secret.
    pub auth: String,
}

/// Remove an installation belonging to the signed-in account.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PushUnsubscribeInput {
    /// Installation UUID returned by push.devices.
    pub device_id: String,
}

/// One active subscription, with no endpoint or browser key material.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PushDevice {
    /// Revocable installation UUID.
    pub device_id: String,
    /// Display name from the installation directory.
    pub name: String,
    /// Whether the current credential belongs to this installation.
    pub current: bool,
    /// Server-enforced expiry in Unix milliseconds.
    pub expires_at_ms: i64,
}

/// Subscription inventory and the public browser application server key.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PushDevices {
    /// Unpadded base64url VAPID public key for PushManager.subscribe.
    pub application_server_key: String,
    /// The signed-in account's live subscriptions.
    pub devices: Vec<PushDevice>,
}

/// Outcome of an idempotent subscription removal.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PushRemoved {
    /// True when a stored subscription was removed.
    pub removed: bool,
}

/// Best-effort admission of a test notification for the current installation.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PushQueued {
    /// True when the background worker accepted the test.
    pub queued: bool,
}

/// The only status values delivered to a browser push receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PushStatus {
    /// A committed action awaits the initiating person.
    NeedsYou,
    /// A thread turn completed.
    Finished,
    /// A person requested a test from this installation.
    Test,
}

/// Browser notification payload, encrypted before the relay receives it.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PushPayload {
    /// Current thread title, or a fixed application title for a test.
    pub title: String,
    /// Notification label. No message, tool output, or request text.
    pub status: PushStatus,
}
