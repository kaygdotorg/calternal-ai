//! Data declarations consumed by the palette without running plugin code.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One palette command, in the web palette's command shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PaletteCommand {
    /// Stable, globally unique command identifier.
    pub id: String,
    /// Searchable command label.
    pub label: String,
    /// Display group.
    pub group: String,
    /// Extra phrases that match this command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub words: Vec<String>,
    /// Navigation address, if this command navigates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub go: Option<String>,
    /// Named host action, if the command invokes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub op: Option<String>,
}

/// A discoverable Settings section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PaletteSettingsSection {
    /// Stable section identifier.
    pub id: String,
    /// Section label.
    pub label: String,
    /// Settings address.
    pub go: String,
}

/// Optional palette declarations in a plugin manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PaletteDeclaration {
    /// Commands with their extra search phrases.
    pub commands: Vec<PaletteCommand>,
    /// Settings sections that the plugin contributes.
    pub settings_sections: Vec<PaletteSettingsSection>,
}

/// A registered palette contribution in the web palette's manifest shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PaletteManifest {
    /// Owning plugin identifier.
    pub plugin: String,
    /// Exact package version.
    pub version: String,
    /// Commands with their extra search phrases.
    pub commands: Vec<PaletteCommand>,
    /// Settings sections provided by this plugin.
    pub settings_sections: Vec<PaletteSettingsSection>,
}

/// Palette declarations visible in the selected space.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct PaletteResult {
    /// Enabled plugins with palette contributions.
    pub manifests: Vec<PaletteManifest>,
}
