//! Addressed operations for space-scoped plugin settings.
use crate::{ManifestSetting, ManifestSettingType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Read one manifest-declared setting as a Manager of the addressed space.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginSettingAddress {
    /// Space UUID.
    pub space_id: String,
    /// Installed package ID.
    pub plugin_id: String,
    /// Stable setting key from that package's manifest.
    pub key: String,
}

/// Edit one setting while retaining edits from other devices.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginSettingEdit {
    /// Space UUID.
    pub space_id: String,
    /// Installed package ID.
    pub plugin_id: String,
    /// Stable setting key from that package's manifest.
    pub key: String,
    /// Override last read for this key. Null means no override.
    pub base: Value,
    /// New override of the declared type. Null clears the override.
    pub value: Value,
    /// Explicit conflict resolution echoes every alternative the caller saw.
    #[ts(optional)]
    pub resolve: Option<Vec<Value>>,
}

/// Current values for a setting. The worker receives only its effective value.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PluginSettingValue {
    /// Value the package sees, using the manifest default when needed.
    pub effective: Value,
    /// Stored override for the next edit's base. Null indicates no override.
    pub value: Value,
    /// Concurrent alternatives preserved until explicit resolution.
    pub alternatives: Vec<Value>,
    /// True when the current settings file can be changed through the API.
    pub writable: bool,
    /// Caller-safe description of the file and line to repair, when applicable.
    pub repair: Option<String>,
}

/// Durable receipt and scoped invalidation for a setting write.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PluginSettingChange {
    /// Package whose setting should be re-read.
    pub plugin_id: String,
    /// Changed manifest key.
    pub key: String,
    /// True when manual resolution is needed.
    pub conflicted: bool,
}

/// Worker input for its own setting. Space and package are taken from admission.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerSettingInput {
    /// Stable key declared in this package's manifest.
    pub key: String,
}

impl ManifestSetting {
    /// Check the declared primitive type. Reset is represented separately by
    /// the caller. A stored value that fails this check uses the default.
    pub fn accepts(&self, value: &Value) -> bool {
        match self.value_type {
            ManifestSettingType::String => value.is_string(),
            ManifestSettingType::Boolean => value.is_boolean(),
            ManifestSettingType::Number => value.is_number(),
        }
    }
}
