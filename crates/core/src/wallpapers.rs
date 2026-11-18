//! Wire models for the built-in wallpaper catalog.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Signed-in operation for the installation's fixed catalog of built-in photos.
pub const WALLPAPERS_LIST: &str = "wallpapers.list";

/// Empty input for the built-in catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WallpapersListInput {}

/// Photo and photographer attribution with Unsplash referral parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WallpaperCredit {
    /// Unsplash photo ID used in the same-origin rendition route.
    pub id: String,
    /// Photographer's display name.
    pub author: String,
    /// Photographer's Unsplash profile URL for the credit tooltip.
    pub author_url: String,
    /// Photo's Unsplash page URL for attribution to Unsplash.
    pub link: String,
}

/// Fixed photo catalog returned without contacting the image service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WallpapersListResult {
    /// Photo used when no wallpaper was chosen.
    pub default_id: String,
    /// All eight built-in photos in picker order.
    pub wallpapers: Vec<WallpaperCredit>,
}
