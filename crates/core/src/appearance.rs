//! Account appearance and local font library wire models.
use crate::{JsonObject, OperationDescription};
use calternal_types::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

/// Available catalogue and local font sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum FontSource {
    /// Google Fonts.
    Google,
    /// Fontsource.
    Fontsource,
    /// Fontshare.
    Fontshare,
    /// An account's own file.
    Upload,
    /// The device's system typeface.
    System,
}

/// A role's selected typeface. Bundled and System faces omit the library ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FontChoice {
    /// Display family name.
    pub family: String,
    /// Catalogue, upload or device source.
    pub source: FontSource,
    /// Opaque library ID returned by catalogue or list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<String>,
}

/// Each independent typography role in the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum FontRole {
    /// Interface labels and controls.
    Ui,
    /// Conversation text.
    Chat,
    /// Code.
    Code,
    /// Terminal.
    Terminal,
    /// Wordmark.
    Brand,
}

/// The user's light and dark appearance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceScheme {
    /// Follow the device preference.
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// Where the accent color is obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceAccent {
    /// Derive it from the wallpaper.
    Wallpaper,
    /// Use the person's custom hex color.
    Custom,
    /// Use the selected preset's palette.
    Theme,
}

/// Conversation column width in the desktop shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceChatWidth {
    /// Use the available width.
    Wide,
    /// Center a readable-width column.
    Centered,
}

macro_rules! patchable {
    ($(#[$doc:meta])* $full:ident, $(#[$patch_doc:meta])* $patch:ident {
        $($(#[$field_doc:meta])* $field:ident: $ty:ty => Option<$patch_ty:ty> $(, $extra:meta)?;)*
    }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $full {
            $($(#[$field_doc])* pub $field: $ty,)*
        }

        $(#[$patch_doc])*
        #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $patch {
            $($(#[$field_doc])* #[serde(default, skip_serializing_if = "Option::is_none")]
                $(#[$extra])? #[ts(optional)] pub $field: Option<$patch_ty>,)*
        }
    };
}

patchable! {
    /// The account's material effects.
    AppearanceMaterials,
    /// Partial update of material switches.
    AppearanceMaterialsPatch {
        /// Blur backgrounds.
        blur: bool => Option<bool>;
        /// Dim backgrounds.
        dim: bool => Option<bool>;
        /// Tint backgrounds.
        tint: bool => Option<bool>;
    }
}

patchable! {
    /// The account's five font selections.
    AppearanceFonts,
    /// Partial update of role selections.
    AppearanceFontsPatch {
        /// Interface typeface.
        ui: FontChoice => Option<FontChoice>;
        /// Conversation typeface.
        chat: FontChoice => Option<FontChoice>;
        /// Code typeface.
        code: FontChoice => Option<FontChoice>;
        /// Terminal typeface.
        terminal: FontChoice => Option<FontChoice>;
        /// Wordmark typeface.
        brand: FontChoice => Option<FontChoice>;
    }
}

patchable! {
    /// The saved account appearance. A client overlays its local device settings for first paint.
    AppearancePreferences,
    /// Apply only supplied fields. Explicit null clears nullable fields.
    AppearancePatch {
        /// Wallpaper library ID, auto for the default, or null for none.
        wallpaper_id: Option<String> => Option<Option<String>>, serde(deserialize_with = "present");
        /// Material effects.
        materials: AppearanceMaterials => Option<AppearanceMaterialsPatch>;
        /// Palette preset, or null for the built-in palette.
        theme_id: Option<String> => Option<Option<String>>, serde(deserialize_with = "present");
        /// Last applied look, or null.
        preset_id: Option<String> => Option<Option<String>>, serde(deserialize_with = "present");
        /// Accent color policy.
        accent: AppearanceAccent => Option<AppearanceAccent>;
        /// Custom accent color in #rrggbb form.
        accent_color: String => Option<String>;
        /// Sidebar wordmark text.
        brand: String => Option<String>;
        /// Show streamed conversation text.
        stream_text: bool => Option<bool>;
        /// Typeface selection per role.
        fonts: AppearanceFonts => Option<AppearanceFontsPatch>;
        /// Enable motion when the device permits it.
        motion: bool => Option<bool>;
    }
}

/// Local overlay for appearance on a single device, independent of the
/// signed-in account's server snapshot. Store it at the client per installation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceAppearancePreferences {
    /// Light/dark policy. System follows the device's current color scheme.
    pub scheme: AppearanceScheme,
    /// UI size in percent, from 80 to 150.
    pub ui_size: u16,
    /// App kinds in this device's switcher order.
    pub app_order: Vec<String>,
    /// Width of this device's conversation column.
    pub chat_width: AppearanceChatWidth,
    /// Backdrop clarity between zero and one, or null to follow the device.
    pub clarity: Option<f64>,
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

/// Empty input for an account-bound read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AppearanceGetInput {}

/// Opaque family identifier, resolved against the caller's permitted library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FontIdInput {
    /// Family ID returned by the catalogue, list or upload reply.
    pub id: String,
}

/// Fetch a page of remote families using local search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FontCatalogueInput {
    /// Case-insensitive substring, at most 120 characters.
    #[ts(optional)]
    pub query: Option<String>,
    /// Optionally filter by a remote provider.
    #[ts(optional)]
    pub source: Option<FontSource>,
    /// Optionally filter by monospace classification.
    #[ts(optional)]
    pub mono: Option<bool>,
    /// Opaque next cursor from the same filters and cached snapshot.
    #[ts(optional)]
    pub cursor: Option<String>,
    /// Page size from 1 to 100. Defaults to 30.
    #[ts(optional)]
    pub limit: Option<u16>,
}

/// A merged catalogue row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontCatalogueFamily {
    /// Stable ID for installation.
    pub id: String,
    /// Display family name.
    pub family: String,
    /// Source category normalized by the server.
    pub category: String,
    /// Provider.
    pub source: FontSource,
    /// Whether the provider classifies this as monospaced.
    pub mono: bool,
}

/// A page of remote catalogue families.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontCatalogueResult {
    /// Matching families in deterministic order.
    pub families: Vec<FontCatalogueFamily>,
    /// Cursor for the next page, or null at the end.
    pub next_cursor: Option<String>,
    /// Whether a source is unreachable, stale or disabled.
    pub partial: bool,
}

/// Availability of one family at this installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum FontInstallState {
    /// No published family or active fill.
    Missing,
    /// A shared fill is in progress.
    Installing,
    /// The requested local family is available.
    Ready,
}

/// Local install state and the installation's download policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontStatusResult {
    /// Family state.
    pub state: FontInstallState,
    /// Whether remote requests are enabled.
    pub downloads_enabled: bool,
}

/// Generate CSS for the account's roles, or inspect one family for a preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FontFacesInput {
    /// Optional ID of a local family to preview.
    #[ts(optional)]
    pub id: Option<String>,
}

/// Local CSS with typeface aliases for clients' font stacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontFacesResult {
    /// Generated rules with same-origin URLs.
    pub css: String,
    /// Library ID to generated CSS family alias.
    pub families: std::collections::BTreeMap<String, String>,
}

/// Page of installed families and the authenticated account's own uploads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FontListInput {
    /// Opaque next cursor from a preceding page.
    #[ts(optional)]
    pub cursor: Option<String>,
    /// Page size from 1 to 100. Defaults to 30.
    #[ts(optional)]
    pub limit: Option<u16>,
}

/// Metadata for one locally usable family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontStoredFamily {
    /// Stable library ID.
    pub id: String,
    /// Display family name.
    pub family: String,
    /// Provider or Upload.
    pub source: FontSource,
    /// Byte count of stored files.
    pub bytes: u32,
    /// Roles in which the authenticated account uses this family.
    pub used_by: Vec<FontRole>,
}

/// The user's visible local font storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontListResult {
    /// Current page of installed or owned families.
    pub families: Vec<FontStoredFamily>,
    /// Next cursor or null.
    pub next_cursor: Option<String>,
}

/// Successful removal of an uploaded family from the caller's library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FontRemoveResult {
    /// Saved preferences after resetting the affected roles.
    pub appearance: AppearancePreferences,
}

macro_rules! operations {
    ($($constant:ident = $name:literal, $doc:literal, $mutates:literal, $input:ty => $output:ty;)*) => {
        $(#[doc = $doc] pub const $constant: &str = $name;)*
        /// Identify operations scoped to the signed-in person's appearance or font library.
        pub fn is_appearance_operation(operation: &str) -> bool {
            matches!(operation, $($constant)|*)
        }
        pub(crate) fn validate(operation: &str, input: &JsonObject) -> Result<(), Error> {
            match operation {
                $($constant => serde_json::from_value::<$input>(
                    serde_json::to_value(input).expect("JSON input"))
                    .map(|_| ())
                    .map_err(|_| Error::invalid_input("input does not match operation schema")) ,)*
                _ => Err(Error::not_found("unknown appearance operation")),
            }
        }
        pub(crate) fn catalogue() -> Vec<OperationDescription> {
            vec![$(super::validation::entry::<$input, $output>($constant, $doc, $mutates)),*]
        }
    };
}

operations! {
    APPEARANCE_GET = "appearance.get", "Read this person's appearance preferences", false, AppearanceGetInput => AppearancePreferences;
    APPEARANCE_PATCH = "appearance.patch", "Patch this person's appearance preferences", true, AppearancePatch => AppearancePreferences;
    FONTS_CATALOGUE = "fonts.catalogue", "Search the bounded shared catalogue", false, FontCatalogueInput => FontCatalogueResult;
    FONTS_INSTALL = "fonts.install", "Install a catalogue family for this installation", true, FontIdInput => FontStatusResult;
    FONTS_STATUS = "fonts.status", "Check a family's local availability", false, FontIdInput => FontStatusResult;
    FONTS_FACES = "fonts.faces", "Generate same-origin face rules", false, FontFacesInput => FontFacesResult;
    FONTS_LIST = "fonts.list", "List shared and caller-owned fonts", false, FontListInput => FontListResult;
    FONTS_REMOVE = "fonts.remove", "Remove a caller-owned family", true, FontIdInput => FontRemoveResult;
}
